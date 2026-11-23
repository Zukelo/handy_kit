pub mod sync;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Operation {
    #[serde(rename = "workspace.read")]
    WorkspaceRead,
    #[serde(rename = "git.version")]
    GitVersion,
    #[serde(rename = "svn.version")]
    SvnVersion,
    #[serde(rename = "git.local-roundtrip")]
    GitLocal,
    #[serde(rename = "svn.local-roundtrip")]
    SvnLocal,
    #[serde(rename = "code-sync.run")]
    CodeSync,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub api_version: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub operations: Vec<Operation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<sync::Workflow>,
}
#[derive(Serialize)]
pub struct Installed {
    pub manifest: Manifest,
    pub enabled: bool,
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub git: String,
    pub svn: String,
    pub svnadmin: String,
    pub workspace: String,
    pub test_file: String,
    pub local_roundtrip: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub status: String,
    pub detail: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub plugin_id: String,
    pub plugin_version: String,
    pub platform: String,
    pub checks: Vec<Check>,
    pub scope: String,
}
pub struct Host {
    root: PathBuf,
    _owner: fs::File,
    lock: Mutex<()>,
    sync_jobs: Mutex<std::collections::HashMap<String, std::sync::Arc<sync::Control>>>,
    sync_stopping: std::sync::atomic::AtomicBool,
    sync_unpersisted: Mutex<std::collections::HashMap<String, sync::Run>>,
}
fn valid_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.')
        || id.starts_with('.')
        || id.ends_with('.')
        || id.contains("..")
    {
        return Err("插件 ID 无效".into());
    }
    Ok(())
}
impl Manifest {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() > 65536 {
            return Err("插件包超过 64 KiB".into());
        }
        let m: Self = serde_json::from_slice(data).map_err(|e| format!("插件包格式错误：{e}"))?;
        valid_id(&m.id)?;
        if m.format != "handykit.plugin" || m.api_version != 2 {
            return Err("不支持的插件包或宿主 API 版本".into());
        }
        if m.operations.contains(&Operation::CodeSync) {
            m.workflow
                .as_ref()
                .ok_or("同步插件包缺少工作流定义")?
                .validate()?;
        } else if m.workflow.is_some() {
            return Err("未声明同步能力的插件不能携带同步工作流".into());
        }
        if m.name.is_empty()
            || m.name.len() > 180
            || m.description.len() > 2000
            || m.operations.is_empty()
            || m.operations.len() > 5
            || m.version.split('.').count() != 3
            || !m
                .version
                .split('.')
                .all(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("插件元数据无效".into());
        }
        for (i, op) in m.operations.iter().enumerate() {
            if m.operations[..i].contains(op) {
                return Err("插件能力重复".into());
            }
        }
        Ok(m)
    }
}
impl Host {
    pub fn open(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let owner = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("host.lock"))
            .map_err(|e| e.to_string())?;
        owner
            .try_lock()
            .map_err(|_| "此插件目录已由另一宿主使用".to_string())?;
        let host = Self {
            root,
            _owner: owner,
            lock: Mutex::new(()),
            sync_jobs: Mutex::new(std::collections::HashMap::new()),
            sync_stopping: std::sync::atomic::AtomicBool::new(false),
            sync_unpersisted: Mutex::new(std::collections::HashMap::new()),
        };
        host.recover_sync_runs()?;
        Ok(host)
    }
    fn manifest(&self, id: &str) -> Result<Manifest> {
        valid_id(id)?;
        let mut data = Vec::new();
        fs::File::open(self.root.join(id).join("manifest.json"))
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        let manifest = Manifest::parse(&data)?;
        if manifest.id != id {
            return Err("插件目录与 ID 不匹配".into());
        }
        Ok(manifest)
    }
    pub fn list(&self) -> Result<Vec<Installed>> {
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for e in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            if !e.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().into_owned();
            if id == "_data" || id.starts_with(".install-") {
                continue;
            }
            let m = self.manifest(&id)?;
            out.push(Installed {
                manifest: m,
                enabled: e.path().join("enabled").is_file(),
            });
        }
        out.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        Ok(out)
    }
    pub fn install(&self, data: &[u8]) -> Result<Manifest> {
        let m = Manifest::parse(data)?;
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        let dir = self.root.join(&m.id);
        if dir.exists() {
            return Err("同 ID 插件已安装，请先卸载".into());
        }
        let staging = tempfile::Builder::new()
            .prefix(".install-")
            .tempdir_in(&self.root)
            .map_err(|e| e.to_string())?;
        let manifest = serde_json::to_vec_pretty(&m).map_err(|e| e.to_string())?;
        {
            use std::io::Write;
            let mut file = fs::File::create(staging.path().join("manifest.json"))
                .map_err(|e| e.to_string())?;
            file.write_all(&manifest)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
        }
        fs::write(staging.path().join("enabled"), []).map_err(|e| e.to_string())?;
        fs::rename(staging.path(), &dir).map_err(|e| format!("发布插件失败：{e}"))?;
        Ok(m)
    }
    pub fn toggle(&self, id: &str, enabled: bool) -> Result<()> {
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        self.manifest(id)?;
        if self.sync_running_count() > 0 {
            return Err("有原生同步正在运行，暂不能启停插件".into());
        }
        let p = self.root.join(id).join("enabled");
        if enabled {
            fs::write(p, []).map_err(|e| e.to_string())
        } else if p.exists() {
            fs::remove_file(p).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }
    pub fn uninstall(&self, id: &str) -> Result<()> {
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        self.manifest(id)?;
        if self.sync_running_count() > 0 {
            return Err("有原生同步正在运行，暂不能卸载插件".into());
        }
        fs::remove_dir_all(self.root.join(id)).map_err(|e| e.to_string())
    }
    pub fn run(&self, id: &str, s: &Settings) -> Result<Report> {
        // Keep one operation in flight per host, so disable/uninstall cannot race a run.
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        let m = self.manifest(id)?;
        if !self.root.join(id).join("enabled").is_file() {
            return Err("插件已停用".into());
        }
        let cwd = PathBuf::from(&s.workspace);
        if !cwd.is_absolute() || !cwd.is_dir() {
            return Err("请选择存在的绝对工作目录".into());
        }
        let mut checks = Vec::new();
        for op in &m.operations {
            match op {
                Operation::WorkspaceRead => {
                    checks.push(Check {
                        name: "小工坊主进程工作目录".into(),
                        status: "passed".into(),
                        detail: cwd.display().to_string(),
                    });
                    if !s.test_file.is_empty() {
                        let p = Path::new(&s.test_file);
                        let r = if !p.is_absolute() {
                            Err("测试文件必须是绝对路径".into())
                        } else {
                            fs::File::open(p)
                                .and_then(|mut f| {
                                    let mut byte = [0];
                                    f.read(&mut byte).map(|_| ())
                                })
                                .map_err(|e| e.to_string())
                        };
                        record(
                            &mut checks,
                            "小工坊主进程读取指定文件",
                            r.map(|_| s.test_file.clone()),
                        );
                    }
                }
                Operation::GitVersion => record(
                    &mut checks,
                    "Git 版本",
                    version(&s.git, "git", &["--version"], &cwd),
                ),
                Operation::SvnVersion => record(
                    &mut checks,
                    "SVN 版本",
                    version(&s.svn, "svn", &["--version", "--quiet"], &cwd),
                ),
                Operation::GitLocal if s.local_roundtrip => record(
                    &mut checks,
                    "Git 本地 clone / merge / push 自检",
                    git_local(s, &cwd),
                ),
                Operation::SvnLocal if s.local_roundtrip => record(
                    &mut checks,
                    "SVN 本地 checkout / commit 自检",
                    svn_local(s, &cwd),
                ),
                _ => {}
            }
        }
        Ok(Report{plugin_id:m.id,plugin_version:m.version,platform:std::env::consts::OS.into(),checks,scope:"仅本机版本、文件视图及所选本地自检；未验证真实远端网络、认证或原生 libgit2/libsvn。Windows/Cloudaemon 结果必须在目标环境运行本插件取得。".into()})
    }
}
fn record(checks: &mut Vec<Check>, name: &str, result: Result<String>) {
    match result {
        Ok(detail) => checks.push(Check {
            name: name.into(),
            status: "passed".into(),
            detail,
        }),
        Err(detail) => checks.push(Check {
            name: name.into(),
            status: "failed".into(),
            detail,
        }),
    }
}
fn resolve(selected: &str, name: &str) -> Result<PathBuf> {
    if !selected.is_empty() {
        let p = PathBuf::from(selected);
        if !p.is_absolute() || !p.is_file() {
            return Err(format!("{name} 路径必须是存在的绝对文件路径"));
        }
        return Ok(p);
    }
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    for d in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        if d.is_absolute() {
            let p = d.join(&exe);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err(format!(
        "PATH 中未找到 {exe}，请显式选择程序；不会自动安装工具"
    ))
}
fn run(exe: &Path, args: &[&str], cwd: &Path, isolated: bool) -> Result<String> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if isolated {
        cmd.env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", cwd.join("empty-git-config"))
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .env_remove("GIT_CONFIG_COUNT");
    }
    handykit_process::configure(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("{} 启动失败：{e}", exe.display()))?;
    let tree = match handykit_process::ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.to_string());
        }
    };
    let drain = |mut p: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut out = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                match p.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let take = n.min(32768usize.saturating_sub(out.len()));
                        out.extend_from_slice(&buffer[..take]);
                    }
                }
            }
            out
        })
    };
    let out = drain(Box::new(child.stdout.take().unwrap()));
    let err = drain(Box::new(child.stderr.take().unwrap()));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Ok(s),
            Ok(None) if start.elapsed() < Duration::from_secs(15) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            _ => {
                tree.kill();
                let _ = child.wait();
                break Err("程序运行超过 15 秒或状态查询失败".to_string());
            }
        }
    };
    tree.kill();
    // Fixed probes should not spawn detached children. Never block the UI waiting on inherited pipes.
    let read = |h: std::thread::JoinHandle<Vec<u8>>| {
        let until = Instant::now() + Duration::from_millis(200);
        while !h.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        if h.is_finished() {
            handykit_process::decode_output(&h.join().unwrap_or_default(), true)
        } else {
            "[输出管道未关闭]".into()
        }
    };
    let stdout = read(out);
    let stderr = read(err);
    let status = status?;
    if !status.success() {
        return Err(format!(
            "退出码 {:?}：{} {}",
            status.code(),
            stdout.trim(),
            stderr.trim()
        ));
    }
    Ok(stdout.trim().into())
}
fn version(selected: &str, name: &str, args: &[&str], cwd: &Path) -> Result<String> {
    let p = resolve(selected, name)?;
    let out = run(&p, args, cwd, false)?;
    let valid = if name == "git" {
        out.starts_with("git version ") && out[12..].starts_with(|c: char| c.is_ascii_digit())
    } else {
        let first = out.lines().next().unwrap_or("");
        let parts: Vec<_> = first.split('.').collect();
        parts.len() >= 3 && parts[..2].iter().all(|p| p.parse::<u32>().is_ok())
    };
    if !valid {
        return Err(format!("程序返回 0，但不是可识别的 {name} 版本：{out}"));
    }
    Ok(format!("{}\n{}", p.display(), out))
}
fn scratch(cwd: &Path) -> Result<PathBuf> {
    let p = cwd.join(format!(".handykit-probe-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&p).map_err(|e| e.to_string())?;
    Ok(p)
}
#[cfg(windows)]
fn writable_probe_tree(dir: &Path) -> std::io::Result<()> {
    for e in fs::read_dir(dir)? {
        let e = e?;
        let kind = e.file_type()?;
        if kind.is_dir() {
            writable_probe_tree(&e.path())?;
        } else if kind.is_file() {
            let mut permissions = e.metadata()?.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(e.path(), permissions)?;
            }
        }
    }
    Ok(())
}
fn local_result(dir: &Path, r: Result<String>) -> Result<String> {
    match r {
        Ok(s) => {
            #[cfg(windows)]
            writable_probe_tree(dir).map_err(|e| format!("自检通过，但清理权限失败：{e}"))?;
            fs::remove_dir_all(dir)
                .map_err(|e| format!("自检通过，但清理 {} 失败：{e}", dir.display()))?;
            Ok(s)
        }
        Err(e) => Err(format!("{e}；保留本轮自检目录：{}", dir.display())),
    }
}
fn text(p: &Path) -> Result<&str> {
    p.to_str().ok_or("路径编码无法传递给工具".into())
}
fn git_local(s: &Settings, cwd: &Path) -> Result<String> {
    let git = resolve(&s.git, "git")?;
    let dir = scratch(cwd)?;
    let result = (|| {
        fs::write(dir.join("empty-git-config"), []).map_err(|e| e.to_string())?;
        let call = |args: &[&str]| run(&git, args, &dir, true);
        call(&["init", "--bare", "remote.git"])?;
        call(&["init", "-b", "main", "seed"])?;
        for (k, v) in [
            ("user.name", "HandyKit Probe"),
            ("user.email", "probe@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "../no-hooks"),
        ] {
            call(&["-C", "seed", "config", k, v])?;
        }
        fs::write(dir.join("seed/probe.txt"), "base\n").map_err(|e| e.to_string())?;
        call(&["-C", "seed", "add", "probe.txt"])?;
        call(&["-C", "seed", "commit", "-m", "local probe"])?;
        call(&["-C", "seed", "remote", "add", "origin", "../remote.git"])?;
        call(&["-C", "seed", "push", "origin", "main"])?;
        call(&["clone", "-b", "main", "remote.git", "copy"])?;
        call(&["-C", "copy", "fetch", "origin"])?;
        for (k, v) in [
            ("user.name", "HandyKit Probe"),
            ("user.email", "probe@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", "../no-hooks"),
        ] {
            call(&["-C", "copy", "config", k, v])?;
        }
        call(&["-C", "copy", "checkout", "-b", "feature"])?;
        fs::write(dir.join("copy/feature.txt"), "local-only\n").map_err(|e| e.to_string())?;
        call(&["-C", "copy", "add", "feature.txt"])?;
        call(&["-C", "copy", "commit", "-m", "feature"])?;
        call(&["-C", "copy", "checkout", "main"])?;
        call(&[
            "-C",
            "copy",
            "merge",
            "--no-ff",
            "feature",
            "-m",
            "local merge",
        ])?;
        call(&["-C", "copy", "push", "origin", "main"])?;
        let a = call(&["-C", "copy", "rev-parse", "HEAD"])?;
        let b = call(&["--git-dir=remote.git", "rev-parse", "refs/heads/main"])?;
        if a != b {
            return Err("本地 push 后提交校验不一致".into());
        }
        Ok(format!(
            "临时本地仓库 clone、commit、merge、push 及提交一致性通过；未连接远端。HEAD={a}"
        ))
    })();
    local_result(&dir, result)
}
fn svn_local(s: &Settings, cwd: &Path) -> Result<String> {
    let svn = resolve(&s.svn, "svn")?;
    let admin = if !s.svnadmin.is_empty() {
        resolve(&s.svnadmin, "svnadmin")?
    } else {
        let sibling = svn.parent().unwrap().join(if cfg!(windows) {
            "svnadmin.exe"
        } else {
            "svnadmin"
        });
        if sibling.is_file() {
            sibling
        } else {
            resolve("", "svnadmin")?
        }
    };
    let dir = scratch(cwd)?;
    let result = (|| {
        run(&admin, &["create", "repository"], &dir, false)?;
        let repo = dir.join("repository");
        let url = url::Url::from_file_path(&repo)
            .map_err(|_| "无法将本地仓库路径转换为 file:// URL")?
            .to_string();
        let cfg = dir.join("config");
        fs::create_dir(&cfg).map_err(|e| e.to_string())?;
        let call = |args: &[&str]| {
            let mut args = args.to_vec();
            args.extend([
                "--non-interactive",
                "--no-auth-cache",
                "--config-dir",
                text(&cfg)?,
            ]);
            run(&svn, &args, &dir, false)
        };
        call(&["checkout", &url, "wc"])?;
        fs::write(dir.join("wc/probe.txt"), "local-only\n").map_err(|e| e.to_string())?;
        call(&["add", "wc/probe.txt"])?;
        call(&["commit", "wc", "-m", "HandyKit local probe"])?;
        let rev = call(&["info", "wc/probe.txt", "--show-item", "revision"])?;
        if rev != "1" {
            return Err(format!("预期本地 revision 1，实际 {rev}"));
        }
        let content = call(&["cat", &format!("{url}/probe.txt")])?;
        if content != "local-only" {
            return Err("SVN 提交后内容校验失败".into());
        }
        Ok("临时 file:// 仓库 checkout、add、commit、revision 及内容一致性通过；未连接真实服务器。".into())
    })();
    local_result(&dir, result)
}
#[cfg(test)]
mod tests;
