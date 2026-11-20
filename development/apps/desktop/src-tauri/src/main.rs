#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod desktop;

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
        .invoke_handler(tauri::generate_handler![set_appearance])
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
