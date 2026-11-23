#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod blocking;
mod desktop;
mod plugins;
mod tasks;
use tauri::Manager;

#[tauri::command]
fn set_appearance(window: tauri::WebviewWindow, mode: String) -> Result<(), String> {
    let theme = match mode.as_str() {
        "system" => None,
        "light" => Some(tauri::Theme::Light),
        "dark" => Some(tauri::Theme::Dark),
        _ => return Err("无效的外观主题".to_string()),
    };
    window.set_theme(theme).map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            desktop::show_main_window(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            set_appearance,
            plugins::plugins_list,
            plugins::plugins_install,
            plugins::plugins_toggle,
            plugins::plugins_uninstall,
            plugins::plugins_run,
            plugins::plugins_sync_profile,
            plugins::plugins_sync_save,
            plugins::plugins_sync_import,
            plugins::plugins_sync_check,
            plugins::plugins_sync_start,
            plugins::plugins_sync_history,
            plugins::plugins_sync_detail,
            plugins::plugins_sync_cancel,
            plugins::plugins_sync_schedule,
            tasks::timer_overview,
            tasks::timer_preview,
            tasks::timer_save,
            tasks::timer_toggle,
            tasks::timer_delete,
            tasks::timer_run,
            tasks::timer_cancel,
            tasks::timer_history,
            tasks::timer_run_detail,
            tasks::timer_pick,
            tasks::timer_platform,
            tasks::timer_shutdown,
            tasks::timer_confirm
        ])
        .manage(desktop::DesktopState::default())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?.join("runtime");
            let plugin_host = std::sync::Arc::new(
                handykit_plugin_host::Host::open(data_dir.join("plugins"))
                    .map_err(std::io::Error::other)?,
            );
            let service = handykit_scheduler::Scheduler::open_with_plugins(
                &data_dir,
                Some(std::sync::Arc::new(plugins::ScheduledPlugins(
                    plugin_host.clone(),
                ))),
            )
            .map_err(std::io::Error::other)?;
            app.manage(plugins::PluginState(plugin_host));
            app.manage(service.clone());
            desktop::setup(app)?;
            service.start();
            Ok(())
        })
        .on_menu_event(desktop::on_menu_event)
        .on_window_event(desktop::on_window_event)
        .build(tauri::generate_context!())
        .expect("无法启动小工坊")
        .run(|app, event| {
            if !tasks::guard_exit(app, &event) {
                desktop::on_run_event(app, event);
            }
        });
}
