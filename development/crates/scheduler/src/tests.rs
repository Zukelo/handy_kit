use super::*;
use std::{
    thread::sleep,
    time::{Duration, Instant},
};
struct Fixture {
    dir: PathBuf,
    service: Scheduler,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("handykit-test-{}", uuid::Uuid::new_v4()));
        let service = Scheduler::open(&dir).unwrap();
        Self { dir, service }
    }
    #[cfg(unix)]
    fn task(&self, script: &str) -> Task {
        let path = self.dir.join("test script.sh");
        std::fs::write(&path, script).unwrap();
        Task {
            id: String::new(),
            revision: 0,
            name: "测试任务".into(),
            description: "".into(),
            enabled: false,
            cron: "* * * * *".into(),
            zone: "UTC".into(),
            action: Action {
                kind: "script".into(),
                target: path.to_string_lossy().into(),
                interpreter: "/bin/sh".into(),
                args: vec![],
                env: vec![],
                cwd: "".into(),
                instance: "reuse".into(),
            },
            timeout: None,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.service.shutdown(true);
        let end = Instant::now() + Duration::from_secs(5);
        while self.service.running_count() > 0 && Instant::now() < end {
            sleep(Duration::from_millis(10));
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
fn done(s: &Scheduler, id: &str) -> Run {
    let start = Instant::now();
    loop {
        let r = s.run_detail(id).unwrap();
        if r.status != "running" {
            return r;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "execution hung");
        sleep(Duration::from_millis(20));
    }
}
#[test]
fn exclusive_owner_and_checksum() {
    let f = Fixture::new();
    assert!(Scheduler::open(&f.dir).is_err());
    let inner = f.service.0.inner.lock().unwrap();
    inner
        .db
        .execute(
            "UPDATE schema_migrations SET checksum='changed' WHERE version='0001_initial.sql'",
            [],
        )
        .unwrap();
    let p = f.dir.join("separate.sqlite3");
    let mut db = Connection::open(p.clone()).unwrap();
    db.execute_batch("CREATE TABLE schema_migrations(version TEXT PRIMARY KEY,checksum TEXT,applied_at TEXT);INSERT INTO schema_migrations VALUES('0001_initial.sql','changed','');").unwrap();
    assert!(storage::migrate(&mut db, &p).is_err());
}
#[test]
fn migration_preserves_legacy_and_backs_up() {
    let dir = std::env::temp_dir().join(format!("handykit-migration-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("handykit.sqlite3");
    let mut db = Connection::open(&path).unwrap();
    let sql = include_str!("../../../../database/migrations/0001_initial.sql");
    db.execute_batch(sql).unwrap();
    db.execute_batch("CREATE TABLE schema_migrations(version TEXT PRIMARY KEY,checksum TEXT,applied_at TEXT DEFAULT CURRENT_TIMESTAMP);INSERT INTO scheduled_tasks(id,name,cron_expression,timezone,created_at,updated_at) VALUES('legacy','reminder','0 9 * * *','UTC','now','now');").unwrap();
    use sha2::{Digest, Sha256};
    db.execute(
        "INSERT INTO schema_migrations(version,checksum) VALUES('0001_initial.sql',?)",
        [format!("{:x}", Sha256::digest(sql))],
    )
    .unwrap();
    storage::migrate(&mut db, &path).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM scheduled_tasks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert!(std::fs::read_dir(&dir).unwrap().any(|p| p
        .unwrap()
        .path()
        .extension()
        .is_some_and(|x| x == "backup")));
    drop(db);
    let _ = std::fs::remove_dir_all(dir);
}
#[test]
#[cfg(unix)]
fn ordered_args_environment_snapshot_and_recovery() {
    let f = Fixture::new();
    let mut t = f.task("printf '<%s>\\n' \"$@\"; printf '%s' \"$HK_VALUE\" >&2\n");
    t.action.args = vec![
        Argument {
            value: "中文 空格".into(),
            secret: false,
        },
        Argument {
            value: "".into(),
            secret: false,
        },
        Argument {
            value: "\"' $HOME ; touch impossible".into(),
            secret: true,
        },
    ];
    t.action.env = vec![Environment {
        key: "HK_VALUE".into(),
        value: "真实输出".into(),
        secret: true,
    }];
    let saved = f.service.save(t).unwrap();
    let run = done(&f.service, &f.service.run_now(&saved.id).unwrap());
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.exit_code, Some(0));
    assert_eq!(
        run.stdout,
        "<中文 空格>\n<>\n<\"' $HOME ; touch impossible>\n"
    );
    assert_eq!(run.stderr, "真实输出");
    assert_eq!(run.snapshot.action.args[2].value, "〈敏感参数〉");
    assert_eq!(run.snapshot.action.env[0].value, "〈敏感值〉");
    let mut stale = saved.clone();
    stale.name = "new".into();
    let new = f.service.save(stale.clone()).unwrap();
    assert_eq!(new.revision, 2);
    assert!(f.service.save(stale).is_err());
    f.service.delete(&saved.id).unwrap();
    assert!(f.service.overview().unwrap().tasks.is_empty());
    assert_eq!(
        f.service.run_detail(&run.id).unwrap().snapshot.name,
        "测试任务"
    );
}
#[test]
#[cfg(unix)]
fn overlap_cancel_timeout_and_child_group() {
    let f = Fixture::new();
    let mut t = f.task("sleep 30 &\nwait\n");
    let saved = f.service.save(t.clone()).unwrap();
    let first = f.service.run_now(&saved.id).unwrap();
    let second = f.service.run_now(&saved.id).unwrap();
    assert_eq!(done(&f.service, &second).status, "skipped");
    f.service.cancel(&first).unwrap();
    assert_eq!(done(&f.service, &first).status, "cancelled");
    t.timeout = Some(1);
    let saved = f.service.save(t).unwrap();
    let run = done(&f.service, &f.service.run_now(&saved.id).unwrap());
    assert_eq!(run.status, "timed_out");
    assert_eq!(run.exit_code, None);
    assert_eq!(f.service.running_count(), 0);
}
#[test]
#[cfg(unix)]
fn actual_scheduled_tick_deduplicates_and_pause_blocks() {
    let f = Fixture::new();
    let mut t = f.task("printf scheduled");
    t.enabled = true;
    let saved = f.service.save(t).unwrap();
    let next = Utc
        .timestamp_opt((Utc::now().timestamp() / 60 + 1) * 60, 0)
        .unwrap();
    f.service.tick(next).unwrap();
    f.service.tick(next).unwrap();
    let history = f.service.history(Some(&saved.id), 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(done(&f.service, &history[0].id).stdout, "scheduled");
    f.service.toggle(&saved.id, false).unwrap();
    f.service.tick(next + chrono::Duration::minutes(1)).unwrap();
    assert_eq!(f.service.history(Some(&saved.id), 0).unwrap().len(), 1);
}
#[test]
#[cfg(unix)]
fn missing_paths_and_deleted_config_do_not_execute() {
    let f = Fixture::new();
    let saved = f.service.save(f.task("printf ok")).unwrap();
    std::fs::remove_file(&saved.action.target).unwrap();
    assert!(f.service.overview().unwrap().tasks[0].issue.is_some());
    assert!(f.service.toggle(&saved.id, true).is_err());
    assert!(!f.service.toggle(&saved.id, false).unwrap().enabled);
    let run = done(&f.service, &f.service.run_now(&saved.id).unwrap());
    assert_eq!(run.status, "failed");
    assert!(run.message.contains("文件不存在"));
    assert_eq!(run.exit_code, None);
}
#[test]
#[cfg(unix)]
fn bounded_output_exit_and_crash_recovery() {
    let f = Fixture::new();
    let t = f
        .service
        .save(f.task("head -c 11000000 /dev/zero; printf error >&2; exit 7"))
        .unwrap();
    let run = done(&f.service, &f.service.run_now(&t.id).unwrap());
    assert_eq!(run.status, "failed");
    assert_eq!(run.exit_code, Some(7));
    assert!(run.truncated);
    assert!(run.stdout.len() + run.stderr.len() <= 10 * 1024 * 1024);
    let mut inner = f.service.0.inner.lock().unwrap();
    let id = storage::reserve(
        &inner.db,
        &t,
        "manual",
        &Utc::now().to_rfc3339(),
        "running",
        "test crash",
    )
    .unwrap()
    .unwrap();
    let p = f.dir.join("handykit.sqlite3");
    storage::migrate(&mut inner.db, &p).unwrap();
    let restored = storage::runs(&inner.db, None, Some(&id), 0).unwrap();
    assert_eq!(restored[0].status, "interrupted");
    assert!(restored[0].message.contains("不会自动重跑"));
}
#[test]
#[cfg(unix)]
fn concurrency_limit_and_shutdown() {
    let f = Fixture::new();
    let mut ids = vec![];
    for n in 0..4 {
        let mut t = f.task("sleep 30 & wait");
        t.name = format!("t{n}");
        let t = f.service.save(t).unwrap();
        ids.push(f.service.run_now(&t.id).unwrap());
    }
    assert_eq!(f.service.running_count(), 3);
    assert_eq!(done(&f.service, &ids[3]).status, "skipped");
    f.service.shutdown(true);
    assert!(f
        .service
        .run_now(&f.service.history(None, 0).unwrap()[0].task_id)
        .is_err());
    for id in &ids[..3] {
        assert_eq!(done(&f.service, id).status, "cancelled");
    }
}

#[test]
#[cfg(unix)]
fn missed_grace_clock_back_and_restart_restore() {
    let f = Fixture::new();
    let mut t = f.task("printf one");
    t.enabled = true;
    let t = f.service.save(t).unwrap();
    let next = Utc
        .timestamp_opt((Utc::now().timestamp() / 60 + 1) * 60, 0)
        .unwrap();
    f.service
        .tick(next + chrono::Duration::seconds(31))
        .unwrap();
    let rows = f.service.history(Some(&t.id), 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "skipped");
    assert!(rows[0].message.contains("30 秒"));
    f.service.tick(next - chrono::Duration::minutes(1)).unwrap();
    assert_eq!(f.service.history(Some(&t.id), 0).unwrap().len(), 1);
    f.service.tick(next + chrono::Duration::minutes(5)).unwrap();
    let rows = f.service.history(Some(&t.id), 0).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(done(&f.service, &rows[0].id).stdout, "one");
    assert!(f.service.overview().unwrap().notice.contains("遗漏"));
    // A fresh independent service reads persisted definitions and completed history.
    let dir = f.dir.clone();
    drop(f.service.clone());
    let db = Connection::open(dir.join("handykit.sqlite3")).unwrap();
    assert_eq!(storage::task(&db, &t.id).unwrap().cron, "* * * * *");
    assert_eq!(storage::runs(&db, Some(&t.id), None, 0).unwrap().len(), 2);
}

#[test]
#[cfg(windows)]
fn windows_process_fixture() {
    match std::env::var("HK_WINDOWS_FIXTURE").as_deref() {
        Ok("leaf") => sleep(Duration::from_secs(30)),
        Ok("tree") => {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tests::windows_process_fixture", "--nocapture"])
                .env("HK_WINDOWS_FIXTURE", "leaf")
                .spawn()
                .unwrap();
            let _ = child.wait();
        }
        _ => {}
    }
}
#[test]
#[cfg(windows)]
fn windows_native_exe_and_job_timeout() {
    let f = Fixture::new();
    let mut t = Task {
        id: String::new(),
        revision: 0,
        name: "Windows exe".into(),
        description: String::new(),
        enabled: false,
        cron: "* * * * *".into(),
        zone: "UTC".into(),
        action: Action {
            kind: "executable".into(),
            target: std::env::current_exe().unwrap().to_string_lossy().into(),
            interpreter: String::new(),
            args: vec![Argument {
                value: "--list".into(),
                secret: false,
            }],
            env: vec![],
            cwd: String::new(),
            instance: "reuse".into(),
        },
        timeout: None,
    };
    let saved = f.service.save(t.clone()).unwrap();
    let run = done(&f.service, &f.service.run_now(&saved.id).unwrap());
    assert_eq!(run.status, "succeeded");
    assert_eq!(run.exit_code, Some(0));
    assert!(run.stdout.contains("windows_native_exe_and_job_timeout"));
    t.timeout = Some(1);
    t.action.args = ["--exact", "tests::windows_process_fixture", "--nocapture"]
        .iter()
        .map(|v| Argument {
            value: v.to_string(),
            secret: false,
        })
        .collect();
    t.action.env.push(Environment {
        key: "HK_WINDOWS_FIXTURE".into(),
        value: "tree".into(),
        secret: false,
    });
    let t = f.service.save(t).unwrap();
    let r = done(&f.service, &f.service.run_now(&t.id).unwrap());
    assert_eq!(r.status, "timed_out");
    assert_eq!(f.service.running_count(), 0);
}

#[test]
#[cfg(windows)]
fn git_bash_launcher_runs_headless_and_preserves_failure_output() {
    let root = std::env::var_os("HANDYKIT_TEST_GIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git"));
    if !root.join("git-bash.exe").is_file() {
        assert!(
            std::env::var_os("GITHUB_ACTIONS").is_none(),
            "CI must provide Git for Windows"
        );
        eprintln!("Git integration requires Git for Windows; skipped on this local machine");
        return;
    }
    let f = Fixture::new();
    let dir = f.dir.join("脚本 含空格");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("failure test.sh");
    std::fs::write(&script,"printf '<%s>\\n' \"$@\"\nprintf 'stderr-visible' >&2\nprintf 'tool='\nbasename /a/tool\nexit 23\n").unwrap();
    let task = Task {
        id: String::new(),
        revision: 0,
        name: "Git Bash failure".into(),
        description: String::new(),
        enabled: false,
        cron: "* * * * *".into(),
        zone: "UTC".into(),
        action: Action {
            kind: "script".into(),
            target: script.to_string_lossy().into(),
            interpreter: root.join("git-bash.exe").to_string_lossy().into(),
            args: vec![
                Argument {
                    value: "中文 空格".into(),
                    secret: false,
                },
                Argument {
                    value: String::new(),
                    secret: false,
                },
                Argument {
                    value: "\"quoted\" $HOME".into(),
                    secret: false,
                },
            ],
            env: vec![],
            cwd: String::new(),
            instance: "reuse".into(),
        },
        timeout: Some(5),
    };
    let saved = f.service.save(task.clone()).unwrap();
    let run = done(&f.service, &f.service.run_now(&saved.id).unwrap());
    assert_eq!(run.status, "failed", "{} / {}", run.message, run.stderr);
    assert_eq!(run.exit_code, Some(23));
    assert!(
        run.stdout.contains("<中文 空格>\n<>\n<\"quoted\" $HOME>\n"),
        "{:?}",
        run.stdout
    );
    assert!(run.stdout.contains("tool=tool"));
    assert_eq!(run.stderr, "stderr-visible");
    let read_again = f.service.run_detail(&run.id).unwrap();
    assert_eq!(read_again.stderr, "stderr-visible");
    assert_eq!(read_again.exit_code, Some(23));
    let mut task = task;
    task.action.interpreter = root.join("bin/bash.exe").to_string_lossy().into();
    let saved = f.service.save(task).unwrap();
    assert_eq!(
        done(&f.service, &f.service.run_now(&saved.id).unwrap()).exit_code,
        Some(23)
    );
}

#[test]
#[cfg(windows)]
fn git_bash_d_drive_script_path() {
    let root = std::env::var_os("HANDYKIT_TEST_GIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git"));
    if !root.join("usr/bin/bash.exe").is_file() || !Path::new(r"D:\").is_dir() {
        assert!(
            std::env::var_os("GITHUB_ACTIONS").is_none(),
            "CI requires Git and D drive"
        );
        return;
    }
    let f = Fixture::new();
    let dir = PathBuf::from(r"D:\").join(format!("handykit-path-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("sync code-merge.sh");
    std::fs::write(
        &script,
        "test -f \"$0\" || exit 91; test \"${BASH_SOURCE[0]}\" = \"$0\" || exit 92; printf 'd-drive-ok'; printf 'retained-error' >&2; exit 17\n",
    )
    .unwrap();
    let task = Task {
        id: String::new(),
        revision: 0,
        name: "D drive script".into(),
        description: String::new(),
        enabled: false,
        cron: "* * * * *".into(),
        zone: "UTC".into(),
        action: Action {
            kind: "script".into(),
            target: script.to_string_lossy().into(),
            interpreter: root.join("git-bash.exe").to_string_lossy().into(),
            args: vec![],
            env: vec![],
            cwd: String::new(),
            instance: "reuse".into(),
        },
        timeout: Some(5),
    };
    let task = f.service.save(task).unwrap();
    let run = done(&f.service, &f.service.run_now(&task.id).unwrap());
    assert_eq!(run.exit_code, Some(17), "{} / {}", run.message, run.stderr);
    assert_eq!(run.stdout, "d-drive-ok");
    assert_eq!(run.stderr, "retained-error");
    assert!(
        run.message.contains("./sync code-merge.sh"),
        "{}",
        run.message
    );
    assert!(run.message.contains("Windows 读取检查：通过"));
    let mut custom = task.clone();
    custom.action.cwd = f.dir.to_string_lossy().into();
    let custom = f.service.save(custom).unwrap();
    let run = done(&f.service, &f.service.run_now(&custom.id).unwrap());
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(run.exit_code, Some(17), "{} / {}", run.message, run.stderr);
    assert!(run.message.contains("/d/"), "{}", run.message);
}
