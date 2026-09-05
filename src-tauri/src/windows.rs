//! 窗口编排：圈选覆盖层的创建/销毁会话（重入与代次防护）、标记窗生命周期、显示器相交判定

use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::commands::{MarkState, MonitorRect, OverlayInit};
use crate::logging;

/// 圈选互斥标记的 RAII 复位：无论正常返回还是 panic 展开都恢复 false，防止标记滞留后圈选永久静默失效
struct SelectingGuard<'a>(&'a Mutex<bool>);

impl Drop for SelectingGuard<'_> {
    fn drop(&mut self) {
        *self.0.lock().unwrap() = false;
    }
}

pub(crate) fn begin_selection(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<crate::AppState>();
    {
        let mut selecting = state.selecting.lock().unwrap();
        // 创建进行中（selecting）或覆盖层会话仍存活（草稿/调整阶段）都视为已在圈选中：
        // 静默忽略，防止按钮双击/主窗口+托盘并发重复唤起，也防再次 build 撞 label 拆掉进行中的选区
        if *selecting || app.webview_windows().keys().any(|l| l.starts_with("overlay-")) {
            return Ok(());
        }
        *selecting = true;
    }
    let _guard = SelectingGuard(&state.selecting);
    let result = (|| -> tauri::Result<()> {
        // 先枚举显示器再隐主窗口：枚举失败不能把用户丢在黑屏里
        let infos: Vec<(String, MonitorRect)> = app
            .available_monitors()?
            .iter()
            .enumerate()
            .map(|(i, m)| (format!("overlay-{i}"), MonitorRect::from(m)))
            .collect();
        if infos.is_empty() {
            // 一台显示器都没有：保留主窗口并报错，别把用户丢进只剩托盘的黑屏
            return Err(tauri::Error::Io(std::io::Error::other("没有可用显示器")));
        }
        hide_main(app);
        // 先存后建：overlay 窗口一创建就可能回调 overlay_ready，必须保证 monitors 已就绪；
        // 就绪集合同步清空，看门狗只统计本会话的 overlay_ready
        *state.monitors.lock().unwrap() = infos.clone();
        state.ready_overlays.lock().unwrap().clear();
        // 创建代次快照：此后 end_selection 一旦递增即说明本会话已被取消/确认，循环要静默收场。
        // 圈选命令已 async 化（Windows 主线程创建窗口死锁，见 commands.rs 文件头），
        // 本函数运行在线程池而 end_selection 可能在主线程/看门狗线程，Acquire 配对增量侧的 AcqRel
        let gen = state.selection_gen.load(Ordering::Acquire);
        let cursor = app.cursor_position().ok();
        let mut last: Option<WebviewWindow> = None;
        let mut focus_target: Option<WebviewWindow> = None;
        for (label, info) in &infos {
            let win = WebviewWindowBuilder::new(app, label.as_str(), WebviewUrl::App("overlay.html".into()))
                .title("markbox-overlay")
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .build()?;
            // 多屏逐个 build 期间用户已在先建的屏上取消/确认（spec：任何阶段取消都不留框）：
            // 销毁刚建出的窗口并中止，不再把后续屏幕的覆盖层弹给已取消的用户
            if state.selection_gen.load(Ordering::Acquire) != gen {
                logging::log_err(&format!("销毁 {label} 失败"), win.destroy());
                return Ok(());
            }
            win.set_position(Position::Physical(PhysicalPosition::new(info.x, info.y)))?;
            win.set_size(Size::Physical(PhysicalSize::new(info.width, info.height)))?;
            win.show()?;
            // 键盘焦点是 Esc/Enter 取消/确认的前提：优先光标所在屏，否则兜底最后建出的
            if cursor.is_some_and(|c| {
                c.x >= f64::from(info.x) && c.x < f64::from(info.x) + f64::from(info.width)
                    && c.y >= f64::from(info.y) && c.y < f64::from(info.y) + f64::from(info.height)
            }) {
                focus_target = Some(win.clone());
            }
            last = Some(win);
        }
        if let Some(win) = focus_target.or(last) {
            logging::log_err("overlay 设置键盘焦点失败", win.set_focus());
        }
        spawn_ready_watchdog(app, gen, &infos);
        Ok(())
    })();
    if result.is_err() {
        // 创建中途失败：清掉已建出的 overlay、恢复主窗口，错误继续上抛
        end_selection(app);
        show_main(app);
    }
    result
}

