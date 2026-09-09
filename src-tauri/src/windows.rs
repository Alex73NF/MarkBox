//! 窗口编排：圈选覆盖层的创建/销毁会话（重入与代次防护）、标记窗生命周期、显示器相交判定

use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::commands::{MarkState, MonitorRect, OverlayInit, PhysRect};
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
                // tao 对 undecorated+shadow 会按 DWM frame insets 裁客户区（WM_NCCALCSIZE，
                // 左右下≈边框厚度、Win11 顶部 1px）：webview 随客户区偏移，CSS(0,0) 不再是
                // 显示器原点，而前端 toPhys 按原点换算——Windows 上确认出的框会比圈选位置
                // 偏移数像素（同 1ea592a 标记窗灰线纹的机制，当时漏了覆盖层）。关掉后
                // 客户区=外框=显示器矩形，坐标链零偏移
                .shadow(false)
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
            // 先定尺寸后定位再显示：tao macOS 的 setContentSize 锚定窗口左下角，若先定位后改尺寸，
            // 窗口会自定位点向上生长、顶缘越出屏幕，show 时被 AppKit 约束压回可见区顶（菜单栏下），
            // 全屏覆盖层因此整体下移一个菜单栏高、底部越屏——前端 toPhys 按 CSS 原点=显示器
            // 原点换算，确认出的框就比圈选位置偏上同样的量。尺寸先行让定位成为最后一次几何操作
            win.set_size(Size::Physical(PhysicalSize::new(info.width, info.height)))?;
            win.set_position(Position::Physical(PhysicalPosition::new(info.x, info.y)))?;
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

/// 标记窗发光边距（CSS px）。与 mark.html 的 #box inset 及 shared/glow.ts 的阴影外延为
/// 跨文件契约（三处需同步），窗口据此外扩、前端 #box 内缩同样的量把边框画回确认位置
const MARK_GLOW_MARGIN_CSS: f64 = 8.0;

/// 标记窗几何：物理边距按目标屏缩放换算（分数 DPR 下 CSS↔物理各算各的，偏差 <1 物理 px）。
/// i64/u64 升宽：ConfirmPayload 数值未经后端校验，i32 减法与 u32 加法在极值下会回绕/溢出
fn mark_window_geometry(x: i32, y: i32, w: u32, h: u32, scale: f64) -> (i32, i32, u32, u32) {
    // max(0.) 防负；NaN/极值经 float→int 饱和转换归 0（Rust 1.45+ 语义），物理边距只会"偏小"不会发散
    let m = i64::from((MARK_GLOW_MARGIN_CSS * scale).round().max(0.0) as u32);
    let x = (i64::from(x) - m).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let y = (i64::from(y) - m).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let w = (u64::from(w) + (m as u64) * 2).min(u64::from(u32::MAX)) as u32;
    let h = (u64::from(h) + (m as u64) * 2).min(u64::from(u32::MAX)) as u32;
    (x, y, w, h)
}

