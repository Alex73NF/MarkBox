use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::commands::OverlayInit;

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

pub fn begin_selection(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<crate::AppState>();
    {
        let mut selecting = state.selecting.lock().unwrap();
        if *selecting {
            return Ok(()); // 已在圈选中则忽略，防止重复唤起（按钮双击/主窗口+托盘并发）
        }
        *selecting = true;
    }
    let result = (|| -> tauri::Result<()> {
        // 先枚举显示器再隐主窗口：枚举失败不能把用户丢在黑屏里
        let mut infos = Vec::new();
        for (i, m) in app.available_monitors()?.iter().enumerate() {
            infos.push((format!("overlay-{i}"), MonitorInfo {
                x: m.position().x,
                y: m.position().y,
                width: m.size().width,
                height: m.size().height,
                scale_factor: m.scale_factor(),
            }));
        }
        if let Some(main) = app.get_webview_window("main") {
            if let Err(e) = main.hide() {
                eprintln!("[markbox] 隐藏主窗口失败: {e}");
            }
        }
        // 先存后建：overlay 窗口一创建就可能回调 overlay_ready，必须保证 monitors 已就绪
        *state.monitors.lock().unwrap() = infos.clone();
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
            if let Err(e) = win.set_focus() {
                eprintln!("[markbox] overlay 设置键盘焦点失败: {e}");
            }
        }
        Ok(())
    })();
    if result.is_err() {
        // 创建中途失败：清掉已建出的 overlay、恢复主窗口，错误继续上抛
        end_selection(app);
        show_main(app);
    }
    *state.selecting.lock().unwrap() = false;
    result
}

/// 显示并聚焦主窗口（单实例二次唤起 / 托盘 / 圈选失败恢复共用）
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if let Err(e) = w.show() {
            eprintln!("[markbox] 显示主窗口失败: {e}");
        }
        if let Err(e) = w.set_focus() {
            eprintln!("[markbox] 聚焦主窗口失败: {e}");
        }
    }
}

pub fn end_selection(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with("overlay-") {
            if let Err(e) = win.destroy() {
                eprintln!("[markbox] 销毁 {label} 失败: {e}");
            }
        }
    }
}

pub fn spawn_mark(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> tauri::Result<()> {
    destroy_mark(app);
    let build = || {
        WebviewWindowBuilder::new(app, "mark", WebviewUrl::App("mark.html".into()))
            .title("markbox-mark")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .focusable(false)
            .resizable(false)
            .visible(false)
            .build()
    };
    // 防回车重复确认的并发残余：build 失败且 "mark" 仍存在（上一次异步 destroy 未完成）
    // → 再销毁一次并重试一次，仍失败才向上抛错
    let win = match build() {
        Ok(win) => win,
        Err(build_err) => {
            if app.get_webview_window("mark").is_none() {
                return Err(build_err);
            }
            destroy_mark(app);
            build()?
        }
    };
    win.set_position(Position::Physical(PhysicalPosition::new(x, y)))?;
    win.set_size(Size::Physical(PhysicalSize::new(w, h)))?;
    win.set_ignore_cursor_events(true)?;
    win.show()?;
    if let Err(e) = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": true })) {
        eprintln!("[markbox] 发送 mark-state 失败: {e}");
    }
    Ok(())
}

pub fn destroy_mark(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("mark") {
        if let Err(e) = win.destroy() {
            eprintln!("[markbox] 销毁标记窗失败: {e}");
        }
    }
}

pub fn mark_exists(app: &AppHandle) -> bool {
    app.get_webview_window("mark").is_some()
}

pub fn emit_mark_state(app: &AppHandle) {
    let _ = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": mark_exists(app) }));
}

/// 显示器热插拔保护：确认时 rect 必须仍落在某个现存显示器上
pub fn rect_on_existing_monitor(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> bool {
    let Ok(monitors) = app.available_monitors() else { return false };
    monitors.iter().any(|m| {
        let (mx, my) = (m.position().x, m.position().y);
        let (mw, mh) = (m.size().width as i32, m.size().height as i32);
        x < mx + mw && x + w as i32 > mx && y < my + mh && y + h as i32 > my
    })
}

pub fn overlay_init(app: &AppHandle, label: &str) -> Option<OverlayInit> {
    let state = app.try_state::<crate::AppState>()?;
    let monitors = state.monitors.lock().unwrap();
    monitors.iter().find(|(l, _)| l == label).map(|(_, info)| OverlayInit {
        monitor: crate::commands::MonitorRect {
            x: info.x, y: info.y, width: info.width, height: info.height,
        },
        scale_factor: info.scale_factor,
    })
}
