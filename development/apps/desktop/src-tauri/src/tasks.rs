use crate::blocking::run as blocking;
use handykit_scheduler::{cron, model::Task, Scheduler};
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
#[tauri::command]
pub async fn timer_overview(
    service: State<'_, Scheduler>,
) -> Result<handykit_scheduler::model::Overview, String> {
    let service = service.inner().clone();
    blocking(move || service.overview()).await
}
#[tauri::command]
pub async fn timer_preview(expression: String, zone: String) -> Result<cron::Preview, String> {
    blocking(move || cron::preview(&expression, &zone, chrono::Utc::now())).await
}
#[tauri::command]
pub async fn timer_save(service: State<'_, Scheduler>, task: Task) -> Result<Task, String> {
    let service = service.inner().clone();
    blocking(move || service.save(task)).await
}
#[tauri::command]
pub async fn timer_toggle(
    service: State<'_, Scheduler>,
    id: String,
    enabled: bool,
) -> Result<Task, String> {
    let service = service.inner().clone();
    blocking(move || service.toggle(&id, enabled)).await
}
#[tauri::command]
pub async fn timer_delete(service: State<'_, Scheduler>, id: String) -> Result<(), String> {
    let service = service.inner().clone();
    blocking(move || service.delete(&id)).await
}
#[tauri::command]
pub async fn timer_run(service: State<'_, Scheduler>, id: String) -> Result<String, String> {
    let service = service.inner().clone();
    blocking(move || service.run_now(&id)).await
}
#[tauri::command]
pub async fn timer_cancel(service: State<'_, Scheduler>, id: String) -> Result<(), String> {
    service.cancel(&id)
}
#[tauri::command]
pub async fn timer_history(
    service: State<'_, Scheduler>,
    task_id: Option<String>,
    offset: u32,
) -> Result<Vec<handykit_scheduler::model::Run>, String> {
    let service = service.inner().clone();
    blocking(move || service.history(task_id.as_deref(), offset)).await
}
#[tauri::command]
pub async fn timer_run_detail(
    service: State<'_, Scheduler>,
    id: String,
) -> Result<handykit_scheduler::model::Run, String> {
    let service = service.inner().clone();
    blocking(move || service.run_detail(&id)).await
}
#[tauri::command]
pub async fn timer_pick(app: tauri::AppHandle, kind: String) -> Result<Option<String>, String> {
    blocking(move || {
        let dialog = app.dialog().file();
        let path = match kind.as_str() {
            "folder" => dialog.blocking_pick_folder(),
            "application" if cfg!(target_os = "macos") => {
                dialog.add_filter("应用", &["app"]).blocking_pick_file()
            }
            "script" => dialog
                .add_filter("Shell 脚本", &["sh", "bash", "zsh"])
                .blocking_pick_file(),
            "target" | "interpreter" | "application" => dialog.blocking_pick_file(),
            _ => return Err("未知文件选择类型".into()),
        };
        path.map(|p| {
            p.into_path()
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| e.to_string())
        })
        .transpose()
    })
    .await
}
#[tauri::command]
pub async fn timer_platform() -> String {
    std::env::consts::OS.into()
}
#[tauri::command]
pub async fn timer_shutdown(app: tauri::AppHandle, mode: String) -> Result<(), String> {
    if mode == "continue" {
        crate::desktop::hide_main_window(&app);
        return Ok(());
    }
    if !["wait", "stop"].contains(&mode.as_str()) {
        return Err("未知退出模式".into());
    }
    let service = app.state::<Scheduler>().inner().clone();
    service.shutdown(mode == "stop");
    let plugins = app.state::<crate::plugins::PluginState>().0.clone();
    plugins.sync_shutdown(mode == "stop");
    std::thread::spawn(move || {
        while service.running_count() > 0 || plugins.sync_running_count() > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        app.exit(0);
    });
    Ok(())
}
pub fn guard_exit(app: &tauri::AppHandle, event: &tauri::RunEvent) -> bool {
    if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
        if code.is_none() && crate::desktop::hide_main_window(app) {
            api.prevent_exit();
            return true;
        }
        let service = app.state::<Scheduler>();
        if service.running_count() > 0
            || app
                .state::<crate::plugins::PluginState>()
                .0
                .sync_running_count()
                > 0
        {
            api.prevent_exit();
            crate::desktop::show_main_window(app);
            use tauri::Emitter;
            let _ = app.emit("timer-quit-requested", ());
            return true;
        }
        service.shutdown(false);
        app.state::<crate::plugins::PluginState>()
            .0
            .sync_shutdown(false);
    }
    false
}

#[tauri::command]
pub async fn timer_confirm(app: tauri::AppHandle, message: String) -> Result<bool, String> {
    if message.len() > 8000 {
        return Err("确认信息过长".into());
    }
    blocking(move || {
        Ok(app
            .dialog()
            .message(message)
            .title("小工坊")
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "继续".into(),
                "取消".into(),
            ))
            .blocking_show())
    })
    .await
}
