use super::*;
fn temp() -> PathBuf {
    let p = std::env::temp_dir().join(format!("handykit-native-sync-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&p).unwrap();
    p
}
fn plugin() -> Vec<u8> {
    include_bytes!("../../../../../plugins/code-sync/code-sync.hkp").to_vec()
}
fn config(root: &Path) -> Config {
    Config {
        workspace: root.join("工作 空间").display().to_string(),
        svn_dir: root.join("svn 中文 工作副本").display().to_string(),
        git_name: "Native Sync Test".into(),
        git_email: "native@example.invalid".into(),
        repositories: vec![Repository {
            name: "repository-one".into(),
            branch: "main".into(),
            merge_branch: "feature".into(),
            url: root.join("remote.git").display().to_string(),
            directory: "project".into(),
        }],
        ..Default::default()
    }
}
fn invoke(exe: &Path, root: &Path, values: &[&str]) -> String {
    crate::run(exe, values, root, true).unwrap()
}
fn setup(root: &Path) -> Config {
    let git = crate::resolve("", "git").unwrap();
    let svn = crate::resolve("", "svn").unwrap();
    let admin = crate::resolve("", "svnadmin").unwrap();
    fs::create_dir(root.join("工作 空间")).unwrap();
    invoke(&git, root, &["init", "--bare", "remote.git"]);
    invoke(&git, root, &["init", "-b", "main", "seed"]);
    for (k, v) in [
        ("user.name", "Native Sync Test"),
        ("user.email", "native@example.invalid"),
        ("commit.gpgsign", "false"),
    ] {
        invoke(&git, root, &["-C", "seed", "config", k, v]);
    }
    fs::write(root.join("seed/conflict.txt"), "base\n").unwrap();
    fs::write(root.join("seed/.hidden.txt"), "hidden\n").unwrap();
    invoke(&git, root, &["-C", "seed", "add", "."]);
    invoke(&git, root, &["-C", "seed", "commit", "-m", "base"]);
    invoke(&git, root, &["-C", "seed", "checkout", "-b", "feature"]);
    fs::write(root.join("seed/conflict.txt"), "theirs\n").unwrap();
    invoke(&git, root, &["-C", "seed", "commit", "-am", "feature"]);
    invoke(&git, root, &["-C", "seed", "checkout", "main"]);
    fs::write(root.join("seed/conflict.txt"), "ours\n").unwrap();
    invoke(&git, root, &["-C", "seed", "commit", "-am", "main"]);
    invoke(
        &git,
        root,
        &["-C", "seed", "remote", "add", "origin", "../remote.git"],
    );
    invoke(
        &git,
        root,
        &["-C", "seed", "push", "origin", "main", "feature"],
    );
    invoke(&admin, root, &["create", "svn-repo"]);
    let url = url::Url::from_file_path(root.join("svn-repo"))
        .unwrap()
        .to_string();
    invoke(
        &svn,
        root,
        &["checkout", &url, "svn 中文 工作副本", "--non-interactive"],
    );
    fs::write(root.join("svn 中文 工作副本/project.zip"), "placeholder").unwrap();
    invoke(&svn, root, &["add", "svn 中文 工作副本/project.zip"]);
    invoke(
        &svn,
        root,
        &[
            "commit",
            "svn 中文 工作副本",
            "-m",
            "initial tracked archive",
            "--non-interactive",
        ],
    );
    config(root)
}
fn wait(host: &Host, id: &str, run: &str) -> Run {
    let start = Instant::now();
    loop {
        let r = host.sync_detail(id, run).unwrap();
        if r.status != "running" {
            return r;
        }
        assert!(start.elapsed() < Duration::from_secs(40));
        std::thread::sleep(Duration::from_millis(30));
    }
}
#[test]
fn static_import_no_shell_execution_and_rejection_of_unsafe_paths() {
    let root = temp();
    fs::create_dir(root.join("工作 空间")).unwrap();
    fs::create_dir(root.join("svn 中文 工作副本")).unwrap();
    let c = config(&root);
    let dir = Path::new(&c.workspace);
    fs::write(dir.join("config.sh"),format!("\u{feff}# test\r\nsvn_dir='{}'\r\nsvn_username='test user'\r\nsvn_password='FAKE-PASSWORD-$-VALUE'\r\nsvn_timeout=42\r\n",c.svn_dir)).unwrap();
    fs::write(dir.join("repository.ini"),format!("\u{feff}[repository-one]\r\nurl={}\r\ndirectory=project\r\nmergebranch=feature\r\nbranch=main\r\n",c.repositories[0].url)).unwrap();
    let imported = config::import(dir).unwrap();
    assert_eq!(imported.password, "FAKE-PASSWORD-$-VALUE");
    assert_eq!(
        config::password_file(&imported.config.svn_credential_file).unwrap(),
        imported.password
    );
    assert_eq!(imported.config.svn_timeout, 42);
    assert_eq!(imported.config.repositories[0].branch, "main");
    let host = Host::open(root.join("plugins")).unwrap();
    let id = host.install(&plugin()).unwrap().id;
    let saved = host.sync_save(&id, 0, imported.config.clone()).unwrap();
    let stored = fs::read_to_string(host.data(&id).join("config.json")).unwrap();
    assert!(!stored.contains("FAKE-PASSWORD"));
    assert_eq!(
        host.sync_save(&id, 1, imported.config.clone())
            .unwrap()
            .revision,
        saved.revision
    );
    let mut unsafe_config = imported.config;
    unsafe_config.repositories[0].directory = "../outside".into();
    assert!(unsafe_config.validate().is_err());
    fs::write(
        dir.join("config.sh"),
        "svn_password=$(touch should-not-exist)\n",
    )
    .unwrap();
    assert!(config::import(dir).is_err());
    assert!(config::password_file(&dir.join("config.sh").display().to_string()).is_err());
    assert!(!dir.join("should-not-exist").exists());
    host.uninstall(&id).unwrap();
    assert!(host.list().unwrap().is_empty());
    host.install(&plugin()).unwrap();
    assert_eq!(host.sync_profile(&id).unwrap().revision, 1);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn native_sync_local_end_to_end() {
    if std::env::var_os("HANDYKIT_SYNC_ISOLATED_CHILD").is_none() {
        if crate::resolve("", "git").is_err()
            || crate::resolve("", "svn").is_err()
            || crate::resolve("", "svnadmin").is_err()
        {
            assert!(std::env::var_os("HANDYKIT_REQUIRE_SCM_TESTS").is_none());
            eprintln!("Native sync integration skipped: SCM tools missing");
            return;
        }
        let env = temp();
        fs::write(env.join("empty"), []).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "sync::tests::native_sync_local_end_to_end",
                "--nocapture",
            ])
            .env("HANDYKIT_SYNC_ISOLATED_CHILD", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", env.join("empty"))
            .env_remove("GIT_CONFIG_COUNT")
            .env("GIT_DIR", env.join("must-not-use-parent-git-dir"))
            .env("GIT_WORK_TREE", env.join("must-not-use-parent-work-tree"))
            .status()
            .unwrap();
        assert!(status.success());
        fs::remove_dir_all(env).unwrap();
        return;
    }
    let root = temp();
    let c = setup(&root);
    let host = Arc::new(Host::open(root.join("plugins")).unwrap());
    let id = host.install(&plugin()).unwrap().id;
    let p = host.sync_save(&id, 0, c.clone()).unwrap();
    assert!(host.sync_check(&id, &c, "").unwrap().passed);
    let mut wrong_wc = c.clone();
    wrong_wc.svn_dir = c.workspace.clone();
    let error = host.sync_check(&id, &wrong_wc, "").unwrap_err();
    assert!(error.contains("E155007"), "{error}");
    assert!(error.contains("SVN 工作副本检查失败；程序："), "{error}");
    assert!(error.contains(&c.workspace), "{error}");
    let git = crate::resolve("", "git").unwrap();
    let before = invoke(
        &git,
        &root,
        &["--git-dir=remote.git", "rev-parse", "refs/heads/main"],
    );
    let run_id = host
        .sync_start(&id, p.revision, "FAKE-SESSION-PASS".into())
        .unwrap();
    let run = wait(&host, &id, &run_id);
    println!("{}", serde_json::to_string_pretty(&run).unwrap());
    assert_eq!(run.status, "succeeded", "{}", run.message);
    assert!(!serde_json::to_string(&run)
        .unwrap()
        .contains("FAKE-SESSION-PASS"));
    assert!(run
        .logs
        .iter()
        .any(|line| line.contains("SVN 工作副本：") && line.contains("svn 中文 工作副本")));
    assert_eq!(run.repositories[0].git_push, "confirmed");
    assert_eq!(run.repositories[0].svn_commit, "confirmed");
    let after = invoke(
        &git,
        &root,
        &["--git-dir=remote.git", "rev-parse", "refs/heads/main"],
    );
    assert_ne!(before, after);
    let mut zip =
        zip::ZipArchive::new(fs::File::open(root.join("svn 中文 工作副本/project.zip")).unwrap())
            .unwrap();
    let mut text = String::new();
    zip.by_name("project/conflict.txt")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "theirs\n");
    assert!(zip.by_name("project/.hidden.txt").is_ok());
    assert!(zip.by_name("project/.git/HEAD").is_ok());
    drop(zip);
    assert!(!Path::new(&c.workspace).join("project").exists());
    assert!(!Path::new(&c.workspace).join("project.zip").exists());
    let svn = crate::resolve("", "svn").unwrap();
    assert_eq!(
        invoke(
            &svn,
            &root,
            &[
                "info",
                "--show-item",
                "last-changed-revision",
                "svn 中文 工作副本/project.zip"
            ]
        ),
        "2"
    );
    assert_eq!(host.sync_history(&id).unwrap()[0].id, run_id);
    // A failed preflight must not push or touch an existing local archive.
    fs::write(Path::new(&c.workspace).join("project.zip"), "keep-old").unwrap();
    let failed_id = host.sync_start(&id, p.revision, String::new()).unwrap();
    let failed = wait(&host, &id, &failed_id);
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.repositories[0].git_push, "not_started");
    assert_eq!(
        fs::read_to_string(Path::new(&c.workspace).join("project.zip")).unwrap(),
        "keep-old"
    );
    assert_eq!(
        invoke(
            &git,
            &root,
            &["--git-dir=remote.git", "rev-parse", "refs/heads/main"]
        ),
        after
    );
    fs::remove_file(Path::new(&c.workspace).join("project.zip")).unwrap();
    let mut changed = c.clone();
    changed.svn_timeout += 1;
    host.sync_save(&id, p.revision, changed).unwrap();
    assert!(host.sync_start(&id, p.revision, String::new()).is_err());
    #[cfg(unix)]
    {
        // Reject only the synthetic SVN server commit. Git push must be recorded
        // separately and the local ZIP retained for inspection, never auto-retried.
        use std::os::unix::fs::PermissionsExt;
        let hook = root.join("svn-repo/hooks/pre-commit");
        fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
        let current = host.sync_profile(&id).unwrap();
        invoke(&git, &root, &["-C", "seed", "checkout", "feature"]);
        fs::write(root.join("seed/new-file.txt"), "second change\n").unwrap();
        invoke(&git, &root, &["-C", "seed", "add", "new-file.txt"]);
        invoke(
            &git,
            &root,
            &["-C", "seed", "commit", "-m", "second feature"],
        );
        invoke(&git, &root, &["-C", "seed", "push", "origin", "feature"]);
        let rejected = host
            .sync_start(&id, current.revision, String::new())
            .unwrap();
        let rejected = wait(&host, &id, &rejected);
        assert_eq!(rejected.status, "failed");
        assert_eq!(rejected.repositories[0].git_push, "confirmed");
        assert_eq!(rejected.repositories[0].svn_commit, "unknown");
        assert!(Path::new(&c.workspace).join("project.zip").is_file());
        assert_eq!(
            invoke(
                &svn,
                &root,
                &[
                    "info",
                    "--show-item",
                    "last-changed-revision",
                    "svn 中文 工作副本/project.zip"
                ]
            ),
            "2"
        );
    }
    #[cfg(unix)]
    {
        let _ = fs::remove_file(root.join("svn-repo/hooks/pre-commit"));
        let alternate_workspace = root.join("插件变体 工作区");
        fs::create_dir(&alternate_workspace).unwrap();
        fs::write(root.join("svn 中文 工作副本/variant.zip"), "placeholder").unwrap();
        invoke(&svn, &root, &["add", "svn 中文 工作副本/variant.zip"]);
        invoke(
            &svn,
            &root,
            &[
                "commit",
                "svn 中文 工作副本/variant.zip",
                "-m",
                "track variant archive",
                "--non-interactive",
            ],
        );
        let mut manifest: serde_json::Value = serde_json::from_slice(&plugin()).unwrap();
        manifest["id"] = "test.package-workflow".into();
        manifest["workflow"]["perRepository"] = serde_json::json!(["git.clone", "zip.archive"]);
        let alternate_id = host
            .install(&serde_json::to_vec(&manifest).unwrap())
            .unwrap()
            .id;
        let mut alternate = c.clone();
        alternate.workspace = alternate_workspace.display().to_string();
        alternate.repositories[0].directory = "variant".into();
        let profile = host.sync_save(&alternate_id, 0, alternate.clone()).unwrap();
        let before = invoke(
            &git,
            &root,
            &["--git-dir=remote.git", "rev-parse", "refs/heads/main"],
        );
        let job = host
            .sync_start(&alternate_id, profile.revision, String::new())
            .unwrap();
        let result = wait(&host, &alternate_id, &job);
        assert_eq!(result.status, "succeeded", "{}", result.message);
        assert_eq!(result.repositories[0].git_push, "not_requested");
        assert!(
            alternate_workspace.join("variant").is_dir(),
            "package omitted cleanup step"
        );
        assert!(!result.logs.iter().any(|l| l.contains("开始：Git push")));
        assert_eq!(
            invoke(
                &git,
                &root,
                &["--git-dir=remote.git", "rev-parse", "refs/heads/main"]
            ),
            before
        );
        assert_eq!(
            invoke(
                &svn,
                &root,
                &[
                    "info",
                    "--show-item",
                    "last-changed-revision",
                    "svn 中文 工作副本/project.zip"
                ]
            ),
            "2",
            "commit must not include unrelated modified project.zip"
        );
    }
    drop(host);
    #[cfg(windows)]
    crate::writable_probe_tree(&root).unwrap();
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn output_redacts_credentials_and_repo_paths_are_unambiguous() {
    let value = process::redact(
        "FAKE-PASS https://u:p@example.invalid/r?token=FAKE-TOKEN\npassword=OTHER",
        "FAKE-PASS",
    );
    assert!(!value.contains("FAKE-PASS"));
    assert!(!value.contains("u:p"));
    assert!(!value.contains("FAKE-TOKEN"));
    assert!(!value.contains("OTHER"));
}
#[cfg(unix)]
#[test]
fn cancellation_terminates_command_group() {
    let root = temp();
    let c = config(&root);
    let h = Host::open(root.join("plugins")).unwrap();
    let ctrl = h.make_control("test.sync", 1, &c, None);
    let copy = ctrl.clone();
    let cwd = root.clone();
    let start = Instant::now();
    let job = std::thread::spawn(move || {
        process::Runner {
            ctrl: &copy,
            external: None,
            timeout: 30,
            password: "",
        }
        .call(
            Path::new("/bin/sh"),
            &process::args(&["-c", "sleep 30 & wait"]),
            &cwd,
            "test-only wait",
            false,
        )
    });
    std::thread::sleep(Duration::from_millis(250));
    ctrl.cancel.store(true, Ordering::SeqCst);
    assert!(job.join().unwrap().is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_workflow_order_is_rejected_before_installation() {
    let mut package: serde_json::Value = serde_json::from_slice(&plugin()).unwrap();
    package["workflow"]["perRepository"] =
        serde_json::json!(["git.clone", "fs.cleanup-clone", "zip.archive"]);
    assert!(crate::Manifest::parse(&serde_json::to_vec(&package).unwrap()).is_err());
    package["workflow"]["perRepository"] =
        serde_json::json!(["git.clone", "zip.archive", "run-shell"]);
    assert!(crate::Manifest::parse(&serde_json::to_vec(&package).unwrap()).is_err());
}

#[cfg(windows)]
#[test]
fn windows_cancellation_stops_live_process_tree() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };
    let root = temp();
    let script = root.join("wait.ps1");
    fs::write(&script, r#"$ErrorActionPreference='Stop'
$p = Start-Process -FilePath (Join-Path $PSHOME 'powershell.exe') -ArgumentList '-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 30' -PassThru
[IO.File]::WriteAllText((Join-Path (Get-Location) 'child.pid'), [string]$p.Id)
Start-Sleep -Seconds 30
"#).unwrap();
    let ps = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let h = Host::open(root.join("plugins")).unwrap();
    let ctrl = h.make_control("test.sync", 1, &config(&root), None);
    let copy = ctrl.clone();
    let cwd = root.clone();
    let worker = std::thread::spawn(move || {
        process::Runner {
            ctrl: &copy,
            external: None,
            timeout: 15,
            password: "",
        }
        .call(
            &ps,
            &process::args(&[
                "-NoProfile",
                "-NonInteractive",
                "-File",
                script.to_str().unwrap(),
            ]),
            &cwd,
            "test-only process tree",
            false,
        )
    });
    let ready = Instant::now();
    while !root.join("child.pid").exists() && ready.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    // Retain the actual descendant process object before cancellation, rather
    // than querying a PID that could have been reused after termination.
    let pid = fs::read_to_string(root.join("child.pid"))
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    assert!(!raw.is_null());
    use std::os::windows::io::FromRawHandle;
    let descendant = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw as _) };
    let start = Instant::now();
    ctrl.cancel.store(true, Ordering::SeqCst);
    assert!(worker.join().unwrap().is_err());
    assert!(start.elapsed() < Duration::from_secs(6));
    assert_eq!(
        unsafe { WaitForSingleObject(descendant.as_raw_handle() as _, 3000) },
        windows_sys::Win32::Foundation::WAIT_OBJECT_0
    );
    assert!(!ctrl
        .state
        .lock()
        .unwrap()
        .logs
        .iter()
        .any(|line| line.contains("Windows 进程树停止未确认")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn conflicting_clone_and_archive_paths_are_rejected() {
    let root = temp();
    fs::create_dir(root.join("工作 空间")).unwrap();
    fs::create_dir(root.join("svn-wc")).unwrap();
    let mut c = config(&root);
    fs::create_dir_all(&c.svn_dir).unwrap();
    c.repositories[0].directory = "a".into();
    let mut other = c.repositories[0].clone();
    other.name = "other".into();
    other.directory = "a.zip/sub".into();
    c.repositories.push(other);
    assert!(c.validate().unwrap_err().contains("输出路径冲突"));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn history_reads_are_pure_and_recovery_happens_once_at_startup() {
    let root = temp();
    let host = Host::open(root.join("plugins")).unwrap();
    let id = host.install(&plugin()).unwrap().id;
    let ctrl = host.make_control(&id, 1, &Config::default(), None);
    let state = ctrl.state.lock().unwrap().clone();
    let runs = host.data(&id).join("runs");
    fs::create_dir_all(&runs).unwrap();
    let path = runs.join(format!("{}.json", state.id));
    fs::write(&path, serde_json::to_vec(&state).unwrap()).unwrap();
    let original = fs::read(&path).unwrap();
    assert_eq!(
        host.sync_detail(&id, &state.id).unwrap().status,
        "interrupted"
    );
    host.sync_history(&id).unwrap();
    assert_eq!(
        fs::read(&path).unwrap(),
        original,
        "reads must not rewrite active records"
    );
    drop(host);
    let host = Host::open(root.join("plugins")).unwrap();
    assert_eq!(
        host.sync_detail(&id, &state.id).unwrap().status,
        "interrupted"
    );
    let recovered = fs::read(&path).unwrap();
    host.sync_history(&id).unwrap();
    assert_eq!(fs::read(&path).unwrap(), recovered);
    drop(host);
    fs::remove_dir_all(root).unwrap();
}
