//! The native lifetime is independent of the main window's visibility.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, WindowEvent,
};

fn lifecycle_log(app: &AppHandle, message: &str) {
    eprintln!("[desktop] {message}");
    #[cfg(not(debug_assertions))]
    let _ = app;
    #[cfg(debug_assertions)]
    if let Ok(directory) = app.path().app_log_dir() {
        use std::io::Write;
        if std::fs::create_dir_all(&directory).is_ok() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(directory.join("desktop.log"))
            {
                let _ = writeln!(file, "{} {message}", std::process::id());
            }
        }
    }
}

const SHOW: &str = "handykit.show";
const HIDE: &str = "handykit.hide";
const QUIT: &str = "handykit.quit";
const TRAY_ID: &str = "handykit.tray";

#[derive(Default)]
pub struct DesktopState {
    // If the system cannot create a tray, closing must still quit normally.
    tray_ready: AtomicBool,
}

pub fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    #[cfg(target_os = "macos")]
    if let Err(error) = app.set_activation_policy(tauri::ActivationPolicy::Regular) {
        eprintln!("Cannot restore Dock visibility: {error}");
    }
    if let Err(error) = window
        .unminimize()
        .and_then(|_| window.show())
        .and_then(|_| window.set_focus())
    {
        eprintln!("Cannot show main window: {error}");
    }
}

/// Return true only after a recoverable background state was established.
pub fn hide_main_window(app: &AppHandle) -> bool {
    lifecycle_log(app, "hide requested");
    if !app
        .state::<DesktopState>()
        .tray_ready
        .load(Ordering::Relaxed)
    {
        return false;
    }
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    if let Err(error) = window.hide() {
        eprintln!("Cannot hide main window: {error}");
        return false;
    }
    #[cfg(target_os = "macos")]
    if let Err(error) = app.set_activation_policy(tauri::ActivationPolicy::Accessory) {
        eprintln!("Cannot hide Dock item: {error}");
    }
    lifecycle_log(app, "main window hidden; tray retained");
    true
}

pub fn on_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    lifecycle_log(app, &format!("menu: {}", event.id().as_ref()));
    match event.id().as_ref() {
        SHOW => show_main_window(app),
        HIDE => {
            if !hide_main_window(app) {
                // A failed tray must not leave a background process without recovery UI.
                app.exit(0);
            }
        }
        QUIT => app.exit(0),
        _ => {}
    }
}

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW, "显示主窗口", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, HIDE, "隐藏主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT, "彻底退出小工坊", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&show, &hide, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
    #[cfg(target_os = "macos")]
    let icon = tauri::include_image!("icons/tray-template.png");
    #[cfg(not(target_os = "macos"))]
    let icon = tauri::include_image!("icons/32x32.png");

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("小工坊 HandyKit")
        .menu(&menu)
        // macOS/Linux use a menu on click; Windows also supports direct restoration.
        .show_menu_on_left_click(!cfg!(target_os = "windows"))
        .on_tray_icon_event(|tray, event| {
            if cfg!(target_os = "windows")
                && matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                )
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app);
    match tray {
        Ok(_) => {
            app.state::<DesktopState>()
                .tray_ready
                .store(true, Ordering::Relaxed);
            lifecycle_log(app.handle(), "tray created");
        }
        Err(error) => lifecycle_log(
            app.handle(),
            &format!("System tray unavailable; closing will exit normally: {error}"),
        ),
    }
    #[cfg(target_os = "macos")]
    setup_macos_menu(app)?;
    Ok(())
}

pub fn on_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() == "main" {
        if let WindowEvent::CloseRequested { api, .. } = event {
            lifecycle_log(window.app_handle(), "close requested");
            if hide_main_window(window.app_handle()) {
                api.prevent_close();
            }
        }
    }
}

pub fn on_run_event(app: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::ExitRequested { code, .. } = &event {
        lifecycle_log(app, &format!("exit requested: {code:?}"));
    }
    match event {
        tauri::RunEvent::Exit => lifecycle_log(app, "process exiting"),
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => show_main_window(app),
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn setup_macos_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::Submenu;
    let app_menu = Submenu::with_items(
        app,
        "小工坊",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("关于小工坊"), None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, SHOW, "显示主窗口", true, None::<&str>)?,
            &MenuItem::with_id(app, HIDE, "隐藏到菜单栏", true, Some("Cmd+Q"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, Some("服务"))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "彻底退出小工坊", true, None::<&str>)?,
        ],
    )?;
    let edit_menu = Submenu::with_items(
        app,
        "编辑",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("撤销"))?,
            &PredefinedMenuItem::redo(app, Some("重做"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("剪切"))?,
            &PredefinedMenuItem::copy(app, Some("复制"))?,
            &PredefinedMenuItem::paste(app, Some("粘贴"))?,
            &PredefinedMenuItem::select_all(app, Some("全选"))?,
        ],
    )?;
    let window_menu = Submenu::with_items(
        app,
        "窗口",
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some("最小化"))?,
            &PredefinedMenuItem::maximize(app, Some("缩放"))?,
            &PredefinedMenuItem::close_window(app, Some("关闭到菜单栏"))?,
        ],
    )?;
    app.set_menu(Menu::with_items(
        app,
        &[&app_menu, &edit_menu, &window_menu],
    )?)?;
    // Builder registers a single app-wide handler, including when tray creation fails.
    Ok(())
}
