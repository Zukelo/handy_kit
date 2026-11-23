use crate::blocking::run as blocking;
use handykit_plugin_host::{
    sync::{Config, Imported, Preflight, Profile, Run},
    Host, Installed, Manifest, Report, Settings,
};
use handykit_scheduler::{
    model::{Action, Task},
    Scheduler,
};
use std::{io::Read, sync::Arc};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

pub struct PluginState(pub Arc<Host>);

#[tauri::command]
pub async fn plugins_list(state: State<'_, PluginState>) -> Result<Vec<Installed>, String> {
    let host = state.0.clone();
    blocking(move || host.list()).await
}

#[tauri::command]
pub async fn plugins_install(
    app: tauri::AppHandle,
    state: State<'_, PluginState>,
) -> Result<Option<Manifest>, String> {
    let host = state.0.clone();
    blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("小工坊插件包", &["hkp"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let mut data = Vec::new();
        std::fs::File::open(file.into_path().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        host.install(&data).map(Some)
    })
    .await
}

#[tauri::command]
pub async fn plugins_toggle(
    state: State<'_, PluginState>,
    id: String,
    enabled: bool,
    service: State<'_, Scheduler>,
) -> Result<(), String> {
    let host = state.0.clone();
    let service = service.inner().clone();
    blocking(move || {
        host.toggle(&id, enabled)?;
        if !enabled {
            service.pause_plugin_tasks(&id)?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn plugins_uninstall(
    state: State<'_, PluginState>,
    id: String,
    service: State<'_, Scheduler>,
) -> Result<(), String> {
    let host = state.0.clone();
    let service = service.inner().clone();
    blocking(move || {
        host.uninstall(&id)?;
        service.pause_plugin_tasks(&id)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn plugins_run(
    state: State<'_, PluginState>,
    id: String,
    settings: Settings,
) -> Result<Report, String> {
    let host = state.0.clone();
    blocking(move || host.run(&id, &settings)).await
}

pub struct ScheduledPlugins(pub Arc<Host>);
impl handykit_scheduler::PluginExecutor for ScheduledPlugins {
    fn validate(&self, id: &str, revision: u64) -> Result<(), String> {
        self.0.sync_validate_saved(id, revision)
    }
    fn execute(
        &self,
        id: &str,
        revision: u64,
        cancel: &std::sync::atomic::AtomicBool,
        output: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Result<(), String> {
        let result = self.0.sync_execute_saved(id, revision, cancel, output)?;
        if result.status == "cancelled" {
            cancel.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if result.status == "succeeded" {
            Ok(())
        } else {
            Err(format!("{}；插件执行记录 {}", result.message, result.id))
        }
    }
}

#[tauri::command]
pub async fn plugins_sync_profile(
    state: State<'_, PluginState>,
    id: String,
) -> Result<Profile, String> {
    let host = state.0.clone();
    blocking(move || host.sync_profile(&id)).await
}
#[tauri::command]
pub async fn plugins_sync_save(
    state: State<'_, PluginState>,
    service: State<'_, Scheduler>,
    id: String,
    revision: u64,
    config: Config,
) -> Result<Profile, String> {
    let host = state.0.clone();
    let service = service.inner().clone();
    blocking(move || {
        let profile = host.sync_save(&id, revision, config)?;
        if profile.revision != revision {
            service.pause_plugin_tasks(&id)?;
        }
        Ok(profile)
    })
    .await
}
#[tauri::command]
pub async fn plugins_sync_import(
    state: State<'_, PluginState>,
    id: String,
    directory: String,
) -> Result<Imported, String> {
    let host = state.0.clone();
    blocking(move || host.sync_import(&id, std::path::Path::new(&directory))).await
}
#[tauri::command]
pub async fn plugins_sync_check(
    state: State<'_, PluginState>,
    id: String,
    config: Config,
    password: String,
) -> Result<Preflight, String> {
    let host = state.0.clone();
    blocking(move || host.sync_check(&id, &config, &password)).await
}
#[tauri::command]
pub async fn plugins_sync_start(
    state: State<'_, PluginState>,
    id: String,
    revision: u64,
    password: String,
) -> Result<String, String> {
    let host = state.0.clone();
    blocking(move || host.sync_start(&id, revision, password)).await
}
#[tauri::command]
pub async fn plugins_sync_history(
    state: State<'_, PluginState>,
    id: String,
) -> Result<Vec<Run>, String> {
    let host = state.0.clone();
    blocking(move || host.sync_history(&id)).await
}
#[tauri::command]
pub async fn plugins_sync_detail(
    state: State<'_, PluginState>,
    id: String,
    run_id: String,
) -> Result<Run, String> {
    let host = state.0.clone();
    blocking(move || host.sync_detail(&id, &run_id)).await
}
#[tauri::command]
pub async fn plugins_sync_cancel(
    state: State<'_, PluginState>,
    run_id: String,
) -> Result<(), String> {
    state.0.sync_cancel(&run_id)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduleOptions {
    name: String,
    cron: String,
    zone: String,
    timeout: Option<u64>,
}

#[tauri::command]
pub async fn plugins_sync_schedule(
    state: State<'_, PluginState>,
    service: State<'_, Scheduler>,
    id: String,
    revision: u64,
    options: ScheduleOptions,
) -> Result<Task, String> {
    let host = state.0.clone();
    let service = service.inner().clone();
    blocking(move || {
        host.sync_validate_saved(&id, revision)?;
        service.save(Task {
            id: String::new(),
            revision: 0,
            name: options.name,
            description: "使用插件已保存的配置；变更配置后需更新计划版本。".into(),
            enabled: false,
            cron: options.cron,
            zone: options.zone,
            timeout: options.timeout,
            action: Action {
                kind: "plugin".into(),
                target: id,
                interpreter: String::new(),
                args: vec![],
                env: vec![],
                cwd: String::new(),
                instance: "reuse".into(),
                plugin_revision: Some(revision),
            },
        })
    })
    .await
}
