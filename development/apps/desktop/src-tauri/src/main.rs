#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;
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
            let service = handykit_scheduler::Scheduler::open(&app.path().app_data_dir()?)
                .map_err(std::io::Error::other)?;
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
