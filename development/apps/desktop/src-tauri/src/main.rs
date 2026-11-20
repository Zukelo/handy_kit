#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;

fn main() {
    tauri::Builder::default()
        .manage(desktop::DesktopState::default())
        .setup(|app| {
            desktop::setup(app)?;
            Ok(())
        })
        .on_menu_event(desktop::on_menu_event)
        .on_window_event(desktop::on_window_event)
        .build(tauri::generate_context!())
        .expect("无法启动小工坊")
        .run(desktop::on_run_event);
}
