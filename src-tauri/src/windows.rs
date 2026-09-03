use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
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
    // 已在圈选中则忽略，防止重复唤起
    if app.webview_windows().keys().any(|l| l.starts_with("overlay-")) {
        return Ok(());
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    let mut infos = Vec::new();
    for (i, m) in app.available_monitors()?.iter().enumerate() {
        let label = format!("overlay-{i}");
        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("overlay.html".into()))
            .title("markbox-overlay")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .build()?;
        win.set_position(Position::Physical(*m.position()))?;
        win.set_size(Size::Physical(*m.size()))?;
        infos.push((label, MonitorInfo {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale_factor: m.scale_factor(),
        }));
    }
    if let Some(state) = app.try_state::<crate::AppState>() {
        *state.monitors.lock().unwrap() = infos;
    }
    Ok(())
}

pub fn end_selection(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with("overlay-") {
            let _ = win.destroy();
        }
    }
}

pub fn spawn_mark(app: &AppHandle, x: i32, y: i32, w: u32, h: u32) -> tauri::Result<()> {
    destroy_mark(app);
    let win = WebviewWindowBuilder::new(app, "mark", WebviewUrl::App("mark.html".into()))
        .title("markbox-mark")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focusable(false)
        .resizable(false)
        .build()?;
    win.set_position(Position::Physical(PhysicalPosition::new(x, y)))?;
    win.set_size(Size::Physical(PhysicalSize::new(w, h)))?;
    win.set_ignore_cursor_events(true)?;
    let _ = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": true }));
    Ok(())
}

pub fn destroy_mark(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("mark") {
        let _ = win.destroy();
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
