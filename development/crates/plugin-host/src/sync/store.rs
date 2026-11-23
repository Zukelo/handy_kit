use super::*;
#[derive(Clone, Serialize, Deserialize)]
pub struct Profile {
    pub revision: u64,
    pub config: Config,
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("配置或记录超过 2 MiB".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().ok_or("记录目录无效")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
    }
    serde_json::to_writer_pretty(&mut file, value).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
impl Host {
    pub(super) fn data(&self, id: &str) -> PathBuf {
        self.root.join("_data").join(id)
    }
    fn sync_manifest(&self, id: &str) -> Result<Workflow> {
        let m = self.manifest(id)?;
        if !m.operations.contains(&Operation::CodeSync) {
            return Err("该插件未声明原生同步能力".into());
        }
        if !self.root.join(id).join("enabled").is_file() {
            return Err("插件已停用".into());
        }
        m.workflow.ok_or("同步插件工作流缺失".into())
    }
    pub fn sync_profile(&self, id: &str) -> Result<Profile> {
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        self.sync_manifest(id)?;
        let path = self.data(id).join("config.json");
        if !path.exists() {
            return Ok(Profile {
                revision: 0,
                config: Config::default(),
            });
        }
        read_json(&path)
    }
    pub fn sync_save(&self, id: &str, expected: u64, config: Config) -> Result<Profile> {
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        self.sync_manifest(id)?;
        if self.sync_running(id) {
            return Err("同步仍在运行，不能修改配置".into());
        }
        config.validate()?;
        let path = self.data(id).join("config.json");
        let revision = if path.exists() {
            read_json::<Profile>(&path)?.revision
        } else {
            0
        };
        if revision != expected {
            return Err("配置已被更新，请重新载入后保存".into());
        }
        if path.exists() {
            let old: Profile = read_json(&path)?;
            if serde_json::to_value(&old.config).map_err(|e| e.to_string())?
                == serde_json::to_value(&config).map_err(|e| e.to_string())?
            {
                return Ok(old);
            }
        }
        let profile = Profile {
            revision: revision + 1,
            config,
        };
        write_json(&path, &profile)?;
        Ok(profile)
    }
    pub fn sync_import(&self, id: &str, path: &Path) -> Result<Imported> {
        self.sync_manifest(id)?;
        config::import(path)
    }
    pub fn sync_check(&self, id: &str, config: &Config, password: &str) -> Result<Preflight> {
        self.sync_manifest(id)?;
        let ctrl = self.make_control(id, 0, config, None);
        let effective = if password.is_empty() && !config.svn_credential_file.is_empty() {
            config::password_file(&config.svn_credential_file)?
        } else {
            password.to_string()
        };
        engine::preflight(config, &effective, &ctrl, None)
            .map_err(|error| process::redact(&error, &effective))
    }
    pub fn sync_validate_saved(&self, id: &str, revision: u64) -> Result<()> {
        let profile = self.sync_profile(id)?;
        if profile.revision == 0 || profile.revision != revision {
            return Err("插件配置已变化或未保存，请在插件中重新创建/更新定时计划".into());
        }
        profile.config.validate()
    }
    fn sync_running(&self, id: &str) -> bool {
        self.sync_jobs
            .lock()
            .unwrap()
            .values()
            .any(|c| c.state.lock().unwrap().plugin_id == id)
    }
    pub fn sync_running_count(&self) -> usize {
        self.sync_jobs.lock().unwrap().len()
    }
    pub fn sync_shutdown(&self, stop: bool) {
        let _guard = self.lock.lock().unwrap();
        self.sync_stopping.store(true, Ordering::SeqCst);
        if stop {
            for c in self.sync_jobs.lock().unwrap().values() {
                c.cancel.store(true, Ordering::SeqCst);
            }
        }
    }
    pub fn sync_cancel(&self, run_id: &str) -> Result<()> {
        let jobs = self.sync_jobs.lock().unwrap();
        let c = jobs.get(run_id).ok_or("执行已结束")?;
        c.cancel.store(true, Ordering::SeqCst);
        Ok(())
    }
    pub(super) fn make_control(
        &self,
        id: &str,
        revision: u64,
        c: &Config,
        sink: Option<LogSink>,
    ) -> Arc<Control> {
        let manifest = self.manifest(id).ok();
        let workflow = manifest.as_ref().and_then(|m| m.workflow.clone());
        let pushes = workflow
            .as_ref()
            .is_some_and(|w| w.per_repository.contains(&Step::Push));
        Arc::new(Control {
            cancel: AtomicBool::new(false),
            state: Mutex::new(Run {
                id: uuid::Uuid::new_v4().to_string(),
                plugin_id: id.into(),
                status: "running".into(),
                started_at: now(),
                finished_at: None,
                logs: vec![],
                repositories: c
                    .repositories
                    .iter()
                    .map(|r| RepoResult {
                        name: r.name.clone(),
                        directory: r.directory.clone(),
                        status: "pending".into(),
                        git_push: if pushes {
                            "not_started"
                        } else {
                            "not_requested"
                        }
                        .into(),
                        git_commit: String::new(),
                        svn_commit: "not_started".into(),
                        archive: String::new(),
                        message: String::new(),
                    })
                    .collect(),
                message: "正在执行；不能以窗口或进程退出推断远端结果".into(),
                config_revision: revision,
                plugin_version: manifest.map(|m| m.version).unwrap_or_default(),
                workflow,
            }),
            sink,
        })
    }
    fn reserve_sync(
        &self,
        id: &str,
        revision: u64,
        sink: Option<LogSink>,
    ) -> Result<(Config, Workflow, Arc<Control>)> {
        let _g = self.lock.lock().map_err(|e| e.to_string())?;
        let workflow = self.sync_manifest(id)?;
        if self.sync_stopping.load(Ordering::SeqCst) {
            return Err("小工坊正在退出".into());
        }
        if self.sync_running_count() > 0 {
            return Err("已有原生同步正在运行，本次不重叠执行，以避免工作目录和 SVN 冲突".into());
        }
        let profile: Profile = read_json(&self.data(id).join("config.json"))?;
        if profile.revision != revision {
            return Err("配置版本已改变，请重新保存或创建定时计划".into());
        }
        profile.config.validate()?;
        let ctrl = self.make_control(id, revision, &profile.config, sink);
        let state = ctrl.state.lock().unwrap().clone();
        write_json(
            &self
                .data(id)
                .join("runs")
                .join(format!("{}.json", state.id)),
            &state,
        )?;
        self.sync_jobs
            .lock()
            .unwrap()
            .insert(state.id, ctrl.clone());
        Ok((profile.config, workflow, ctrl))
    }
    fn perform_sync(
        &self,
        config: &Config,
        workflow: &Workflow,
        password: &str,
        ctrl: &Arc<Control>,
        external: Option<&AtomicBool>,
    ) -> Run {
        let credential = if password.is_empty() && !config.svn_credential_file.is_empty() {
            config::password_file(&config.svn_credential_file)
        } else {
            Ok(password.to_string())
        };
        let effective = credential.as_ref().map(String::as_str).unwrap_or("");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match &credential {
            Ok(value) => engine::execute(config, workflow, value, ctrl, external),
            Err(e) => Err(e.clone()),
        }))
        .unwrap_or_else(|_| Err("原生同步发生异常，保留现场；已发生的远端写入需核对".into()));
        {
            let mut state = ctrl.state.lock().unwrap();
            let cancelled = ctrl.cancel.load(Ordering::SeqCst)
                || external.is_some_and(|c| c.load(Ordering::SeqCst));
            state.status = if cancelled {
                "cancelled"
            } else if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            }
            .into();
            state.finished_at = Some(now());
            state.message = match result {
                Ok(()) => "同步完成；Git 和 SVN 命令均成功，可能包含无变更提交".into(),
                Err(e) => process::redact(&e, effective),
            };
        }
        {
            let mut state = ctrl.state.lock().unwrap();
            let clip = |text: &mut String, max: usize| {
                if text.len() > max {
                    let mut end = max;
                    while !text.is_char_boundary(end) {
                        end -= 1;
                    }
                    text.truncate(end);
                    text.push_str(" [已截断]");
                }
            };
            clip(&mut state.message, 8192);
            for repo in &mut state.repositories {
                clip(&mut repo.message, 4096);
                clip(&mut repo.archive, 4096);
            }
        }
        let state = ctrl.state.lock().unwrap().clone();
        let _guard = self.lock.lock().unwrap();
        let persisted = write_json(
            &self
                .data(&state.plugin_id)
                .join("runs")
                .join(format!("{}.json", state.id)),
            &state,
        );
        if let Err(e) = persisted {
            let mut state = ctrl.state.lock().unwrap();
            state.status = "interrupted".into();
            state.message = format!("执行结果持久化失败：{e}；请核对远端写入");
            let mut failed = self.sync_unpersisted.lock().unwrap();
            failed.insert(state.id.clone(), state.clone());
            if failed.len() > 50 {
                if let Some(oldest) = failed
                    .values()
                    .min_by(|a, b| a.started_at.cmp(&b.started_at))
                    .map(|run| run.id.clone())
                {
                    failed.remove(&oldest);
                }
            }
        }
        let state = ctrl.state.lock().unwrap().clone();
        self.sync_jobs.lock().unwrap().remove(&state.id);
        // Retention errors never change the actual execution result.
        let _ = self.prune_sync_runs(&state.plugin_id);
        state
    }
    pub fn sync_start(
        self: &Arc<Self>,
        id: &str,
        revision: u64,
        password: String,
    ) -> Result<String> {
        if password.len() > 1024 || password.contains(['\r', '\n', '\0']) {
            return Err("SVN 口令无效".into());
        }
        let (config, workflow, ctrl) = self.reserve_sync(id, revision, None)?;
        let run_id = ctrl.state.lock().unwrap().id.clone();
        let host = self.clone();
        std::thread::spawn(move || {
            host.perform_sync(&config, &workflow, &password, &ctrl, None);
        });
        Ok(run_id)
    }
    pub fn sync_execute_saved(
        self: &Arc<Self>,
        id: &str,
        revision: u64,
        cancel: &AtomicBool,
        sink: LogSink,
    ) -> Result<Run> {
        let (config, workflow, ctrl) = self.reserve_sync(id, revision, Some(sink))?;
        Ok(self.perform_sync(&config, &workflow, "", &ctrl, Some(cancel)))
    }
    pub fn sync_history(&self, id: &str) -> Result<Vec<Run>> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        crate::valid_id(id)?;
        let dir = self.data(id).join("runs");
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|v| v != "json") {
                continue;
            }
            let cached = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(|id| self.sync_unpersisted.lock().unwrap().get(id).cloned());
            let mut run: Run = match cached {
                Some(run) => run,
                None => read_json(&path)?,
            };
            if let Some(ctrl) = self.sync_jobs.lock().unwrap().get(&run.id) {
                run = ctrl.state.lock().unwrap().clone();
            } else if run.status == "running" {
                run.status = "interrupted".into();
                run.message = "当前无此活动执行；外部操作结果需核对".into();
            }
            out.push(run);
        }
        out.sort_by(|a, b| {
            (b.status == "running")
                .cmp(&(a.status == "running"))
                .then_with(|| b.started_at.cmp(&a.started_at))
        });
        out.truncate(50);
        Ok(out)
    }
    pub fn sync_detail(&self, id: &str, run_id: &str) -> Result<Run> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        crate::valid_id(id)?;
        uuid::Uuid::parse_str(run_id).map_err(|_| "执行 ID 无效")?;
        if let Some(ctrl) = self.sync_jobs.lock().unwrap().get(run_id) {
            let state = ctrl.state.lock().unwrap().clone();
            if state.plugin_id != id {
                return Err("执行不属于此插件".into());
            }
            return Ok(state);
        }
        if let Some(run) = self
            .sync_unpersisted
            .lock()
            .unwrap()
            .get(run_id)
            .filter(|run| run.plugin_id == id)
        {
            return Ok(run.clone());
        }
        let mut run: Run = read_json(&self.data(id).join("runs").join(format!("{run_id}.json")))?;
        if run.status == "running" {
            run.status = "interrupted".into();
            run.message = "当前无此活动执行；外部操作结果需核对".into();
        }
        Ok(run)
    }
    pub(crate) fn recover_sync_runs(&self) -> Result<()> {
        let data = self.root.join("_data");
        if !data.is_dir() {
            return Ok(());
        }
        for plugin in fs::read_dir(data).map_err(|e| e.to_string())? {
            let plugin = plugin.map_err(|e| e.to_string())?;
            let id = plugin.file_name().to_string_lossy().into_owned();
            if crate::valid_id(&id).is_err() {
                continue;
            }
            let directory = self.data(&id).join("runs");
            if !directory.is_dir() {
                continue;
            }
            for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.extension().is_none_or(|v| v != "json") {
                    continue;
                }
                let Ok(mut run) = read_json::<Run>(&path) else {
                    continue;
                };
                if run.status == "running" {
                    run.status = "interrupted".into();
                    run.finished_at = Some(now());
                    run.message = "上次运行未正常结束；需核对外部操作结果，不自动重跑".into();
                    write_json(&path, &run)?;
                }
            }
            self.prune_sync_runs(&id)?;
        }
        Ok(())
    }
    fn prune_sync_runs(&self, id: &str) -> Result<()> {
        let directory = self.data(id).join("runs");
        if !directory.is_dir() {
            return Ok(());
        }
        let mut finished = Vec::new();
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|v| v != "json") {
                continue;
            }
            if let Ok(run) = read_json::<Run>(&path) {
                if run.status != "running" {
                    finished.push((run.started_at, path));
                }
            }
        }
        finished.sort_by(|a, b| b.0.cmp(&a.0));
        for (_, path) in finished.into_iter().skip(50) {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_final_persistence_remains_queryable_without_blocking_future_runs() {
        let directory = tempfile::tempdir().unwrap();
        let host = Host::open(directory.path().join("plugins")).unwrap();
        let manifest = host
            .install(include_bytes!(
                "../../../../../plugins/code-sync/code-sync.hkp"
            ))
            .unwrap();
        let control = host.make_control(&manifest.id, 1, &Config::default(), None);
        let id = control.state.lock().unwrap().id.clone();
        host.sync_jobs
            .lock()
            .unwrap()
            .insert(id.clone(), control.clone());
        fs::create_dir_all(
            host.data(&manifest.id)
                .join("runs")
                .join(format!("{id}.json")),
        )
        .unwrap();
        let result = host.perform_sync(
            &Config::default(),
            &manifest.workflow.unwrap(),
            "",
            &control,
            None,
        );
        assert_eq!(result.status, "interrupted");
        assert_eq!(host.sync_running_count(), 0);
        assert!(host
            .sync_detail(&manifest.id, &id)
            .unwrap()
            .message
            .contains("持久化失败"));
        assert_eq!(
            host.sync_history(&manifest.id).unwrap()[0].status,
            "interrupted"
        );
    }
    #[test]
    fn retention_prunes_finished_records_but_keeps_active_execution_visible() {
        let directory = tempfile::tempdir().unwrap();
        let host = Host::open(directory.path().join("plugins")).unwrap();
        let manifest = host
            .install(include_bytes!(
                "../../../../../plugins/code-sync/code-sync.hkp"
            ))
            .unwrap();
        let control = host.make_control(&manifest.id, 1, &Config::default(), None);
        let active = control.state.lock().unwrap().clone();
        host.sync_jobs
            .lock()
            .unwrap()
            .insert(active.id.clone(), control);
        let runs = host.data(&manifest.id).join("runs");
        write_json(&runs.join(format!("{}.json", active.id)), &active).unwrap();
        for index in 1..=55 {
            let mut finished = active.clone();
            finished.id = uuid::Uuid::new_v4().to_string();
            finished.status = "succeeded".into();
            finished.started_at =
                (chrono::Utc::now() + chrono::Duration::seconds(index)).to_rfc3339();
            finished.finished_at = Some(finished.started_at.clone());
            write_json(&runs.join(format!("{}.json", finished.id)), &finished).unwrap();
        }
        host.prune_sync_runs(&manifest.id).unwrap();
        assert_eq!(fs::read_dir(runs).unwrap().count(), 51);
        let history = host.sync_history(&manifest.id).unwrap();
        assert_eq!(history.len(), 50);
        assert_eq!(history[0].id, active.id);
        assert_eq!(history[0].status, "running");
    }
}
