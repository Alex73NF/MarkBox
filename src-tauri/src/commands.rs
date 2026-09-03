use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::settings;
use crate::windows;
use crate::AppState;

/// 屏幕级物理矩形（全局坐标），序列化给前端的唯一监控器几何载体
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlayInit { pub monitor: MonitorRect }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhysRect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfirmPayload { pub rect: PhysRect }

#[tauri::command]
pub(crate) fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub(crate) fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: settings::Settings) -> Result<settings::Settings, String> {
    let normalized = settings::normalize(&settings);
    settings::save_to(&settings::settings_path(&app), &normalized).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = normalized.clone();
    if let Err(e) = app.emit_to("mark", "settings-updated", &normalized) {
        eprintln!("[markbox] 发送 settings-updated 失败: {e}");
    }
    Ok(normalized)
}

#[tauri::command]
pub(crate) fn start_selection(app: AppHandle) -> Result<(), String> {
    windows::begin_selection(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn overlay_ready(app: AppHandle, label: String) -> Result<OverlayInit, String> {
    windows::overlay_init(&app, &label).ok_or_else(|| format!("unknown overlay label: {label}"))
}

#[tauri::command]
pub(crate) fn confirm_selection(app: AppHandle, payload: ConfirmPayload) -> Result<(), String> {
    let r = &payload.rect;
    let ok = windows::rect_on_existing_monitor(&app, r.x, r.y, r.w, r.h);
    windows::end_selection(&app);
    if ok {
        if let Err(e) = windows::spawn_mark(&app, r.x, r.y, r.w, r.h) {
            windows::show_main(&app); // 标记窗建失败也要把主窗口还给用户，别丢在黑屏里
            return Err(e.to_string());
        }
    } else {
        // 圈选期间显示器被拔掉：整单取消
        windows::emit_mark_state(&app);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn cancel_selection(app: AppHandle) {
    windows::end_selection(&app);
}

#[tauri::command]
pub(crate) fn clear_mark(app: AppHandle) {
    windows::destroy_mark(&app);
    // destroy 在事件循环异步生效，事后查询会拿到过期状态，直接广播已知结果
    if let Err(e) = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": false })) {
        eprintln!("[markbox] 发送 mark-state 失败: {e}");
    }
}

/// 托盘菜单事件分发用
pub(crate) fn handle_tray(app: &AppHandle, id: &str) {
    match id {
        "select" => {
            if let Err(e) = windows::begin_selection(app) {
                eprintln!("[markbox] 发起圈选失败: {e}");
            }
        }
        "clear" => clear_mark(app.clone()),
        "show" => windows::show_main(app),
        "quit" => app.exit(0),
        _ => {}
    }
}
