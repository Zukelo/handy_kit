use crate::model::Task;
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub truncated: bool,
}
#[derive(Default)]
pub struct Control {
    pub cancel: AtomicBool,
    pub finished: AtomicBool,
    pub output: Mutex<Output>,
}
pub struct Outcome {
    pub status: String,
    pub code: Option<i32>,
    pub message: String,
}
pub fn validate(task: &Task) -> Result<(), String> {
    if !["script", "executable", "application"].contains(&task.action.kind.as_str()) {
        return Err("未知动作类型".into());
    }
    let a = &task.action;
    let path = std::path::Path::new(&a.target);
    if !path.is_absolute() {
        return Err("目标必须是绝对路径，不自动展开 ~ 或环境变量".into());
    }
    if a.kind == "application" && cfg!(target_os = "macos") {
        if !path.is_dir() || path.extension().is_none_or(|x| x != "app") {
            return Err("请选择存在的 .app 应用包".into());
        }
    } else {
        executable_file(path, a.kind != "script")?;
    }
    if a.kind == "script" {
        let p = std::path::Path::new(&a.interpreter);
        if !p.is_absolute() {
            return Err("请选择解释器绝对路径".into());
        }
        executable_file(p, true)?;
    }
    if a.kind != "application"
        && !a.cwd.is_empty()
        && (!std::path::Path::new(&a.cwd).is_absolute() || !std::path::Path::new(&a.cwd).is_dir())
    {
        return Err("工作目录必须是存在的绝对目录".into());
    }
    if a.args.len() > 128 || a.env.len() > 128 {
        return Err("参数或环境变量不能超过 128 项".into());
    }
    for arg in &a.args {
        if arg.value.len() > 32768 || arg.value.contains('\0') {
            return Err("参数过长或包含 NUL 字符".into());
        }
    }
    let mut keys = std::collections::HashSet::new();
    for e in &a.env {
        let mut chars = e.key.chars();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            || e.value.contains('\0')
            || e.value.len() > 32768
            || !keys.insert(if cfg!(windows) {
                e.key.to_uppercase()
            } else {
                e.key.clone()
            })
        {
            return Err("环境变量名称无效、重复或值过长".into());
        }
    }
    if !["reuse", "new"].contains(&a.instance.as_str()) {
        return Err("未知应用实例策略".into());
    }
    if a.kind == "application" && (!a.cwd.is_empty() || !a.env.is_empty() || task.timeout.is_some())
    {
        return Err("应用启动模式不支持工作目录、环境覆盖或受管超时".into());
    }
    if task.timeout.is_some_and(|n| n == 0 || n > 604800) {
        return Err("超时应为 1–604800 秒，留空表示不限时".into());
    }
    Ok(())
}
fn executable_file(path: &std::path::Path, execute: bool) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    if execute {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if path
                .metadata()
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o111
                == 0
            {
                return Err(format!("文件没有执行权限：{}", path.display()));
            }
        }
        #[cfg(windows)]
        {
            if !path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("exe"))
            {
                return Err("Windows 首版仅直接执行 exe；bat/cmd/ps1 需要专用适配".into());
            }
        }
    }
    Ok(())
}
fn command(task: &Task) -> Command {
    let a = &task.action;
    #[cfg(target_os = "macos")]
    if a.kind == "application" {
        let mut c = Command::new("/usr/bin/open");
        if a.instance == "new" {
            c.arg("-n");
        }
        c.arg("-a").arg(&a.target);
        if !a.args.is_empty() {
            c.arg("--args").args(a.args.iter().map(|a| &a.value));
        }
        return c;
    }
    let mut c = if a.kind == "script" {
        let mut c = Command::new(&a.interpreter);
        c.arg(&a.target);
        c
    } else {
        Command::new(&a.target)
    };
    c.args(a.args.iter().map(|a| &a.value));
    if a.kind != "application" {
        let cwd = if a.cwd.is_empty() {
            std::path::Path::new(&a.target)
                .parent()
                .unwrap()
                .to_path_buf()
        } else {
            a.cwd.clone().into()
        };
        c.current_dir(cwd);
        for e in &a.env {
            c.env(&e.key, &e.value);
        }
    }
    c
}
trait Pipe: Read + Send {
    fn ready_read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize>;
}
#[cfg(unix)]
impl<T: Read + Send + std::os::fd::AsRawFd> Pipe for T {
    fn ready_read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            let fd = self.as_raw_fd();
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        self.read(bytes)
    }
}
#[cfg(windows)]
impl<T: Read + Send + std::os::windows::io::AsRawHandle> Pipe for T {
    fn ready_read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let mut available = 0;
        unsafe {
            if windows_sys::Win32::System::Pipes::PeekNamedPipe(
                self.as_raw_handle() as _,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        if available == 0 {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        let count = bytes.len().min(available as usize);
        self.read(&mut bytes[..count])
    }
}
fn drain(
    mut reader: Box<dyn Pipe>,
    ctrl: Arc<Control>,
    stderr: bool,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut bytes = [0u8; 8192];
        let mut finishing = None;
        loop {
            if ctrl.finished.load(Ordering::SeqCst) {
                let start = finishing.get_or_insert_with(Instant::now);
                if start.elapsed() > Duration::from_millis(250) {
                    ctrl.output.lock().unwrap().truncated = true;
                    break;
                }
            }
            let n = match reader.ready_read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if finishing.is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(_) => break,
            };
            let mut out = ctrl.output.lock().unwrap();
            let remaining =
                (10 * 1024 * 1024usize).saturating_sub(out.stdout.len() + out.stderr.len());
            let take = n.min(remaining);
            if stderr {
                out.stderr.extend_from_slice(&bytes[..take]);
            } else {
                out.stdout.extend_from_slice(&bytes[..take]);
            }
            if take < n {
                out.truncated = true;
            }
        }
    })
}
// Kill only the managed group/job owned by this run; never independent GUI apps.
#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) {
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}
#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Job {
    fn assign(child: &std::process::Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0
            {
                windows_sys::Win32::Foundation::CloseHandle(job);
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(Self(job))
        }
    }
    fn kill(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
#[cfg(windows)]
fn resume_managed(child: &std::process::Child) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::*,
        System::{Diagnostics::ToolHelp::*, Threading::*},
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut ok = Thread32First(snapshot, &mut entry);
        let mut resumed = false;
        while ok != 0 {
            if entry.th32OwnerProcessID == child.id() {
                let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if !thread.is_null() {
                    resumed = ResumeThread(thread) != u32::MAX;
                    CloseHandle(thread);
                }
                break;
            }
            ok = Thread32Next(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        if resumed {
            Ok(())
        } else {
            Err("无法恢复已纳入 Job 的受管进程".into())
        }
    }
}
fn failure(message: String) -> Outcome {
    Outcome {
        status: "failed".into(),
        code: None,
        message,
    }
}
pub fn execute(task: &Task, ctrl: Arc<Control>) -> Outcome {
    if let Err(e) = validate(task) {
        return failure(e);
    }
    if ctrl.cancel.load(Ordering::SeqCst) {
        return Outcome {
            status: "cancelled".into(),
            code: None,
            message: "启动前已取消".into(),
        };
    }
    let mut cmd = command(task);
    cmd.stdin(Stdio::null());
    if task.action.kind == "application" {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        match cmd.spawn() {
            Err(e) => return failure(format!("启动请求失败：{e}")),
            Ok(mut child) => {
                #[cfg(target_os = "macos")]
                {
                    return match child.wait() {
                        Ok(s) if s.success() => Outcome {
                            status: "launched".into(),
                            code: None,
                            message: "系统已接受应用启动请求；不代表应用内部工作完成".into(),
                        },
                        Ok(s) => failure(format!("系统拒绝启动请求：{s}")),
                        Err(e) => failure(e.to_string()),
                    };
                }
                #[cfg(not(target_os = "macos"))]
                {
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    return Outcome {
                        status: "launched".into(),
                        code: None,
                        message:
                            "已创建独立应用进程；不代表其内部工作完成，复用/新实例由目标应用决定"
                                .into(),
                    };
                }
            }
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000 | 0x00000004); // NO_WINDOW | CREATE_SUSPENDED; assign job before user code runs.
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return failure(format!("启动失败：{e}")),
    };
    #[cfg(windows)]
    let job = match Job::assign(&child) {
        Ok(j) => j,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return failure(format!("无法建立受管进程组：{e}"));
        }
    };
    #[cfg(windows)]
    if let Err(e) = resume_managed(&child) {
        job.kill();
        let _ = child.wait();
        return failure(e);
    }
    let stdout = drain(Box::new(child.stdout.take().unwrap()), ctrl.clone(), false);
    let stderr = drain(Box::new(child.stderr.take().unwrap()), ctrl.clone(), true);
    let start = Instant::now();
    let result = loop {
        if ctrl.cancel.load(Ordering::SeqCst)
            || task
                .timeout
                .is_some_and(|n| start.elapsed() >= Duration::from_secs(n))
        {
            let cancelled = ctrl.cancel.load(Ordering::SeqCst);
            #[cfg(unix)]
            kill_tree(&mut child);
            #[cfg(windows)]
            job.kill();
            let _ = child.wait();
            break Outcome {
                status: if cancelled { "cancelled" } else { "timed_out" }.into(),
                code: None,
                message: if cancelled {
                    "已停止受管进程组"
                } else {
                    "任务超过配置的超时时间，已停止受管进程组"
                }
                .into(),
            };
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break Outcome {
                    status: if status.success() {
                        "succeeded"
                    } else {
                        "failed"
                    }
                    .into(),
                    code: status.code(),
                    message: if status.success() {
                        "受管进程正常退出".into()
                    } else {
                        format!("受管进程异常退出：{status}")
                    },
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                #[cfg(unix)]
                kill_tree(&mut child);
                #[cfg(windows)]
                job.kill();
                let _ = child.wait();
                break failure(e.to_string());
            }
        }
    };
    // Descendants retaining pipes cannot make log collection hang after the managed parent exits.
    #[cfg(unix)]
    kill_tree(&mut child);
    #[cfg(windows)]
    job.kill();
    ctrl.finished.store(true, Ordering::SeqCst);
    let _ = stdout.join();
    let _ = stderr.join();
    result
}