/// 目标矩形所在显示器的缩放：外扩边距必须按"落位屏"换算——窗口自身的 scale_factor 读到的是
/// 移动前所在屏（复用路径=旧标记屏）或平台默认落位屏（新建路径=通常主屏），而 mark.html 的
/// inset 由 webview 按落位后的屏解释，混合 DPI 多屏下二者失配会把边框画偏。中心点查屏，
/// 跨屏/贴缝矩形退化为任一相交屏，仍查不到保底 1.0（边距偏小，不发散）
fn rect_scale_factor(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> f64 {
    let cx = f64::from(x) + f64::from(w) / 2.0;
    let cy = f64::from(y) + f64::from(h) / 2.0;
    if let Ok(Some(m)) = app.monitor_from_point(cx, cy) {
        return m.scale_factor();
    }
    if let Ok(monitors) = app.available_monitors() {
        for m in &monitors {
            if rect_intersects_any_monitor(x, y, w, h, &[MonitorRect::from(m)]) {
                return m.scale_factor();
            }
        }
    }
    1.0
}

pub(crate) fn spawn_mark(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> tauri::Result<()> {
    // 已有标记窗时换框：销毁旧窗 + 挂起重建（lib.rs 的 mark Destroyed 事件取出 pending_mark
    // 在线程池重建）。不再沿用旧窗 set_position/set_size 改位——Windows 上对已可见的穿透
    // 标记窗这两个调用会静默失效（实测现象：确认后老框不动、新框不出现），且它们只是投递
    // 消息、必返 Ok，失败无从感知；销毁与新建恰是 Windows 上每轮圈选都在验证的可靠原语。
    // 重建不能原地 build：destroy 是投递消息，label 要等 Destroyed 处理完才移出窗口表，
    // 中间 build 必撞 WindowLabelAlreadyExists（6d40d08 的原始竞态），事件驱动天然避开
    if let Some(win) = app.get_webview_window("mark") {
        *app.state::<crate::AppState>().pending_mark.lock().unwrap() = Some(PhysRect { x, y, w, h });
        if let Err(e) = win.destroy() {
            // 销毁请求都没发出去：Destroyed 不会来，清掉挂起矩形避免重建通道滞留
            *app.state::<crate::AppState>().pending_mark.lock().unwrap() = None;
            return Err(e);
        }
        logging::log_err("发送 mark-state 失败", app.emit_to("main", "mark-state", &MarkState { has_mark: true }));
        return Ok(());
    }
    build_mark(app, x, y, w, h)
}

/// 真正落建标记窗（首次确认与 Destroyed 事件触发的换框重建共用）。
/// 几何在此时现算：重建路径从挂起到执行可能跨越显示器拓扑变化，不复用确认时刻的换算
pub(crate) fn build_mark(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> tauri::Result<()> {
    let scale = rect_scale_factor(app, x, y, w, h);
    let (gx, gy, gw, gh) = mark_window_geometry(x, y, w, h, scale);
    let win = WebviewWindowBuilder::new(app, "mark", WebviewUrl::App("mark.html".into()))
        .title("markbox-mark")
        .decorations(false)
        .transparent(true)
        // macOS 系统窗影按内容 alpha 轮廓计算：会沿发光外沿画一圈暗晕、并渗进透明中心
        // （框内外各一条灰线的根因，纯灰背板像素剖面实测），标记窗必须关掉
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .resizable(false)
        .visible(false)
        .build()?;
    win.set_position(Position::Physical(PhysicalPosition::new(gx, gy)))?;
    win.set_size(Size::Physical(PhysicalSize::new(gw, gh)))?;
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
    // 换框的销毁→重建窗口期（旧窗已销毁、新窗未落成）也视为"有标记"：
    // 清除按钮不能在这几十毫秒里失灵，真失败由 Destroyed 事件侧的 emit 校正
    app.get_webview_window("mark").is_some()
        || app.state::<crate::AppState>().pending_mark.lock().unwrap().is_some()
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
    use super::{mark_window_geometry, rect_intersects_any_monitor, MARK_GLOW_MARGIN_CSS};
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

    #[test]
    fn mark_geometry_expands_by_dpr_scaled_margin() {
        // 整数 DPR：边距 = 8 × scale，四周各扩、宽高各加双边距
        assert_eq!(mark_window_geometry(100, 200, 300, 400, 1.0), (92, 192, 316, 416));
        assert_eq!(mark_window_geometry(100, 200, 300, 400, 2.0), (84, 184, 332, 432));
        // 分数 DPR 取整：1.25 × 8 = 10
        assert_eq!(mark_window_geometry(0, 0, 100, 100, 1.25), (-10, -10, 120, 120));
    }

    #[test]
    fn mark_geometry_extreme_rect_saturates_instead_of_wrapping() {
        // 契约同步钉子：CSS 常量若改动（外扩边距），这里失败提醒同步 mark.html / glow.ts
        assert_eq!(MARK_GLOW_MARGIN_CSS, 8.0);
        // 极值饱和而非回绕：i32::MIN - m 不得翻成正，u32::MAX + 2m 不得翻成小数
        let (x, y, w, h) = mark_window_geometry(i32::MIN, i32::MIN, u32::MAX, u32::MAX, 2.0);
        assert!(x < 0 && y < 0);
        assert_eq!((w, h), (u32::MAX, u32::MAX));
    }
}
