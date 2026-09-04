//! IPC 边界：7 个 tauri command 的实现、前后端契约类型（serde camelCase）与托盘事件分发

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::logging;
use crate::settings;
use crate::windows;
use crate::AppState;

/// 屏幕级物理矩形（全局坐标），序列化给前端的唯一监控器几何载体
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorRect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }

impl From<&tauri::Monitor> for MonitorRect {
    fn from(m: &tauri::Monitor) -> Self {
        MonitorRect {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OverlayInit { pub monitor: MonitorRect }

/// w/h：选区矩形字段名（对齐前端 types.ts 的 w/h vs width/height 约定，勿"统一"）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PhysRect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfirmPayload { pub rect: PhysRect }

/// mark-state 事件载荷（Rust 侧唯一契约来源，与前端 MarkState 接口字段对齐）
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkState { pub has_mark: bool }

// 设置读写含磁盘 IO，必须移出主线程：同步命令在主线程执行，写盘期间会冻结托盘与全部窗口；
// async 属性使其转到运行时线程池执行，锁语义不变。
// 圈选/确认/取消等窗口操作命令保持同步（主线程串行），窗口创建/销毁时序才确定。
#[tauri::command(async)]
pub(crate) fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    lock_settings(&state).clone()
}

#[tauri::command(async)]
pub(crate) fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: settings::Settings) -> Result<settings::Settings, String> {
    let normalized = settings::normalize(&settings);
    // 路径求值放在锁外：其内部 expect 是设置链路唯一现实 panic 点，不能在持锁时引爆
    let path = settings::settings_path(&app);
    // 写盘与内存提交在同一把锁内串行：并发 saveNow 交错时保证内存 == 磁盘，
    // 否则 A 写盘 → B 写盘 → B 先拿锁 → A 后拿锁会让磁盘是 B、内存是 A
    let mut guard = lock_settings(&state);
    settings::save_to(&path, &normalized).map_err(|e| e.to_string())?;
    *guard = normalized.clone();
    drop(guard);
    logging::log_err("发送 settings-updated 失败", app.emit_to("mark", "settings-updated", &normalized));
    Ok(normalized)
}

/// 取设置锁；毒化（持锁 panic 过）也取回内部数据继续服务——
/// 设置并非锁协议参与者，毒化只说明曾有一次 panic，级联拒服比用旧值更糟
fn lock_settings<'a>(state: &'a tauri::State<'_, AppState>) -> std::sync::MutexGuard<'a, settings::Settings> {
    state.settings.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[tauri::command]
pub(crate) fn start_selection(app: AppHandle) -> Result<(), String> {
    windows::begin_selection(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn overlay_ready(app: AppHandle, label: String) -> Result<OverlayInit, String> {
    windows::overlay_init(&app, &label).ok_or_else(|| format!("未知的覆盖层标签: {label}"))
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
        // 圈选期间显示器被拔掉：整个圈选会话取消
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
    // destroy 在事件循环异步生效，成功后查询会拿到过期状态：按销毁请求结果广播——
    // 失败时保守置 true，按钮保持可用（托盘清除是重试通道）
    let has_mark = windows::destroy_mark(&app).is_err();
    logging::log_err("发送 mark-state 失败", app.emit_to("main", "mark-state", &MarkState { has_mark }));
}

/// 托盘菜单事件分发用
pub(crate) fn handle_tray(app: &AppHandle, id: &str) {
    match id {
        "select" => logging::log_err("发起圈选失败", windows::begin_selection(app)),
        "clear" => clear_mark(app.clone()),
        "show" => windows::show_main(app),
        "quit" => app.exit(0),
        _ => {}
    }
}