/// 就绪看门狗：窗口已 show 而 overlay_ready 一直未到（webview 崩溃/脚本未执行）时，
/// 覆盖层是无退出通道的全屏输入拦截层——超时自动收场并还原主窗。
/// 代次快照防误伤：会话正常结束（取消/确认/兜底）后 gen 已变，直接退出。
/// 注意就绪与退出监听的因果：overlay_ready 在 overlay.ts 模块末尾才发起，
/// 监听注册（模块顶部）必然先于它——"未就绪"即"监听不存在"，看门狗正是为此而设
fn spawn_ready_watchdog(app: &AppHandle, gen: u64, infos: &[(String, MonitorRect)]) {
    let handle = app.clone();
    let expected: Vec<String> = infos.iter().map(|(l, _)| l.clone()).collect();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        let state = handle.state::<crate::AppState>();
        if state.selection_gen.load(Ordering::Acquire) != gen {
            return;
        }
        let ready = state.ready_overlays.lock().unwrap();
        let stuck: Vec<&String> = expected.iter().filter(|l| !ready.contains(*l)).collect();
        drop(ready);
        if stuck.is_empty() {
            return;
        }
        logging::log_error(&format!("覆盖层 {stuck:?} 5 秒未就绪（前端未加载？），圈选自动收场"));
        end_selection(&handle);
        show_main(&handle);
    });
}

/// 隐藏主窗口（圈选启动/关闭到托盘共用）
pub(crate) fn hide_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        logging::log_err("隐藏主窗口失败", w.hide());
    }
}

/// 显示并聚焦主窗口（单实例二次唤起 / 托盘 / macOS Dock 图标 Reopen / 圈选失败恢复共用）
pub(crate) fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        logging::log_err("显示主窗口失败", w.show());
        logging::log_err("聚焦主窗口失败", w.set_focus());
    }
}

pub(crate) fn end_selection(app: &AppHandle) {
    // 先递增代次再销毁：正在逐屏 build 的 begin_selection 据此发现会话已被取消并停止续建。
    // AcqRel：增量侧释放，配对 begin_selection/看门狗的 Acquire 载入，保证跨线程可见
    let state = app.state::<crate::AppState>();
    state.selection_gen.fetch_add(1, Ordering::AcqRel);
    for (label, win) in app.webview_windows() {
        if label.starts_with("overlay-") {
            logging::log_err(&format!("销毁 {label} 失败"), win.destroy());
        }
    }
}

