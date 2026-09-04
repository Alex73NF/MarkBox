mod commands;
mod logging;
mod settings;
mod windows;

use std::sync::atomic::AtomicU64;
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
    // 圈选创建代次：end_selection（取消/确认/兜底销毁）递增，创建循环据此发现会话已被取消并静默中止。
    // 当前圈选命令与 Destroyed 事件都在主线程串行执行，代次读写实际单线程，Relaxed 即正确；
    // 若未来把圈选命令 async 化，需改用 Acquire/Release 建立跨线程可见性
    pub(crate) selection_gen: AtomicU64,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show_main(app);
        }))
        .setup(|app| {
            logging::init(app.handle());
            let path = settings::settings_path(app.handle());
            // save_to 崩在写盘与改名之间会留下唯一名 tmp，启动时先清掉再加载
            settings::cleanup_tmp_leftovers(&path);
            // 文件缺失/损坏时回退默认并重建；正常路径不多写一次磁盘
            let (loaded, needs_repair) = settings::load_or_repair(&path);
            if needs_repair {
                logging::log_error("设置文件缺失或损坏，已回退默认值并在下次保存时重建");
                logging::log_err("重建设置文件失败", settings::save_to(&path, &loaded));
            }
            app.manage(AppState {
                settings: Mutex::new(loaded),
                monitors: Mutex::default(),
                selecting: Mutex::default(),
                selection_gen: AtomicU64::new(0),
            });

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
                windows::hide_main(window.app_handle());
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
