mod commands;
mod settings;
mod windows;

use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};

use crate::commands::MonitorRect;

#[derive(Default)]
pub(crate) struct AppState {
    pub(crate) settings: Mutex<settings::Settings>,
    pub(crate) monitors: Mutex<Vec<(String, MonitorRect)>>,
    pub(crate) selecting: Mutex<bool>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_main(app);
        }))
        .setup(|app| {
            let path = settings::settings_path(app.handle());
            // 文件缺失/损坏时回退默认并重建；正常路径不多写一次磁盘
            let (loaded, needs_repair) = settings::load_or_repair(&path);
            if needs_repair {
                if let Err(e) = settings::save_to(&path, &loaded) {
                    eprintln!("[markbox] 重建设置文件失败: {e}");
                }
            }
            app.manage(AppState { settings: Mutex::new(loaded), monitors: Mutex::default(), selecting: Mutex::default() });

            let select = MenuItem::with_id(app, "select", "圈选", true, None::<&str>)?;
            let clear = MenuItem::with_id(app, "clear", "清除标记", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = MenuBuilder::new(app).items(&[&select, &clear, &show]).separator().item(&quit).build()?;

            TrayIconBuilder::with_id("markbox-tray")
                .icon(app.default_window_icon().expect("default window icon must exist").clone())
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
                if let Err(e) = window.hide() {
                    eprintln!("[markbox] 隐藏主窗口失败: {e}");
                }
            }
            WindowEvent::Destroyed if window.label().starts_with("overlay-") => {
                // 任一 overlay 被销毁（崩溃/拔屏）→ 兜底清掉全部圈选层
                windows::end_selection(window.app_handle());
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings, commands::save_settings,
            commands::start_selection, commands::overlay_ready,
            commands::confirm_selection, commands::cancel_selection, commands::clear_mark
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