pub(crate) fn spawn_mark(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> tauri::Result<()> {
    // 已有标记窗优先复用（改位置/尺寸即可，mark.html 的 #box 是 inset:0 相对布局，内容自适应）：
    // destroy 是投递给事件循环的异步消息，label 要等 Destroyed 事件处理完才从窗口表移除；
    // 在同一线程里 destroy→build 中间事件循环一次都没跑，build 必撞 WindowLabelAlreadyExists
    // （重试同样来不及），结果就是旧框被拆、新框建不出。复用从根上消除该竞态，还免去 webview 重载。
    if let Some(win) = app.get_webview_window("mark") {
        let moved = win
            .set_position(Position::Physical(PhysicalPosition::new(x, y)))
            .and_then(|_| win.set_size(Size::Physical(PhysicalSize::new(w, h))));
        if moved.is_ok() {
            win.show()?;
            logging::log_err("发送 mark-state 失败", app.emit_to("main", "mark-state", &MarkState { has_mark: true }));
            return Ok(());
        }
        // 极小窗口：复用中途窗口刚被销毁（清除标记后瞬间确认），落回新建；若 label 仍在
        // 销毁流程中导致 build 失败，错误上抛由调用方记录并还回主窗口
        logging::log_err("复用旧标记窗失败，改为重建", moved);
    }
    let win = WebviewWindowBuilder::new(app, "mark", WebviewUrl::App("mark.html".into()))
        .title("markbox-mark")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .resizable(false)
        .visible(false)
        .build()?;
    win.set_position(Position::Physical(PhysicalPosition::new(x, y)))?;
    win.set_size(Size::Physical(PhysicalSize::new(w, h)))?;
    win.set_ignore_cursor_events(true)?;
    win.show()?;
    logging::log_err("发送 mark-state 失败", app.emit_to("main", "mark-state", &MarkState { has_mark: true }));
    Ok(())
}

pub(crate) fn destroy_mark(app: &AppHandle) -> tauri::Result<()> {
    match app.get_webview_window("mark") {
        Some(win) => win.destroy(),
        None => Ok(()),
    }
}

pub(crate) fn mark_exists(app: &AppHandle) -> bool {
    app.get_webview_window("mark").is_some()
}

pub(crate) fn emit_mark_state(app: &AppHandle) {
    logging::log_err("发送 mark-state 失败", app.emit_to("main", "mark-state", &MarkState { has_mark: mark_exists(app) }));
}

/// 显示器热插拔保护：确认时 rect 必须仍落在某个现存显示器上
pub(crate) fn rect_on_existing_monitor(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> bool {
    let Ok(monitors) = app.available_monitors() else { return false };
    let rects: Vec<MonitorRect> = monitors.iter().map(MonitorRect::from).collect();
    rect_intersects_any_monitor(x, y, w, h, &rects)
}

/// 判定物理矩形是否与任一显示器相交（含负原点屏）。i64 运算：ConfirmPayload 是唯一未经
/// 后端校验的数值入口，u32 尺寸 as i32 可回绕成负值、直接相加在 release（无 overflow-checks）
/// 下可溢出，统一升宽消除；0 尺寸显式拒绝（原先靠相交判定隐式排除）
pub(crate) fn rect_intersects_any_monitor(x: i32, y: i32, w: u32, h: u32, monitors: &[MonitorRect]) -> bool {
    let (x, y, w, h) = (i64::from(x), i64::from(y), i64::from(w), i64::from(h));
    monitors.iter().any(|m| {
        let (mx, my) = (i64::from(m.x), i64::from(m.y));
        let (mw, mh) = (i64::from(m.width), i64::from(m.height));
        w > 0 && h > 0 && x < mx + mw && x + w > mx && y < my + mh && y + h > my
    })
}

pub(crate) fn overlay_init(app: &AppHandle, label: &str) -> Option<OverlayInit> {
    let state = app.try_state::<crate::AppState>()?;
    let monitors = state.monitors.lock().unwrap();
    monitors.iter().find(|(l, _)| l == label).map(|(_, info)| OverlayInit { monitor: *info })
}

#[cfg(test)]
mod tests {
    use super::rect_intersects_any_monitor;
    use crate::commands::MonitorRect;

    fn monitor(x: i32, y: i32, w: u32, h: u32) -> MonitorRect {
        MonitorRect { x, y, width: w, height: h }
    }

    #[test]
    fn rect_inside_monitor_intersects() {
        let ms = [monitor(0, 0, 1920, 1080)];
        assert!(rect_intersects_any_monitor(100, 100, 50, 50, &ms));
    }

    #[test]
    fn rect_outside_monitor_does_not_intersect() {
        let ms = [monitor(0, 0, 1920, 1080)];
        assert!(!rect_intersects_any_monitor(2000, 100, 50, 50, &ms));
    }

    #[test]
    fn adjacent_edge_is_not_intersection() {
        // 显示器区间左闭右开 [mx, mx+mw)：恰贴边缘（相等）不算相交
        let ms = [monitor(0, 0, 1920, 1080)];
        assert!(!rect_intersects_any_monitor(1920, 100, 50, 50, &ms)); // 右缘外贴：x == mx+mw
        assert!(!rect_intersects_any_monitor(-50, 100, 50, 50, &ms)); // 左缘外贴：x+w == mx
        assert!(!rect_intersects_any_monitor(100, -50, 50, 50, &ms)); // 顶缘外贴：y+h == my
        assert!(!rect_intersects_any_monitor(100, 1080, 50, 50, &ms)); // 底缘外贴：y == my+mh
    }

    #[test]
    fn negative_origin_monitor_intersects() {
        let ms = [monitor(-1920, -200, 1920, 1080)];
        assert!(rect_intersects_any_monitor(-1900, -180, 100, 100, &ms));
        assert!(!rect_intersects_any_monitor(-1900, 1000, 100, 100, &ms));
    }

    #[test]
    fn zero_size_rect_is_rejected() {
        let ms = [monitor(0, 0, 1920, 1080)];
        assert!(!rect_intersects_any_monitor(100, 100, 0, 100, &ms));
        assert!(!rect_intersects_any_monitor(100, 100, 100, 0, &ms));
    }

    #[test]
    fn extreme_values_do_not_wrap_or_overflow() {
        // u32::MAX as i32 会回绕成 -1；i64 升宽后仅按几何事实判定（不相交）
        let ms = [monitor(0, 0, 1920, 1080)];
        assert!(!rect_intersects_any_monitor(i32::MAX, i32::MAX, u32::MAX, u32::MAX, &ms));
        // 常规坐标下极端尺寸仍与屏幕相交
        assert!(rect_intersects_any_monitor(0, 0, u32::MAX, u32::MAX, &ms));
    }
}
