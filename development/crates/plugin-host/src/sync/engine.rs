use super::*;
use chrono::{Datelike, Timelike};
use process::{args, path, Runner};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
fn svn_args(c: &Config, password: &str, items: &[&str]) -> Vec<String> {
    let mut a = args(&[
        "--non-interactive",
        "--config-option",
        &format!("servers:global:http-timeout={}", c.svn_timeout),
    ]);
    if !c.svn_username.is_empty() {
        a.extend(args(&["--username", &c.svn_username]));
    }
    if !password.is_empty() {
        a.extend(args(&["--password-from-stdin", "--no-auth-cache"]));
    }
    a.extend(args(items));
    a
}
// Empty peg revision makes local paths containing @ unambiguous to SVN.
fn svn_target(path: &Path) -> String {
    format!("{}@", path.display())
}
pub fn preflight(
    c: &Config,
    password: &str,
    ctrl: &Control,
    external: Option<&AtomicBool>,
) -> Result<Preflight> {
    c.validate()?;
    if password.len() > 1024 || password.contains(['\n', '\r', '\0']) {
        return Err("SVN 口令过长或包含换行/控制字符".into());
    }
    let git = crate::resolve(&c.git, "git")?;
    let svn = crate::resolve(&c.svn, "svn")?;
    let runner = Runner {
        ctrl,
        external,
        timeout: c.command_timeout.min(30),
        password,
    };
    let cwd = Path::new(&c.workspace);
    let svn_dir = Path::new(&c.svn_dir);
    ctrl.log(&process::redact(
        &format!(
            "工作目录：{}；SVN 工作副本：{}",
            cwd.display(),
            svn_dir.display()
        ),
        password,
    ));
    let mut checks = Vec::new();
    let g = runner.call(&git, &args(&["--version"]), cwd, "Git 版本", false)?;
    if !g.starts_with("git version ") {
        return Err("所选程序未返回 Git 版本".into());
    }
    crate::record(
        &mut checks,
        "Git 版本",
        Ok(format!("{}\n{g}", git.display())),
    );
    let s = runner.call(
        &svn,
        &args(&["--version", "--quiet"]),
        cwd,
        "SVN 版本",
        false,
    )?;
    if !s.starts_with(|c: char| c.is_ascii_digit()) {
        return Err("所选程序未返回 SVN 版本".into());
    }
    crate::record(
        &mut checks,
        "SVN 版本",
        Ok(format!("{}\n{s}", svn.display())),
    );
    let root = runner
        .call(
            &svn,
            &svn_args(
                c,
                password,
                &["info", "--show-item", "wc-root", "--", &svn_target(svn_dir)],
            ),
            svn_dir,
            "检查 SVN 工作副本",
            !password.is_empty(),
        )
        .map_err(|e| {
            format!(
                "SVN 工作副本检查失败；程序：{}；目录：{}；{e}",
                svn.display(),
                svn_dir.display()
            )
        })?;
    crate::record(&mut checks, "SVN 工作副本", Ok(root));
    for r in &c.repositories {
        ctrl.check(external)?;
        let target = cwd.join(&r.directory);
        let archive = cwd.join(format!("{}.zip", r.directory));
        if target.try_exists().map_err(|e| e.to_string())?
            || archive.try_exists().map_err(|e| e.to_string())?
        {
            return Err(format!(
                "仓库 {} 的克隆目录或本地 ZIP 已存在；请先检查旧现场，不覆盖",
                r.name
            ));
        }
        runner.call(
            &git,
            &args(&["check-ref-format", "--branch", &r.branch]),
            cwd,
            "检查目标分支格式",
            false,
        )?;
        if !r.merge_branch.is_empty() {
            runner.call(
                &git,
                &args(&["check-ref-format", "--branch", &r.merge_branch]),
                cwd,
                "检查合并分支格式",
                false,
            )?;
        }
        let filename = r.filename();
        let dest = svn_dir.join(&filename);
        if fs::symlink_metadata(&dest).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("SVN 归档目标不能是符号链接".into());
        }
        let kind = runner.call(
            &svn,
            &svn_args(
                c,
                password,
                &["info", "--show-item", "kind", "--", &svn_target(&dest)],
            ),
            svn_dir,
            &format!("检查 SVN 归档是否已纳入版本：{filename}"),
            !password.is_empty(),
        )?;
        if kind != "file" {
            return Err(format!(
                "{filename} 必须是 SVN 已版本化文件；不会自动 svn add"
            ));
        }
        crate::record(
            &mut checks,
            &format!("仓库 {} 配置", r.name),
            Ok("分支格式、空闲输出路径及 SVN 目标已通过本地检查；未联网验证 Git 远端".into()),
        );
    }
    Ok(Preflight {
        passed: true,
        checks,
    })
}
fn own_remove(root: &Path) -> Result<()> {
    if fs::symlink_metadata(root)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("清理目标已变为链接，保留现场".into());
    }
    #[cfg(windows)]
    crate::writable_probe_tree(root).map_err(|e| e.to_string())?;
    fs::remove_dir_all(root).map_err(|e| e.to_string())
}
fn archive(
    c: &Config,
    r: &Repository,
    ctrl: &Control,
    external: Option<&AtomicBool>,
    run_id: &str,
) -> Result<PathBuf> {
    let base = Path::new(&c.workspace);
    let source = base.join(&r.directory);
    let final_path = base.join(format!("{}.zip", r.directory));
    let partial = base.join(format!("{}.zip.part-{run_id}", r.directory));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(9));
    let mut count = 0usize;
    fn visit(
        zip: &mut ZipWriter<fs::File>,
        path: &Path,
        name: &str,
        options: SimpleFileOptions,
        ctrl: &Control,
        external: Option<&AtomicBool>,
        count: &mut usize,
    ) -> Result<()> {
        ctrl.check(external)?;
        let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() {
            return Err("仓库内存在符号链接，归档已停止并保留现场；未跟随链接读取外部文件".into());
        }
        let options = if let Ok(modified) = meta.modified() {
            let time: chrono::DateTime<chrono::Local> = modified.into();
            let date = zip::DateTime::from_date_and_time(
                time.year().clamp(1980, 2107) as u16,
                time.month() as u8,
                time.day() as u8,
                time.hour() as u8,
                time.minute() as u8,
                time.second() as u8,
            )
            .unwrap_or_default();
            options.last_modified_time(date)
        } else {
            options
        };
        #[cfg(unix)]
        let options = {
            use std::os::unix::fs::PermissionsExt;
            options.unix_permissions(meta.permissions().mode())
        };
        if meta.is_dir() {
            zip.add_directory(format!("{name}/"), options)
                .map_err(|e| e.to_string())?;
            let mut entries = fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let file_name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "ZIP 中的文件名不是 UTF-8")?;
                visit(
                    zip,
                    &entry.path(),
                    &format!("{name}/{file_name}"),
                    options,
                    ctrl,
                    external,
                    count,
                )?;
            }
        } else if meta.is_file() {
            zip.start_file(name, options.large_file(meta.len() >= u32::MAX as u64))
                .map_err(|e| e.to_string())?;
            let mut input = fs::File::open(path).map_err(|e| e.to_string())?;
            let mut buffer = [0; 65536];
            loop {
                ctrl.check(external)?;
                let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            }
            *count += 1;
        } else {
            return Err("归档遇到非普通文件，已停止".into());
        }
        Ok(())
    }
    ctrl.log(&format!(
        "开始原生 ZIP 归档：{}（包含隐藏文件和 .git）",
        r.name
    ));
    visit(
        &mut zip,
        &source,
        &r.directory,
        options,
        ctrl,
        external,
        &mut count,
    )?;
    let file = zip.finish().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    ctrl.check(external)?;
    let mut input = fs::File::open(&partial).map_err(|e| e.to_string())?;
    let mut output = tempfile::NamedTempFile::new_in(final_path.parent().ok_or("归档父目录无效")?)
        .map_err(|e| e.to_string())?;
    let mut buffer = [0; 65536];
    loop {
        ctrl.check(external)?;
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())?;
    output.as_file().sync_all().map_err(|e| e.to_string())?;
    ctrl.check(external)?;
    output
        .persist_noclobber(&final_path)
        .map_err(|e| format!("拒绝覆盖已有 ZIP 或发布失败：{e}"))?;
    fs::remove_file(&partial).map_err(|e| e.to_string())?;
    ctrl.log(&format!("ZIP 完成：{}，{} 个文件", r.filename(), count));
    Ok(final_path)
}
fn update(ctrl: &Control, index: usize, f: impl FnOnce(&mut RepoResult)) {
    let mut state = ctrl.state.lock().unwrap();
    f(&mut state.repositories[index]);
}
pub fn execute(
    c: &Config,
    workflow: &Workflow,
    password: &str,
    ctrl: &Control,
    external: Option<&AtomicBool>,
) -> Result<()> {
    preflight(c, password, ctrl, external)?;
    ctrl.check(external)?;
    let git = crate::resolve(&c.git, "git")?;
    let svn = crate::resolve(&c.svn, "svn")?;
    let runner = Runner {
        ctrl,
        external,
        timeout: c.command_timeout,
        password,
    };
    let workspace = Path::new(&c.workspace);
    let run_id = ctrl.state.lock().unwrap().id.clone();
    let commit_message = format!(
        "源码同步 - {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    let mut archives = Vec::new();
    let mut failures = 0;
    for (index, r) in c.repositories.iter().enumerate() {
        ctrl.check(external)?;
        update(ctrl, index, |o| o.status = "running".into());
        let target = workspace.join(&r.directory);
        let result: Result<()> = (|| {
            for step in &workflow.per_repository {
                ctrl.check(external)?;
                match step {
                    Step::Clone => {
                        config::no_child_links(workspace, &r.directory)?;
                        if target.exists() {
                            return Err("克隆目标在预检后已出现，拒绝覆盖".into());
                        }
                        // Claim the final directory before calling git, preventing accidental reuse.
                        if let Some(parent) = target.parent() {
                            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        fs::create_dir(&target).map_err(|e| e.to_string())?;
                        runner.call(
                            &git,
                            &args(&[
                                "clone",
                                "--single-branch",
                                "--no-hardlinks",
                                "-b",
                                &r.branch,
                                "--",
                                &r.url,
                                &path(&target)?,
                            ]),
                            workspace,
                            &format!("Git clone：{}", r.name),
                            false,
                        )?;
                        if !c.git_name.is_empty() {
                            runner.call(
                                &git,
                                &args(&["config", "--local", "--", "user.name", &c.git_name]),
                                &target,
                                "设置本轮 Git 提交姓名",
                                false,
                            )?;
                            runner.call(
                                &git,
                                &args(&["config", "--local", "--", "user.email", &c.git_email]),
                                &target,
                                "设置本轮 Git 提交邮箱",
                                false,
                            )?;
                        }
                    }
                    Step::Merge => {
                        if !r.merge_branch.is_empty() {
                            runner.call(
                                &git,
                                &args(&[
                                    "fetch",
                                    "origin",
                                    &format!(
                                        "{}:refs/remotes/origin/{}",
                                        r.merge_branch, r.merge_branch
                                    ),
                                ]),
                                &target,
                                &format!("Git fetch：{}", r.name),
                                false,
                            )?;
                            runner.call(
                                &git,
                                &args(&[
                                    "merge",
                                    "-s",
                                    "recursive",
                                    "-X",
                                    "theirs",
                                    &format!("origin/{}", r.merge_branch),
                                    "--no-edit",
                                    "-q",
                                ]),
                                &target,
                                &format!("Git merge：{}", r.name),
                                false,
                            )?;
                        }
                    }
                    Step::Push => {
                        ctrl.check(external)?;
                        update(ctrl, index, |o| o.git_push = "unknown".into());
                        runner.call(
                            &git,
                            &args(&["push", "origin", &r.branch]),
                            &target,
                            &format!("Git push：{}", r.name),
                            false,
                        )?;
                        update(ctrl, index, |o| o.git_push = "confirmed".into());
                        let head = runner.call(
                            &git,
                            &args(&["rev-parse", "HEAD"]),
                            &target,
                            "记录 Git HEAD",
                            false,
                        )?;
                        update(ctrl, index, |o| o.git_commit = head);
                    }
                    Step::Archive => {
                        let file = archive(c, r, ctrl, external, &run_id)?;
                        update(ctrl, index, |o| {
                            o.archive = file.display().to_string();
                            o.status = "archived".into();
                        });
                        archives.push((index, file));
                    }
                    Step::Cleanup => {
                        own_remove(&target)?;
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            failures += 1;
            let e = process::redact(&e, password);
            ctrl.log(&format!("仓库 {} 失败：{e}；保留本轮现场", r.name));
            update(ctrl, index, |o| {
                o.status = "failed".into();
                o.message = e;
            });
        }
    }
    for (index, file) in archives {
        ctrl.check(external)?;
        let r = &c.repositories[index];
        let filename = r.filename();
        let dest = Path::new(&c.svn_dir).join(&filename);
        let result: Result<()> = (|| {
            if fs::symlink_metadata(&dest).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err("SVN 目标已变为链接，拒绝覆盖".into());
            }
            ctrl.log(&format!("复制归档到 SVN：{filename}"));
            let mut input = fs::File::open(&file).map_err(|e| e.to_string())?;
            let mut replacement = tempfile::NamedTempFile::new_in(Path::new(&c.svn_dir))
                .map_err(|e| e.to_string())?;
            let mut buffer = [0; 65536];
            loop {
                ctrl.check(external)?;
                let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                replacement
                    .write_all(&buffer[..n])
                    .map_err(|e| e.to_string())?;
            }
            replacement.flush().map_err(|e| e.to_string())?;
            replacement
                .as_file()
                .sync_all()
                .map_err(|e| e.to_string())?;
            ctrl.check(external)?;
            replacement
                .persist(&dest)
                .map_err(|e| format!("替换 SVN 归档失败：{e}"))?;
            ctrl.check(external)?;
            update(ctrl, index, |o| o.svn_commit = "unknown".into());
            runner.call(
                &svn,
                &svn_args(
                    c,
                    password,
                    &[
                        "commit",
                        "-m",
                        &commit_message,
                        "--force-log",
                        "--",
                        &svn_target(&dest),
                    ],
                ),
                Path::new(&c.svn_dir),
                &format!("SVN commit：{}", r.name),
                !password.is_empty(),
            )?;
            update(ctrl, index, |o| o.svn_commit = "confirmed".into());
            fs::remove_file(&file).map_err(|e| e.to_string())?;
            update(ctrl, index, |o| {
                if o.status != "failed" {
                    o.status = "succeeded".into();
                }
                if o.status == "failed" {
                    o.message
                        .push_str("；SVN commit 成功，本地 ZIP 已清理，但此前阶段仍有错误");
                } else {
                    o.message = "SVN commit 命令成功（可能无内容变更）；本地归档已清理".into();
                }
            });
            Ok(())
        })();
        if let Err(e) = result {
            failures += 1;
            let e = process::redact(&e, password);
            ctrl.log(&format!("SVN 阶段失败：{}；保留本地 ZIP", r.name));
            update(ctrl, index, |o| {
                o.status = "failed".into();
                o.message = e;
            });
        }
    }
    ctrl.check(external)?;
    if failures > 0 {
        Err(format!(
            "{failures} 个阶段失败；请检查每个仓库的 Git/SVN 写入状态，不自动重试"
        ))
    } else {
        Ok(())
    }
}
