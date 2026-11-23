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
pub(crate) use handykit_process::decode_output;
use handykit_process::{configure, ProcessTree};
pub struct Outcome {
    pub status: String,
    pub code: Option<i32>,
    pub message: String,
}
pub fn validate(task: &Task, plugins: Option<&dyn crate::PluginExecutor>) -> Result<(), String> {
    if task.action.kind == "plugin" {
        let a = &task.action;
        if !a.args.is_empty()
            || !a.env.is_empty()
            || !a.cwd.is_empty()
            || !a.interpreter.is_empty()
            || a.instance != "reuse"
        {
            return Err("原生插件任务不接受脚本参数、环境覆盖或工作目录；请在插件中配置".into());
        }
        if task.timeout.is_some_and(|n| n == 0 || n > 604800) {
            return Err("超时应为 1–604800 秒".into());
        }
        return plugins
            .ok_or("此构建未接入插件宿主")?
            .validate(&a.target, a.plugin_revision.ok_or("插件任务缺少配置版本")?);
    }
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
                return Err("Windows 直接执行程序请选择 exe；脚本请选择对应解释器".into());
            }
        }
    }
    Ok(())
}
#[cfg(windows)]
fn script_argument(task: &Task) -> String {
    // Preserve the user's drive/UNC namespace. Never canonicalize to the backing
    // store or infer an MSYS mount (/c, /d), and never launch an auxiliary cygpath.
    // Forward slashes are accepted by Git Bash without exposing backing paths.
    let path = &task.action.target;
    let logical = path
        .strip_prefix(r"\\?\UNC\")
        .map(|p| format!(r"\\{p}"))
        .unwrap_or_else(|| path.strip_prefix(r"\\?\").unwrap_or(path).to_owned());
    logical.replace('\\', "/")
}
#[cfg(windows)]
fn git_bash_launcher(task: &Task) -> bool {
    task.action.kind == "script"
        && std::path::Path::new(&task.action.interpreter)
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("git-bash.exe"))
}
// The configured launcher initializes MSYSTEM/PATH/HOME and synchronously runs
// its own Bash, with inherited output handles. User data remains separate argv;
// no -c string, sibling-interpreter fallback or host-side path probing is used.
#[cfg(windows)]
const GIT_BASH_HEADLESS_ARGS: [&str; 5] = [
    "--no-needs-console",
    "--hide",
    "--no-cd",
    "--no-append-quote",
    r"--command=usr\bin\bash.exe --login",
];
fn command(task: &Task) -> Result<Command, String> {
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
        return Ok(c);
    }
    let mut c = if a.kind == "script" {
        #[cfg(not(windows))]
        let c = {
            let mut c = Command::new(&a.interpreter);
            c.arg(&a.target);
            c
        };
        #[cfg(windows)]
        let c = {
            let mut c = Command::new(&a.interpreter);
            if git_bash_launcher(task) {
                c.args(GIT_BASH_HEADLESS_ARGS);
            }
            c.arg("--").arg(script_argument(task));
            c
        };
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
    Ok(c)
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
fn failure(message: String) -> Outcome {
    Outcome {
        status: "failed".into(),
        code: None,
        message,
    }
}
pub fn execute(
    task: &Task,
    ctrl: Arc<Control>,
    plugins: Option<&dyn crate::PluginExecutor>,
) -> Outcome {
    if let Err(e) = validate(task, plugins) {
        return failure(e);
    }
    if ctrl.cancel.load(Ordering::SeqCst) {
        return Outcome {
            status: "cancelled".into(),
            code: None,
            message: "启动前已取消".into(),
        };
    }
    if task.action.kind == "plugin" {
        struct Finish(Arc<Control>);
        impl Drop for Finish {
            fn drop(&mut self) {
                self.0.finished.store(true, Ordering::SeqCst);
            }
        }
        let _finish = Finish(ctrl.clone());
        let start = Instant::now();
        let watchdog = task.timeout.map(|seconds| {
            let c = ctrl.clone();
            std::thread::spawn(move || {
                while !c.finished.load(Ordering::SeqCst) {
                    if start.elapsed() >= Duration::from_secs(seconds) {
                        c.cancel.store(true, Ordering::SeqCst);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
            })
        });
        let output_ctrl = ctrl.clone();
        let sink: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |line| {
            let mut o = output_ctrl.output.lock().unwrap();
            if o.stdout.len() < 10 * 1024 * 1024 {
                o.stdout.extend_from_slice(line.as_bytes());
                o.stdout.push(b'\n');
            } else {
                o.truncated = true;
            }
        });
        let result = plugins.unwrap().execute(
            &task.action.target,
            task.action.plugin_revision.unwrap(),
            &ctrl.cancel,
            sink,
        );
        ctrl.finished.store(true, Ordering::SeqCst);
        if let Some(watch) = watchdog {
            let _ = watch.join();
        }
        let stopped = ctrl.cancel.load(Ordering::SeqCst);
        let timed_out = task
            .timeout
            .is_some_and(|n| start.elapsed() >= Duration::from_secs(n))
            && stopped;
        return Outcome {
            status: if timed_out {
                "timed_out"
            } else if stopped {
                "cancelled"
            } else if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            }
            .into(),
            code: None,
            message: result
                .err()
                .unwrap_or_else(|| "原生插件执行完成；详见插件执行记录".into()),
        };
    }
    let mut cmd = match command(task) {
        Ok(c) => c,
        Err(e) => return failure(e),
    };
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
    configure(&mut cmd);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return failure(format!("启动失败：{e}")),
    };
    let tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return failure(format!("无法管理子进程：{error}"));
        }
    };
    let stdout = drain(Box::new(child.stdout.take().unwrap()), ctrl.clone(), false);
    let stderr = drain(Box::new(child.stderr.take().unwrap()), ctrl.clone(), true);
    let start = Instant::now();
    #[allow(unused_mut)]
    let mut result = loop {
        if ctrl.cancel.load(Ordering::SeqCst)
            || task
                .timeout
                .is_some_and(|n| start.elapsed() >= Duration::from_secs(n))
        {
            let cancelled = ctrl.cancel.load(Ordering::SeqCst);
            tree.kill();
            let stop_message = tree
                .warning()
                .unwrap_or_else(|| "已请求停止进程及其进程树；外部操作结果需核对".into());
            let _ = child.wait();
            break Outcome {
                status: if cancelled { "cancelled" } else { "timed_out" }.into(),
                code: None,
                message: if cancelled {
                    stop_message
                } else {
                    format!("任务超过配置的超时时间，{stop_message}")
                },
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
                tree.kill();
                let _ = child.wait();
                break failure(e.to_string());
            }
        }
    };
    // The reader has a bounded drain after the supervised root exits.
    tree.kill();
    ctrl.finished.store(true, Ordering::SeqCst);
    let _ = stdout.join();
    let _ = stderr.join();
    #[cfg(windows)]
    if git_bash_launcher(task) {
        result.message.push_str("；Git Bash 后台命令模式");
    }
    result
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    fn task(interpreter: &str, target: &str) -> Task {
        serde_json::from_value(serde_json::json!({
            "id":"", "revision":0, "name":"logical paths", "description":"",
            "enabled":false, "cron":"* * * * *", "zone":"UTC", "timeout":5,
            "action": {"kind":"script", "target":target, "interpreter":interpreter,
                "args":[{"value":"中文 空格"},{"value":""},{"value":"$HOME"}],
                "env":[], "cwd":r"D:\沙箱工作区", "instance":"reuse"}
        }))
        .unwrap()
    }
    #[test]
    fn configured_logical_paths_and_environment_are_preserved() {
        // No physical files required: building a command must not inspect a backing
        // path, find a sibling interpreter or launch cygpath in another context.
        for interpreter in [
            r"C:\Git\git-bash.exe",
            r"C:\Git\bin\bash.exe",
            r"C:\Git\usr\bin\sh.exe",
        ] {
            let task = task(interpreter, r"C:\映射目录\script with spaces.sh");
            let cmd = command(&task).unwrap();
            assert_eq!(cmd.get_program(), std::ffi::OsStr::new(interpreter));
            let args: Vec<_> = cmd
                .get_args()
                .map(|a| a.to_string_lossy().into_owned())
                .collect();
            let mut expected = Vec::new();
            if git_bash_launcher(&task) {
                expected.extend(GIT_BASH_HEADLESS_ARGS.map(String::from));
            }
            expected.extend(
                [
                    "--",
                    "C:/映射目录/script with spaces.sh",
                    "中文 空格",
                    "",
                    "$HOME",
                ]
                .map(String::from),
            );
            assert_eq!(args, expected);
            assert_eq!(
                cmd.get_current_dir(),
                Some(std::path::Path::new(r"D:\沙箱工作区"))
            );
            assert_eq!(
                cmd.get_envs().count(),
                0,
                "no implicit PATH/HOME/MSYSTEM edits or env removals"
            );
        }
    }
    #[test]
    fn explicit_environment_overrides_are_preserved() {
        let mut task = task(r"C:\Git\git-bash.exe", r"C:\scripts\script.sh");
        task.action.env = vec![crate::model::Environment {
            key: "PATH".into(),
            value: r"C:\sandbox-tools".into(),
            secret: false,
        }];
        let cmd = command(&task).unwrap();
        assert_eq!(
            cmd.get_envs().collect::<Vec<_>>(),
            vec![(
                std::ffi::OsStr::new("PATH"),
                Some(std::ffi::OsStr::new(r"C:\sandbox-tools"))
            )]
        );
    }
    #[test]
    fn unc_and_extended_paths_do_not_turn_into_backing_paths() {
        for (input, expected) in [
            (r"\\server\share\script.sh", "//server/share/script.sh"),
            (r"\\?\C:\scripts\script.sh", "C:/scripts/script.sh"),
        ] {
            assert_eq!(script_argument(&task(r"C:\Git\bash.exe", input)), expected);
        }
    }
}
