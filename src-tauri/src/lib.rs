mod commands;
mod settings;
mod windows;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::WindowEvent;

use crate::windows::MonitorInfo;

#[derive(Default)]
pub struct AppState {
    pub settings: Mutex<settings::Settings>,
    pub monitors: Mutex<Vec<(String, MonitorInfo)>>,
    pub selecting: Mutex<bool>,
}

pub fn settings_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_config_dir().unwrap().join("settings.json")
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: settings::Settings) -> Result<settings::Settings, String> {
    let normalized = settings::normalize(&settings);
    settings::save_to(&settings_path(&app), &normalized).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = normalized.clone();
    let _ = app.emit_to("mark", "settings-updated", &normalized);
    Ok(normalized)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_main(app);
        }))
        .setup(|app| {
            let loaded = settings::load_from(&settings_path(app.handle()));
            // 回写默认值：重建缺失/损坏的配置文件并持久化归一化结果
            let _ = settings::save_to(&settings_path(app.handle()), &loaded);
            app.manage(AppState { settings: Mutex::new(loaded), monitors: Mutex::default(), selecting: Mutex::default() });

            let select = MenuItem::with_id(app, "select", "圈选", true, None::<&str>)?;
            let clear = MenuItem::with_id(app, "clear", "清除标记", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = MenuBuilder::new(app).items(&[&select, &clear, &show]).separator().item(&quit).build()?;

            TrayIconBuilder::with_id("markbox-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("MarkBox")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| commands::handle_tray(app, event.id().as_ref()))
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        commands::handle_tray(tray.app_handle(), "show");
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Destroyed => {
                // 任一 overlay 被销毁（崩溃/拔屏）→ 兜底清掉全部圈选层
                if window.label().starts_with("overlay-") {
                    windows::end_selection(window.app_handle());
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings, save_settings,
            commands::start_selection, commands::overlay_ready,
            commands::confirm_selection, commands::cancel_selection, commands::clear_mark
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
