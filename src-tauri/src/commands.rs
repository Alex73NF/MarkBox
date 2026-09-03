use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::windows;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayInit { pub monitor: MonitorRect, pub scale_factor: f64 }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysRect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmPayload { pub rect: PhysRect }

#[tauri::command]
pub fn start_selection(app: AppHandle) -> Result<(), String> {
    windows::begin_selection(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn overlay_ready(app: AppHandle, label: String) -> Result<OverlayInit, String> {
    windows::overlay_init(&app, &label).ok_or_else(|| format!("unknown overlay label: {label}"))
}

#[tauri::command]
pub fn confirm_selection(app: AppHandle, payload: ConfirmPayload) -> Result<(), String> {
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
pub fn cancel_selection(app: AppHandle) {
    windows::end_selection(&app);
}

#[tauri::command]
pub fn clear_mark(app: AppHandle) {
    windows::destroy_mark(&app);
    // destroy 在事件循环异步生效，事后查询会拿到过期状态，直接广播已知结果
    let _ = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": false }));
}

/// 托盘菜单事件分发用
pub fn handle_tray(app: &AppHandle, id: &str) {
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
