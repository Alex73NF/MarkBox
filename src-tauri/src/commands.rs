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

// 命令线程模型（Windows 限制是第一约束）：
// 1. 同步命令与托盘/菜单事件回调都在主线程执行，而 Windows 主线程上创建 WebView 窗口会死锁
//    （wry#583，tauri 对 WebviewWindowBuilder 的官方文档警告）。因此所有创建窗口的命令
//    （start_selection 建 overlay、confirm_selection 建标记窗）必须 async：命令转到运行时
//    线程池执行，窗口创建/定位/显示经事件循环代理按投递顺序（FIFO）串行生效，时序仍确定。
// 2. 设置读写含磁盘 IO，同样 async 移出主线程，避免写盘冻结托盘与全部窗口。
// 3. 取消/清除/就绪上报只读状态或只销毁窗口：destroy 本身就是向事件循环投递消息、
//    任意线程可安全调用，保持同步无死锁面。
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

// async：本命令创建 overlay 窗口，Windows 主线程内创建会死锁（见文件头注释 1）
#[tauri::command(async)]
pub(crate) fn start_selection(app: AppHandle) -> Result<(), String> {
    windows::begin_selection(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn overlay_ready(app: AppHandle, state: tauri::State<AppState>, label: String) -> Result<OverlayInit, String> {
    let init = windows::overlay_init(&app, &label).ok_or_else(|| format!("未知的覆盖层标签: {label}"))?;
    // 标记就绪供圈选看门狗判定（见 windows.rs spawn_ready_watchdog）
    state.ready_overlays.lock().unwrap().insert(label);
    Ok(init)
}

// async：确认链路要创建标记窗，同 start_selection 的 Windows 死锁规避（见文件头注释 1）
#[tauri::command(async)]
pub(crate) fn confirm_selection(app: AppHandle, payload: ConfirmPayload) -> Result<(), String> {
    let r = &payload.rect;
    let ok = windows::rect_on_existing_monitor(&app, r.x, r.y, r.w, r.h);
    windows::end_selection(&app);
    if ok {
        if let Err(e) = windows::spawn_mark(&app, r.x, r.y, r.w, r.h) {
            // 失败必留痕（markbox.log）： rect 一并记录，便于远程定位是几何问题还是窗口问题
            logging::log_error(&format!("创建标记窗失败 rect=({},{},{},{}): {e}", r.x, r.y, r.w, r.h));
            windows::show_main(&app); // 标记窗建失败也要把主窗口还给用户，别丢在黑屏里
            return Err(e.to_string());
        }
    } else {
        // 圈选期间显示器被拔掉：整个圈选会话取消。同样把主窗还回去，
        // 否则覆盖层已销毁、标记未建、主窗仍隐藏，用户面对的是"全没了"的无窗状态
        logging::log_error(&format!(
            "确认的选区不在任何现存显示器上，圈选已取消 rect=({},{},{},{})",
            r.x, r.y, r.w, r.h
        ));
        windows::show_main(&app);
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
        "select" => {
            // 托盘菜单事件同样在主线程派发，直接创建窗口触发同一死锁（wry#583）：
            // 移到运行时阻塞线程池发起，错误在线程内记日志（与按钮路径共用 begin_selection）
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                logging::log_err("发起圈选失败", windows::begin_selection(&app));
            });
        }
        "clear" => clear_mark(app.clone()),
        "show" => windows::show_main(app),
        "quit" => app.exit(0),
        _ => {}
    }
}
