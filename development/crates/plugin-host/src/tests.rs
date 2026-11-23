use super::*;
fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("handykit-plugin-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&p).unwrap();
    p
}
fn package() -> Vec<u8> {
    include_bytes!("../../../../plugins/code-sync-check/code-sync-check.hkp").to_vec()
}
#[test]
fn independent_install_disable_remove_and_api_validation() {
    let d = dir();
    let host = Host::open(d.join("plugins")).unwrap();
    let m = host.install(&package()).unwrap();
    assert!(host.install(&package()).is_err());
    assert_eq!(host.list().unwrap().len(), 1);
    host.toggle(&m.id, false).unwrap();
    assert!(host
        .run(
            &m.id,
            &Settings {
                workspace: d.display().to_string(),
                ..Default::default()
            }
        )
        .unwrap_err()
        .contains("停用"));
    host.toggle(&m.id, true).unwrap();
    host.uninstall(&m.id).unwrap();
    assert!(host.list().unwrap().is_empty());
    let mut json: serde_json::Value = serde_json::from_slice(&package()).unwrap();
    json["apiVersion"] = 1.into();
    assert!(host.install(&serde_json::to_vec(&json).unwrap()).is_err());
    json["apiVersion"] = 3.into();
    assert!(host.install(&serde_json::to_vec(&json).unwrap()).is_err());
    json["apiVersion"] = 2.into();
    json["id"] = "../outside".into();
    assert!(host.install(&serde_json::to_vec(&json).unwrap()).is_err());
    json["id"] = "other.plugin".into();
    json["operations"] = serde_json::json!(["run-shell"]);
    assert!(host.install(&serde_json::to_vec(&json).unwrap()).is_err());
    fs::remove_dir_all(d).unwrap();
}
#[test]
fn generic_packages_do_not_require_a_builtin_plugin_id() {
    let d = dir();
    let h = Host::open(d.join("plugins")).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&package()).unwrap();
    json["id"] = "different.plugin".into();
    json["operations"] = serde_json::json!(["workspace.read"]);
    h.install(&serde_json::to_vec(&json).unwrap()).unwrap();
    let file = d.join("沙箱 文件.txt");
    fs::write(&file, "read-only").unwrap();
    let report = h
        .run(
            "different.plugin",
            &Settings {
                workspace: d.display().to_string(),
                test_file: file.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(report.checks.len(), 2);
    assert!(report.checks.iter().all(|c| c.status == "passed"));
    drop(h);
    let h = Host::open(d.join("plugins")).unwrap();
    assert_eq!(h.list().unwrap().len(), 1);
    assert_eq!(fs::read_to_string(file).unwrap(), "read-only");
    fs::remove_dir_all(d).unwrap();
}
#[test]
fn invalid_executables_and_missing_files_are_not_reported_as_available() {
    let d = dir();
    let h = Host::open(d.join("plugins")).unwrap();
    let m = h.install(&package()).unwrap();
    let report = h
        .run(
            &m.id,
            &Settings {
                workspace: d.display().to_string(),
                git: d.join("missing-git").display().to_string(),
                svn: d.join("missing-svn").display().to_string(),
                test_file: d.join("missing-file").display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        report
            .checks
            .iter()
            .filter(|c| c.status == "failed")
            .count(),
        3
    );
    fs::remove_dir_all(d).unwrap();
}
#[test]
fn real_local_git_and_svn_roundtrip() {
    if resolve("", "git").is_err()
        || resolve("", "svn").is_err()
        || resolve("", "svnadmin").is_err()
    {
        assert!(
            std::env::var_os("HANDYKIT_REQUIRE_SCM_TESTS").is_none(),
            "Required Git/SVN tools missing"
        );
        eprintln!("Local SCM integration skipped: git/svn/svnadmin required");
        return;
    }
    let d = dir();
    let workspace = d.join("自检 含空格");
    fs::create_dir(&workspace).unwrap();
    let h = Host::open(d.join("plugins")).unwrap();
    let m = h.install(&package()).unwrap();
    let report = h
        .run(
            &m.id,
            &Settings {
                workspace: workspace.display().to_string(),
                local_roundtrip: true,
                ..Default::default()
            },
        )
        .unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    assert!(
        report.checks.iter().all(|c| c.status == "passed"),
        "{report:?}"
    );
    assert!(fs::read_dir(workspace).unwrap().next().is_none());
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn plugin_directory_has_one_owner() {
    let d = dir();
    let host = Host::open(d.join("plugins")).unwrap();
    assert!(Host::open(d.join("plugins")).is_err());
    drop(host);
    assert!(Host::open(d.join("plugins")).is_ok());
    fs::remove_dir_all(d).unwrap();
}

#[cfg(unix)]
#[test]
fn diagnostic_capture_keeps_draining_after_its_output_limit() {
    let d = dir();
    let output = run(
        Path::new("/bin/sh"),
        &[
            "-c",
            "i=0; while [ $i -lt 20000 ]; do printf abcdefghijklmnop; i=$((i+1)); done",
        ],
        &d,
        false,
    )
    .unwrap();
    assert_eq!(output.len(), 32768);
    assert!(output.starts_with("abcdefghijklmnop"));
    fs::remove_dir_all(d).unwrap();
}

#[test]
fn interrupted_installation_cannot_hide_valid_plugins_or_block_reinstallation() {
    let d = dir();
    let root = d.join("plugins");
    let host = Host::open(root.clone()).unwrap();
    let staging = root.join(".install-interrupted");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("manifest.json"), "incomplete").unwrap();
    assert!(host.list().unwrap().is_empty());
    let manifest = host.install(&package()).unwrap();
    assert_eq!(host.list().unwrap().len(), 1);
    host.uninstall(&manifest.id).unwrap();
    assert!(host.install(&package()).is_ok());
    drop(host);
    fs::remove_dir_all(d).unwrap();
}
