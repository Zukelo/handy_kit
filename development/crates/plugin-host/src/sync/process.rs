use super::*;
use std::process::{Command, Stdio};
pub fn redact(s: &str, password: &str) -> String {
    use std::sync::LazyLock;
    static USERINFO: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)((?:https?|ssh|svn|svn\+ssh|git)://)[^/@\s]+@").unwrap()
    });
    static QUERY: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)(https?://[^?\s]+)\?[^\s]+").unwrap());
    static SECRET: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?im)((?:password|token|secret)\s*[=:])[^\r\n]*").unwrap()
    });
    let text = if password.is_empty() {
        s.to_string()
    } else {
        s.replace(password, "[REDACTED]")
    };
    let text = USERINFO.replace_all(&text, "${1}[REDACTED]@");
    let text = QUERY.replace_all(&text, "${1}?[REDACTED]");
    SECRET.replace_all(&text, "${1}[REDACTED]").into_owned()
}
use handykit_process::decode_output;
use handykit_process::{configure, ProcessTree};
pub struct Runner<'a> {
    pub ctrl: &'a Control,
    pub external: Option<&'a AtomicBool>,
    pub timeout: u64,
    pub password: &'a str,
}
impl Runner<'_> {
    pub fn call(
        &self,
        exe: &Path,
        args: &[String],
        cwd: &Path,
        label: &str,
        stdin_password: bool,
    ) -> Result<String> {
        self.ctrl.check(self.external)?;
        self.ctrl.log(&format!("开始：{label}（不记录原始参数）"));
        let mut cmd = Command::new(exe);
        cmd.args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(if stdin_password {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_PARAMETERS",
            "GIT_NAMESPACE",
            "GIT_TRACE",
            "GIT_TRACE_PACKET",
            "GIT_TRACE_CURL",
            "GIT_CURL_VERBOSE",
            "GIT_TRACE_SETUP",
            "GIT_TRACE2",
            "GIT_TRACE2_EVENT",
        ] {
            cmd.env_remove(key);
        }
        configure(&mut cmd);
        let mut child = cmd.spawn().map_err(|e| format!("{label} 启动失败：{e}"))?;
        let guard = match ProcessTree::attach(&child) {
            Ok(g) => g,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        };
        if stdin_password {
            if let Some(mut input) = child.stdin.take() {
                if let Err(e) = input
                    .write_all(self.password.as_bytes())
                    .and_then(|_| input.write_all(b"\n"))
                {
                    guard.kill();
                    let _ = child.wait();
                    return Err(format!("写入 SVN 认证输入失败：{e}"));
                }
            }
        }
        let drain = |mut pipe: Box<dyn Read + Send>| {
            std::thread::spawn(move || {
                let mut out = Vec::new();
                let mut buf = [0; 4096];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let take = n.min(65536usize.saturating_sub(out.len()));
                            out.extend_from_slice(&buf[..take]);
                        }
                    }
                }
                out
            })
        };
        let out = drain(Box::new(child.stdout.take().unwrap()));
        let err = drain(Box::new(child.stderr.take().unwrap()));
        let start = Instant::now();
        let result = loop {
            if let Err(e) = self.ctrl.check(self.external) {
                guard.kill();
                let _ = child.wait();
                break Err(e);
            }
            if start.elapsed() >= Duration::from_secs(self.timeout) {
                guard.kill();
                let _ = child.wait();
                break Err(format!(
                    "{label} 超过 {} 秒；远端写入结果需人工核对",
                    self.timeout
                ));
            }
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => std::thread::sleep(Duration::from_millis(25)),
                Err(e) => {
                    guard.kill();
                    let _ = child.wait();
                    break Err(format!("{label} 状态查询失败：{e}"));
                }
            }
        };
        guard.kill();
        if let Some(warning) = guard.warning() {
            self.ctrl.log(&warning);
        }
        let collect = |h: std::thread::JoinHandle<Vec<u8>>| {
            let until = Instant::now() + Duration::from_secs(1);
            while !h.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            if h.is_finished() {
                decode_output(&h.join().unwrap_or_default(), true)
            } else {
                "[输出管道未关闭，已结束等待]".into()
            }
        };
        let stdout = redact(&collect(out), self.password);
        let stderr = redact(&collect(err), self.password);
        for (kind, value) in [("输出", stdout.trim()), ("错误输出", stderr.trim())] {
            if !value.is_empty() {
                self.ctrl.log(&format!("{label} {kind}：{value}"));
            }
        }
        let status = result?;
        if !status.success() {
            return Err(format!(
                "{label} 退出码 {:?}：{}",
                status.code(),
                stderr.trim()
            ));
        }
        self.ctrl.log(&format!("完成：{label}"));
        Ok(stdout.trim().into())
    }
}
pub fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}
pub fn path(p: &Path) -> Result<String> {
    p.to_str()
        .map(str::to_string)
        .ok_or("路径不是 UTF-8，无法传递给程序".into())
}
