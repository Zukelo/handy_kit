//! Shared process lifecycle for supervised local commands.
//! Normal startup preserves sandbox context; it never requests job breakaway.
#[cfg(windows)]
use std::{
    path::PathBuf,
    process::Stdio,
    sync::Mutex,
    time::{Duration, Instant},
};
use std::{
    process::{Child, Command},
    sync::atomic::{AtomicBool, Ordering},
};

pub fn configure(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
}

// Live snapshots may end in an incomplete UTF-8 character. Omit that suffix
// until the next read rather than transiently decoding the whole log as ANSI.
pub fn decode_output(bytes: &[u8], complete: bool) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => return text.into(),
        Err(error) if error.error_len().is_none() => {
            return String::from_utf8_lossy(if complete {
                bytes
            } else {
                &bytes[..error.valid_up_to()]
            })
            .into_owned();
        }
        _ => {}
    }
    #[cfg(windows)]
    if let Some(text) = decode_code_page(bytes, 0) {
        return text;
    }
    String::from_utf8_lossy(bytes).into_owned()
}
#[cfg(windows)]
fn decode_code_page(bytes: &[u8], code_page: u32) -> Option<String> {
    use windows_sys::Win32::Globalization::MultiByteToWideChar;
    let len = i32::try_from(bytes.len()).ok()?;
    if len == 0 {
        return Some(String::new());
    }
    unsafe {
        let size = MultiByteToWideChar(code_page, 0, bytes.as_ptr(), len, std::ptr::null_mut(), 0);
        if size == 0 {
            return None;
        }
        let mut wide = vec![0u16; size as usize];
        if MultiByteToWideChar(code_page, 0, bytes.as_ptr(), len, wide.as_mut_ptr(), size) == 0 {
            return None;
        }
        String::from_utf16(&wide).ok()
    }
}
#[cfg(unix)]
pub struct ProcessTree {
    pid: u32,
    stopped: AtomicBool,
}
#[cfg(unix)]
impl ProcessTree {
    pub fn attach(child: &Child) -> std::io::Result<Self> {
        Ok(Self {
            pid: child.id(),
            stopped: AtomicBool::new(false),
        })
    }
    pub fn warning(&self) -> Option<String> {
        None
    }
    pub fn kill(&self) {
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        unsafe {
            libc::kill(-(self.pid as i32), libc::SIGKILL);
        }
    }
}
#[cfg(unix)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.kill();
    }
}
#[cfg(windows)]
pub struct ProcessTree {
    // Retain a process handle until termination completes; never target an image
    // name or an unrelated PID. No job assignment or breakaway is used.
    process: std::os::windows::io::OwnedHandle,
    pid: u32,
    stopped: AtomicBool,
    warning: Mutex<Option<String>>,
}
#[cfg(windows)]
impl ProcessTree {
    pub fn attach(child: &Child) -> std::io::Result<Self> {
        use std::os::windows::io::AsHandle;
        Ok(Self {
            process: child.as_handle().try_clone_to_owned()?,
            pid: child.id(),
            stopped: AtomicBool::new(false),
            warning: Mutex::new(None),
        })
    }
    pub fn warning(&self) -> Option<String> {
        self.warning.lock().unwrap().clone()
    }
    pub fn kill(&self) {
        use std::os::windows::{io::AsRawHandle, process::CommandExt};
        use windows_sys::Win32::{
            Foundation::WAIT_TIMEOUT,
            System::{
                SystemInformation::GetSystemDirectoryW,
                Threading::{TerminateProcess, WaitForSingleObject},
            },
        };
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        let handle = self.process.as_raw_handle() as _;
        if unsafe { WaitForSingleObject(handle, 0) } != WAIT_TIMEOUT {
            return;
        }
        // Best-effort tree cancellation only while the root process is live.
        // Resolve the OS tool from the system API, never from PATH or a shell.
        let tree_stopped = (|| -> Option<bool> {
            let mut buffer = vec![0u16; 32768];
            let size =
                unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
            if size == 0 || size >= buffer.len() {
                return None;
            }
            use std::os::windows::ffi::OsStringExt;
            let exe =
                PathBuf::from(std::ffi::OsString::from_wide(&buffer[..size])).join("taskkill.exe");
            let mut killer = Command::new(exe)
                .args(["/PID", &self.pid.to_string(), "/T", "/F"])
                .creation_flags(0x08000000)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let start = Instant::now();
            loop {
                match killer.try_wait() {
                    Ok(Some(status)) => return Some(status.success()),
                    Ok(None) if start.elapsed() < Duration::from_secs(2) => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    _ => {
                        let _ = killer.kill();
                        let _ = killer.wait();
                        return None;
                    }
                }
            }
        })()
        .unwrap_or(false);
        if !tree_stopped {
            *self.warning.lock().unwrap() = Some(
                "Windows 进程树停止未确认；已请求停止直接子进程，请核对后代进程及外部操作结果"
                    .into(),
            );
        }
        // Always stop the retained process itself if the tree tool was blocked.
        if unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT {
            unsafe {
                TerminateProcess(handle, 1);
            }
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_utf8_is_not_decoded_as_ansi() {
        let bytes = "begin 中文".as_bytes();
        assert_eq!(decode_output(&bytes[..bytes.len() - 1], false), "begin 中");
        assert_eq!(decode_output(&bytes[..bytes.len() - 1], true), "begin 中�");
        assert_eq!(decode_output(bytes, true), "begin 中文");
    }
    #[cfg(windows)]
    #[test]
    fn ansi_chinese_is_decoded_without_loss() {
        assert_eq!(
            decode_code_page(&[0xd2, 0xbd, 0xc1, 0xc6, 0xbf, 0xe2], 936).unwrap(),
            "医疗库"
        );
    }
}
