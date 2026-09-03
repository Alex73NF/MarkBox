# MarkBox MVP Implementation Plan

> **状态：已全部完成 @ v0.1.0。** 历史文档，勾选项不再逐个回填。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 MarkBox v0.1.0——托盘常驻的屏幕边框标记工具：主界面/托盘唤起微信截图式圈选（拖拽→手柄调整→✓/回车确认），确认后留下点击穿透的置顶边框，单框替换。

**Architecture:** Tauri 2 单进程：Rust 侧负责托盘、多显示器 overlay 窗口的编排、标记窗的创建/销毁与设置持久化；前端 3 个 Vite 多入口页面（main 主窗口兼设置、overlay 圈选层、mark 标记框），页面间不直接通信，全部经 Rust 命令与事件中转。窗口坐标一律物理像素。

**Tech Stack:** Tauri 2（Rust stable）、Vanilla TypeScript（strict）、Vite 6 多入口、Vitest（前端纯函数）、cargo test（Rust 单元）、pnpm 10、GitHub Actions（windows-latest 出包）。

**Spec:** `docs/superpowers/specs/2026-09-03-markbox-design.md`

## Global Constraints

- 仓库：`/Users/alex/A_GitRepos/MarkBox`，远程 `git@github.com:Alex73NF/MarkBox.git`，分支 `main`，包管理器 `pnpm`。
- 目标平台 Windows；开发调试在 macOS（`pnpm tauri dev`），GUI 行为最终以 Windows 验收为准。
- 禁止引入：UI 框架（React/Vue 等）、全局快捷键插件、自启插件。唯一 Tauri 插件：`tauri-plugin-single-instance`。
- 所有窗口定位/尺寸使用 Tauri 的 `Position::Physical` / `Size::Physical`（物理像素）。
- Rust 与前端之间 JSON 字段一律 camelCase（serde `rename_all = "camelCase"`）。
- 设置默认值：`borderColor = "#FF4D4F"`、`borderWidth = 3`（合法 1–10）、`borderRadius = 0`（合法 0–16）；颜色必须匹配 `^#[0-9A-Fa-f]{6}$`，非法值回退默认。
- 命令名固定：`get_settings`、`save_settings`、`start_selection`、`overlay_ready`、`confirm_selection`、`cancel_selection`、`clear_mark`。
- 事件名固定：`mark-state`（负载 `{ hasMark: boolean }`，发往 `main`）、`settings-updated`（负载 Settings，发往 `mark`）。
- 窗口 label 固定：`main`、`overlay-{i}`（i 为显示器序号）、`mark`。
- 每个任务结束必须：自动化检查通过（`pnpm build`、`pnpm test`、`cargo test` 中该任务涉及的项）+ commit。
- Conventional commits，中文描述，如 `feat: 圈选覆盖层完整交互`。

---

### Task 1: 项目脚手架与多入口结构

**Files:**
- Create: 从官方模板拷贝（package.json、src-tauri/ 全套含 icons、index.html 等）
- Create: `overlay.html`、`mark.html`、`src/main/main.ts`、`src/overlay/overlay.ts`、`src/mark/mark.ts`、`src/shared/types.ts`
- Modify: `vite.config.ts`、`src-tauri/tauri.conf.json`、`package.json`、`src-tauri/Cargo.toml`
- Delete: 模板的 `src/main.ts`、`src/assets/`

**Interfaces:**
- Produces: 可运行的 `pnpm tauri dev`（三个入口占位页）；`src/shared/types.ts` 导出后续任务依赖的全部类型。

- [ ] **Step 1: 确认工具链**

```bash
node -v    # ≥20
pnpm -v    # ≥10
rustc --version  # 若未安装: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

- [ ] **Step 2: 用官方模板生成脚手架并合入仓库**

```bash
cd /Users/alex/A_GitRepos
pnpm create tauri-app@latest markbox-scaffold --template vanilla-ts --manager pnpm --yes
rsync -a --exclude .git --exclude node_modules markbox-scaffold/ MarkBox/
rm -rf markbox-scaffold
cd MarkBox && pnpm install
```

- [ ] **Step 3: 改写为三入口结构**

删除 `src/main.ts`、`src/assets/`，然后创建：

`src/shared/types.ts`（后续任务的契约，一次性给全）：

```typescript
export interface Settings {
  borderColor: string;
  borderWidth: number; // 1-10
  borderRadius: number; // 0-16
}
export interface MonitorRect { x: number; y: number; width: number; height: number } // 物理像素，全局坐标
export interface OverlayInit { monitor: MonitorRect; scaleFactor: number }
export interface PhysRect { x: number; y: number; w: number; h: number } // 物理像素，全局坐标
export interface ConfirmPayload { label: string; rect: PhysRect }
export interface MarkState { hasMark: boolean }
```

`index.html`：

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <title>MarkBox</title>
    <style>
      body { font-family: system-ui, sans-serif; margin: 0; padding: 16px; color: #222; }
      button { padding: 6px 14px; margin-right: 8px; cursor: pointer; }
    </style>
  </head>
  <body>
    <h1>MarkBox</h1>
    <button id="start">开始圈选</button>
    <button id="clear" disabled>清除标记</button>
    <script type="module" src="/src/main/main.ts"></script>
  </body>
</html>
```

`overlay.html`：

```html
<!doctype html>
<html lang="zh-CN">
  <head><meta charset="UTF-8" /><title>overlay</title>
    <style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:transparent}#tip{position:fixed;inset:0;display:grid;place-items:center;color:#fff;font:14px system-ui}</style>
  </head>
  <body><div id="tip">overlay 占位</div><script type="module" src="/src/overlay/overlay.ts"></script></body>
</html>
```

`mark.html`：

```html
<!doctype html>
<html lang="zh-CN">
  <head><meta charset="UTF-8" /><title>mark</title>
    <style>html,body{margin:0;width:100%;height:100%;background:transparent}#box{position:fixed;inset:0;box-sizing:border-box;border:3px solid #FF4D4F}</style>
  </head>
  <body><div id="box"></div><script type="module" src="/src/mark/mark.ts"></script></body>
</html>
```

`src/main/main.ts`、`src/overlay/overlay.ts`、`src/mark/mark.ts` 先各放一行占位：

```typescript
console.log('entry placeholder');
```

`vite.config.ts`（覆盖模板的）：

```typescript
import { defineConfig } from 'vite';

export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'chrome110',
    rollupOptions: {
      input: { main: 'index.html', overlay: 'overlay.html', mark: 'mark.html' },
    },
  },
});
```

- [ ] **Step 4: 改标识与依赖**

`package.json`：`"name": "markbox"`。
`src-tauri/tauri.conf.json`：`productName` 改 `"MarkBox"`，`identifier` 改 `"com.alex73nf.markbox"`，`app.windows[0]` 改为：

```json
{ "label": "main", "title": "MarkBox", "width": 420, "height": 380, "resizable": false, "center": true }
```

`src-tauri/Cargo.toml`：`name` 改 `"markbox"`，`tauri` 依赖加 tray 特性，并加插件依赖：

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-single-instance = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

（模板已有的 `tauri-build`、`[lib]` 段保留不动。）

确认忽略文件（模板一般已带，缺则补）：根 `.gitignore` 含 `node_modules`、`dist`；`src-tauri/.gitignore` 含 `/target`、`/gen`。

- [ ] **Step 5: 自动化验证（不开 GUI）**

```bash
pnpm build        # 三个入口都产出 dist/*.html
cd src-tauri && cargo check && cd ..
```

Expected: vite 构建成功且 `dist/` 下有 `index.html overlay.html mark.html`；cargo 无错误。

- [ ] **Step 6: 手动验证（有显示环境时）**

Run: `pnpm tauri dev`
Expected: 弹出 420×380 的 MarkBox 主窗口（占位按钮）；无白屏报错。

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: Tauri2 脚手架与 Vite 三入口结构"
```

---

### Task 2: Rust 设置模块（TDD）

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/lib.rs`（挂载模块、App 状态、`get_settings`/`save_settings` 命令并注册）

**Interfaces:**
- Produces: `Settings { border_color: String, border_width: u8, border_radius: u8 }`（serde camelCase）；命令 `get_settings() -> Settings`、`save_settings(settings: Settings) -> Settings`（规范化后返回）；Rust 内部供后续任务调用 `settings::normalize(&Settings) -> Settings`、`settings::settings_path(&AppHandle) -> PathBuf`。

- [ ] **Step 1: 写失败测试**

`src-tauri/src/settings.rs`：

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub border_color: String,
    pub border_width: u8,
    pub border_radius: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self { border_color: "#FF4D4F".into(), border_width: 3, border_radius: 0 }
    }
}

/// 非法值回退默认（颜色格式、粗细 1-10、圆角 0-16）
pub fn normalize(s: &Settings) -> Settings {
    let color_ok = s.border_color.len() == 7
        && s.border_color.starts_with('#')
        && s.border_color[1..].chars().all(|c| c.is_ascii_hexdigit());
    Settings {
        border_color: if color_ok { s.border_color.clone() } else { "#FF4D4F".into() },
        border_width: s.border_width.clamp(1, 10),
        border_radius: s.border_radius.clamp(0, 16),
    }
}

pub fn load_from(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|txt| serde_json::from_str::<Settings>(&txt).ok())
        .map(|s| normalize(&s))
        .unwrap_or_default()
}

pub fn save_to(path: &Path, s: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(s).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("markbox-test-{}-{}.json", name, std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn default_values() {
        let s = Settings::default();
        assert_eq!(s.border_color, "#FF4D4F");
        assert_eq!(s.border_width, 3);
        assert_eq!(s.border_radius, 0);
    }

    #[test]
    fn normalize_fixes_invalid() {
        let s = normalize(&Settings { border_color: "red".into(), border_width: 99, border_radius: 99 });
        assert_eq!(s.border_color, "#FF4D4F");
        assert_eq!(s.border_width, 10);
        assert_eq!(s.border_radius, 16);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let p = tmp_path("roundtrip");
        let s = Settings { border_color: "#00C2FF".into(), border_width: 5, border_radius: 8 };
        save_to(&p, &s).unwrap();
        assert_eq!(load_from(&p), s);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn load_missing_or_broken_file_falls_back_to_default() {
        assert_eq!(load_from(Path::new("/nonexistent/markbox/settings.json")), Settings::default());
        let p = tmp_path("broken");
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(load_from(&p), Settings::default());
        let _ = std::fs::remove_file(&p);
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test`
Expected: 4 个测试全 PASS（此时模块尚未挂到 lib，先 `mod settings;` 加进 lib.rs 再跑，见 Step 3 同时进行亦可——标准做法：先在 lib.rs 声明 `mod settings;` 再跑测试，红→绿流程以 Step 3 完成后全绿为准）。

- [ ] **Step 3: 挂载模块、状态与命令**

`src-tauri/src/lib.rs`（在模板基础上改，`run()` 其余部分保持模板原样，本任务只加设置相关；后续任务再扩展）：

```rust
mod settings;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct AppState {
    pub settings: Mutex<settings::Settings>,
}

pub fn settings_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_config_dir().unwrap().join("settings.json")
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: settings::Settings) -> Result<settings::Settings, String> {
    let normalized = settings::normalize(&settings);
    settings::save_to(&settings_path(&app), &normalized).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = normalized.clone();
    let _ = app.emit_to("mark", "settings-updated", &normalized);
    Ok(normalized)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            let loaded = settings::load_from(&settings_path(app.handle()));
            app.manage(AppState { settings: Mutex::new(loaded) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_settings, save_settings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

同时确认 `src-tauri/src/main.rs` 调用 `markbox_lib::run()`（模板默认即是，`Cargo.toml` 中 `[lib] name` 保持模板的 `markbox_lib`）。

- [ ] **Step 4: 全部测试与构建通过**

Run: `cd src-tauri && cargo test && cargo check`，然后 `cd .. && pnpm build`
Expected: 全部 PASS / 成功。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: 设置模块（加载/规范化/持久化）与 get/save_settings 命令"
```

---

### Task 3: 前端几何模块（TDD / Vitest）

**Files:**
- Create: `src/shared/geometry.ts`、`src/shared/geometry.test.ts`
- Modify: `package.json`（加 vitest devDep 与 `test` 脚本）

**Interfaces:**
- Produces（全部 CSS 像素纯函数，Task 6 的 overlay 依赖）:
  - `interface Rect { x: number; y: number; w: number; h: number }`
  - `type Handle = 'nw'|'n'|'ne'|'e'|'se'|'s'|'sw'|'w'`
  - `normalizeDrag(sx, sy, cx, cy): Rect`
  - `applyResize(r, handle, dx, dy, min: {w:number;h:number}, max: Rect): Rect`
  - `applyMove(r, dx, dy, max: Rect): Rect`
  - `clampRect(r, max): Rect`

- [ ] **Step 1: 安装 vitest 并加脚本**

```bash
pnpm add -D vitest
```

`package.json` scripts 加：`"test": "vitest run"`。

- [ ] **Step 2: 写失败测试**

`src/shared/geometry.test.ts`：

```typescript
import { describe, expect, it } from 'vitest';
import { applyMove, applyResize, clampRect, normalizeDrag, type Rect } from './geometry';

const B: Rect = { x: 0, y: 0, w: 1000, h: 800 };
const MIN = { w: 10, h: 10 };

describe('normalizeDrag', () => {
  it('向右下拖出常规矩形', () => {
    expect(normalizeDrag(100, 100, 300, 200)).toEqual({ x: 100, y: 100, w: 200, h: 100 });
  });
  it('向左上拖自动归一化', () => {
    expect(normalizeDrag(300, 200, 100, 100)).toEqual({ x: 100, y: 100, w: 200, h: 100 });
  });
});

describe('applyResize', () => {
  const r: Rect = { x: 100, y: 100, w: 200, h: 100 };
  it('se 手柄向右下扩', () => {
    expect(applyResize(r, 'se', 50, 20, MIN, B)).toEqual({ x: 100, y: 100, w: 250, h: 120 });
  });
  it('nw 手柄向左上收（右下角锚定）', () => {
    expect(applyResize(r, 'nw', -50, -30, MIN, B)).toEqual({ x: 50, y: 70, w: 250, h: 130 });
  });
  it('w 手柄拖过右边界时钳到最小宽', () => {
    expect(applyResize(r, 'w', 500, 0, MIN, B)).toEqual({ x: 290, y: 100, w: 10, h: 100 });
  });
  it('e 手柄不越屏', () => {
    const out = applyResize(r, 'e', 5000, 0, MIN, B);
    expect(out.x + out.w).toBe(1000);
  });
  it('s 手柄不小于最小高', () => {
    const out = applyResize(r, 's', 0, -500, MIN, B);
    expect(out.h).toBe(10);
  });
});

describe('applyMove', () => {
  it('整体平移并钳在屏内', () => {
    const r: Rect = { x: 990, y: 790, w: 50, h: 50 };
    expect(applyMove(r, 30, 30, B)).toEqual({ x: 950, y: 750, w: 50, h: 50 });
  });
});

describe('clampRect', () => {
  it('越界矩形与屏求交', () => {
    expect(clampRect({ x: -50, y: -50, w: 200, h: 200 }, B)).toEqual({ x: 0, y: 0, w: 150, h: 150 });
  });
});
```

- [ ] **Step 3: 跑测试确认失败**

Run: `pnpm test`
Expected: FAIL（`geometry.ts` 不存在）。

- [ ] **Step 4: 实现**

`src/shared/geometry.ts`：

```typescript
export interface Rect { x: number; y: number; w: number; h: number }
export type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** 起点(sx,sy)拖到(cx,cy)的归一化矩形（正 w/h） */
export function normalizeDrag(sx: number, sy: number, cx: number, cy: number): Rect {
  return { x: Math.min(sx, cx), y: Math.min(sy, cy), w: Math.abs(cx - sx), h: Math.abs(cy - sy) };
}

/** 手柄拖动：对边锚定，强制 min 尺寸并钳制在 max（屏幕）内 */
export function applyResize(r: Rect, handle: Handle, dx: number, dy: number, min: { w: number; h: number }, max: Rect): Rect {
  const a = { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h };
  let { left, top, right, bottom } = a;
  if (handle.includes('w')) left = clamp(a.left + dx, max.x, a.right - min.w);
  if (handle.includes('e')) right = clamp(a.right + dx, a.left + min.w, max.x + max.w);
  if (handle.includes('n')) top = clamp(a.top + dy, max.y, a.bottom - min.h);
  if (handle.includes('s')) bottom = clamp(a.bottom + dy, a.top + min.h, max.y + max.h);
  return { x: left, y: top, w: right - left, h: bottom - top };
}

/** 整体移动并钳在屏内 */
export function applyMove(r: Rect, dx: number, dy: number, max: Rect): Rect {
  return {
    ...r,
    x: clamp(r.x + dx, max.x, max.x + max.w - r.w),
    y: clamp(r.y + dy, max.y, max.y + max.h - r.h),
  };
}

/** 与 max 求交（空交时返回 0 尺寸） */
export function clampRect(r: Rect, max: Rect): Rect {
  const x1 = Math.max(r.x, max.x), y1 = Math.max(r.y, max.y);
  const x2 = Math.min(r.x + r.w, max.x + max.w), y2 = Math.min(r.y + r.h, max.y + max.h);
  return { x: x1, y: y1, w: Math.max(0, x2 - x1), h: Math.max(0, y2 - y1) };
}
```

- [ ] **Step 5: 测试全绿**

Run: `pnpm test`
Expected: 9 个用例全 PASS。

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: 圈选几何纯函数（拖拽归一化/手柄缩放/移动/钳制）与 Vitest 用例"
```

---

### Task 4: 主窗口 UI（操作按钮 + 外观设置）

**Files:**
- Modify: `index.html`、`src/main/main.ts`

**Interfaces:**
- Consumes: `get_settings`/`save_settings` 命令（Task 2）、`start_selection`/`clear_mark`（Task 5 提供——本任务先写调用、按钮在 Task 5 完成前点击会报"命令不存在"，属预期）、`mark-state` 事件（Task 5 发出）。
- Produces: 可运行的主窗口；`save_settings` 的入参对象形状 `{ borderColor, borderWidth, borderRadius }`。

- [ ] **Step 1: 完整主窗口页面**

`index.html` 整体替换为：

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <title>MarkBox</title>
    <style>
      body { font-family: system-ui, "Microsoft YaHei", sans-serif; margin: 0; padding: 16px; color: #222; user-select: none; }
      h1 { font-size: 16px; margin: 0 0 12px; }
      .btns button { padding: 8px 0; flex: 1; cursor: pointer; }
      .btns { display: flex; gap: 8px; margin-bottom: 16px; }
      button:disabled { opacity: .45; cursor: not-allowed; }
      fieldset { border: 1px solid #ddd; border-radius: 8px; margin: 0; }
      legend { font-size: 12px; color: #888; padding: 0 6px; }
      .row { display: flex; align-items: center; gap: 8px; margin: 10px 2px; font-size: 13px; }
      .row label { width: 52px; }
      .row output { width: 28px; text-align: right; }
      input[type="color"] { width: 40px; height: 24px; padding: 0; border: none; background: none; }
      input[type="range"] { flex: 1; }
    </style>
  </head>
  <body>
    <h1>MarkBox</h1>
    <div class="btns">
      <button id="start">开始圈选</button>
      <button id="clear" disabled>清除标记</button>
    </div>
    <fieldset>
      <legend>边框外观（即时生效）</legend>
      <div class="row"><label>颜色</label><input type="color" id="color" /></div>
      <div class="row"><label>粗细</label><input type="range" id="width" min="1" max="10" step="1" /><output id="widthv">3</output></div>
      <div class="row"><label>圆角</label><input type="range" id="radius" min="0" max="16" step="1" /><output id="radiusv">0</output></div>
    </fieldset>
    <script type="module" src="/src/main/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: 主窗口逻辑**

`src/main/main.ts` 整体替换为：

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { MarkState, Settings } from '../shared/types';

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function fillForm(s: Settings) {
  $<HTMLInputElement>('color').value = s.borderColor;
  $<HTMLInputElement>('width').value = String(s.borderWidth);
  $<HTMLInputElement>('radius').value = String(s.borderRadius);
  $('widthv').textContent = String(s.borderWidth);
  $('radiusv').textContent = String(s.borderRadius);
}

function readForm(): Settings {
  return {
    borderColor: $<HTMLInputElement>('color').value,
    borderWidth: Number($<HTMLInputElement>('width').value),
    borderRadius: Number($<HTMLInputElement>('radius').value),
  };
}

invoke<Settings>('get_settings').then(fillForm);

for (const id of ['color', 'width', 'radius']) {
  const el = $<HTMLInputElement>(id);
  el.addEventListener('input', async () => {
    const saved = await invoke<Settings>('save_settings', { settings: readForm() });
    fillForm(saved);
  });
}

$('start').addEventListener('click', () => invoke('start_selection'));
$('clear').addEventListener('click', () => invoke('clear_mark'));

void listen<MarkState>('mark-state', (e) => {
  $('clear').disabled = !e.payload.hasMark;
});
```

- [ ] **Step 3: 自动化验证**

Run: `pnpm build && pnpm test`
Expected: 构建成功（`start_selection`/`clear_mark` 此时不存在不影响构建）。

- [ ] **Step 4: 手动验证**

Run: `pnpm tauri dev`
Expected: 表单加载默认值（红 #FF4D4F、3、0）；拖动粗细滑块 output 跟随；重启后设置保留（配置文件位于 `~/Library/Application Support/com.alex73nf.markbox/settings.json`，打开可见 JSON）。

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: 主窗口操作按钮与外观设置表单"
```

---

### Task 5: Rust 窗口编排、托盘与常驻行为

**Files:**
- Create: `src-tauri/src/windows.rs`、`src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `settings::Settings`、`AppState`（Task 2）。
- Produces（overlay/mark 页面在 Task 6/7 使用）:
  - 命令 `start_selection()`：主窗口隐藏，每显示器创建 `overlay-{i}`（物理像素对齐，置顶、无边框、透明、不进任务栏），显示器信息存入 `AppState.monitors`
  - 命令 `overlay_ready(label: String) -> OverlayInit`：返回该 overlay 的 `{ monitor, scaleFactor }`
  - 命令 `confirm_selection(payload: ConfirmPayload)`：校验 rect 仍落在现存显示器上（热插拔保护，不在则仅销毁 overlays），销毁全部 overlays 与旧 mark，在 rect 处创建 `mark` 窗（`focusable(false)` + `set_ignore_cursor_events(true)`），向 main 发 `mark-state {hasMark:true}`
  - 命令 `cancel_selection()`：销毁全部 overlays（不动 mark）
  - 命令 `clear_mark()`：销毁 mark，发 `mark-state {hasMark:false}`
  - 托盘菜单：圈选 / 清除标记 / 显示主窗口 / 退出；主窗口关闭=隐藏到托盘
  - 事件：任一 overlay 窗口被销毁（如显示器拔掉）→ 兜底销毁全部 overlays

- [ ] **Step 1: windows.rs**

`src-tauri/src/windows.rs`：

```rust
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
```

- [ ] **Step 2: commands.rs**

`src-tauri/src/commands.rs`：

```rust
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

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
pub struct ConfirmPayload { pub label: String, pub rect: PhysRect }

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
        windows::spawn_mark(&app, r.x, r.y, r.w, r.h).map_err(|e| e.to_string())?;
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
    windows::emit_mark_state(&app);
}

/// 托盘菜单事件分发用
pub fn handle_tray(app: &AppHandle, id: &str) {
    match id {
        "select" => { let _ = windows::begin_selection(app); }
        "clear" => clear_mark(app.clone()),
        "show" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
```

- [ ] **Step 3: lib.rs 整合（托盘、关闭到托盘、overlay 兜底清理）**

`src-tauri/src/lib.rs` 整体替换为：

```rust
mod commands;
mod settings;
mod windows;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::WindowEvent;

use crate::windows::MonitorInfo;

#[derive(Default)]
pub struct AppState {
    pub settings: Mutex<settings::Settings>,
    pub monitors: Mutex<Vec<(String, MonitorInfo)>>,
}

pub fn settings_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_config_dir().unwrap().join("settings.json")
}

#[tauri::command]
fn get_settings(state: tauri::State<AppState>) -> settings::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: tauri::State<AppState>, settings: settings::Settings) -> Result<settings::Settings, String> {
    let normalized = settings::normalize(&settings);
    settings::save_to(&settings_path(&app), &normalized).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = normalized.clone();
    let _ = app.emit_to("mark", "settings-updated", &normalized);
    Ok(normalized)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            let loaded = settings::load_from(&settings_path(app.handle()));
            app.manage(AppState { settings: Mutex::new(loaded), monitors: Mutex::default() });

            let select = MenuItem::with_id(app, "select", "圈选", true, None::<&str>)?;
            let clear = MenuItem::with_id(app, "clear", "清除标记", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = MenuBuilder::new(app).items(&[&select, &clear, &show]).separator().item(&quit).build()?;

            TrayIconBuilder::with_id("markbox-tray")
                .icon(app.default_window_icon().unwrap().clone())
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

            let _ = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": false }));
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                let _ = window.hide();
            }
            WindowEvent::Destroyed => {
                // 任一 overlay 被销毁（崩溃/拔屏）→ 兜底清掉全部圈选层
                if window.label().starts_with("overlay-") {
                    windows::end_selection(window.app_handle());
                }
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings, save_settings,
            commands::start_selection, commands::overlay_ready,
            commands::confirm_selection, commands::cancel_selection, commands::clear_mark
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 自动化验证**

Run: `cd src-tauri && cargo test && cargo check`，然后 `cd .. && pnpm build`
Expected: 编译零错误、既有测试仍绿。

- [ ] **Step 5: 手动验证**

Run: `pnpm tauri dev`
Expected（overlay/mark 还是占位页，验证编排本身）:
1. 点"开始圈选"→ 主窗口隐藏，每个显示器出现一个全屏置顶半透明可点击的占位页
2. 托盘左键单击 → 主窗口回来；右键菜单四项可见
3. 点主窗口关闭 × → 隐藏到托盘而非退出；托盘"退出"才退出
4. 再跑一个 `pnpm tauri dev` 实例 → 不会出现第二个应用（single-instance 唤起已有实例）
5. 在 overlay 占位页按 F12（或用 `invoke` 调 `cancel_selection`）→ 所有圈选层关闭

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: 多显示器 overlay 编排、mark 窗口创建、托盘与关闭到托盘"
```

---

### Task 6: 圈选覆盖层完整交互

**Files:**
- Modify: `overlay.html`、`src/overlay/overlay.ts`

**Interfaces:**
- Consumes: `overlay_ready`/`confirm_selection`/`cancel_selection`（Task 5）、`geometry.ts` 全部函数（Task 3）、`types.ts`。
- Produces: 完整圈选交互；确认时上报物理像素 `PhysRect`。

- [ ] **Step 1: overlay.html 完整结构**

`overlay.html` 整体替换为：

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" /><title>overlay</title>
    <style>
      html, body { margin: 0; width: 100%; height: 100%; overflow: hidden; background: transparent; cursor: crosshair; }
      #sel {
        position: fixed; left: 0; top: 0; width: 0; height: 0;
        border: 1px solid #00C2FF; box-sizing: border-box;
        box-shadow: 0 0 0 200vmax rgba(0, 0, 0, 0.35); /* 巨型阴影=整屏压暗，选区内透明 */
      }
      #sel .handle {
        position: absolute; width: 10px; height: 10px; background: #fff;
        border: 1px solid #00C2FF; border-radius: 2px; display: none;
      }
      #sel.adjusting { cursor: move; }
      #sel.adjusting .handle { display: block; }
      .handle[data-h="nw"] { left: -6px; top: -6px; cursor: nwse-resize; }
      .handle[data-h="n"]  { left: calc(50% - 5px); top: -6px; cursor: ns-resize; }
      .handle[data-h="ne"] { right: -6px; top: -6px; cursor: nesw-resize; }
      .handle[data-h="e"]  { right: -6px; top: calc(50% - 5px); cursor: ew-resize; }
      .handle[data-h="se"] { right: -6px; bottom: -6px; cursor: nwse-resize; }
      .handle[data-h="s"]  { left: calc(50% - 5px); bottom: -6px; cursor: ns-resize; }
      .handle[data-h="sw"] { left: -6px; bottom: -6px; cursor: nesw-resize; }
      .handle[data-h="w"]  { left: -6px; top: calc(50% - 5px); cursor: ew-resize; }
      #size {
        position: absolute; left: 0; top: -26px; padding: 2px 8px; white-space: nowrap;
        background: rgba(0, 0, 0, 0.7); color: #fff; font: 12px/1.4 system-ui; border-radius: 3px;
      }
      #confirm {
        position: absolute; padding: 4px 12px; border: none; border-radius: 4px; cursor: pointer;
        background: #1677ff; color: #fff; font: 13px/1.4 system-ui; display: none;
      }
      #sel.adjusting #confirm { display: block; }
    </style>
  </head>
  <body>
    <div id="sel">
      <div class="handle" data-h="nw"></div><div class="handle" data-h="n"></div>
      <div class="handle" data-h="ne"></div><div class="handle" data-h="e"></div>
      <div class="handle" data-h="se"></div><div class="handle" data-h="s"></div>
      <div class="handle" data-h="sw"></div><div class="handle" data-h="w"></div>
      <div id="size"></div>
      <button id="confirm">✓ 确认</button>
    </div>
    <script type="module" src="/src/overlay/overlay.ts"></script>
  </body>
</html>
```

- [ ] **Step 2: overlay.ts 完整实现**

`src/overlay/overlay.ts` 整体替换为：

```typescript
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { ConfirmPayload, OverlayInit, PhysRect, Rect } from '../shared/types';
import { applyMove, applyResize, clampRect, normalizeDrag, type Handle } from '../shared/geometry';

const MIN_DRAG = 5;                                   // 松手时小于此值视为误触
const MIN_SIZE = { w: 10, h: 10 };                    // 手柄调整的最小尺寸
const label = getCurrentWebviewWindow().label;

let init: OverlayInit;                                // 本屏物理几何
let bounds: Rect;                                     // 本屏 CSS 像素边界 {0,0,w,h}
let phase: 'idle' | 'draft' | 'adjust' = 'idle';
let rect: Rect = { x: 0, y: 0, w: 0, h: 0 };
let dragStart = { x: 0, y: 0 };
let active: { kind: 'move'; base: Rect } | { kind: 'resize'; handle: Handle; base: Rect } | null = null;

const sel = document.getElementById('sel')!;
const size = document.getElementById('size')!;
const confirmBtn = document.getElementById('confirm')!;

const cancel = () => invoke('cancel_selection');
const confirm = () => {
  const dpr = window.devicePixelRatio;
  const phys: PhysRect = {
    x: init.monitor.x + Math.round(rect.x * dpr),
    y: init.monitor.y + Math.round(rect.y * dpr),
    w: Math.round(rect.w * dpr),
    h: Math.round(rect.h * dpr),
  };
  invoke('confirm_selection', { payload: { label, rect: phys } satisfies ConfirmPayload });
};

function render() {
  sel.style.left = `${rect.x}px`;
  sel.style.top = `${rect.y}px`;
  sel.style.width = `${rect.w}px`;
  sel.style.height = `${rect.h}px`;
  const dpr = window.devicePixelRatio;
  size.textContent = `${Math.round(rect.w * dpr)} × ${Math.round(rect.h * dpr)}`;
  size.style.left = '0px';
  size.style.top = rect.y < 30 ? '4px' : '-26px';
  // 确认按钮：框右下角外侧，贴底翻到框上方，贴右收到框内
  const btnW = 84, btnH = 30;
  let bx = rect.w + 8;
  if (rect.x + rect.w + btnW > bounds.w) bx = rect.w - btnW;
  let by = rect.h + 8;
  if (rect.y + rect.h + btnH > bounds.h) by = -btnH - 8;
  confirmBtn.style.left = `${Math.max(0, bx)}px`;
  confirmBtn.style.top = `${by}px`;
}

function enterAdjust() {
  phase = 'adjust';
  sel.classList.add('adjusting');
  document.body.style.cursor = 'default';
}

window.addEventListener('pointerdown', (e) => {
  if (e.button !== 0) return;
  const target = e.target as HTMLElement;
  if (phase === 'idle') {
    phase = 'draft';
    dragStart = { x: e.clientX, y: e.clientY };
    rect = { x: e.clientX, y: e.clientY, w: 0, h: 0 };
  } else if (phase === 'adjust') {
    const handle = target.dataset['h'] as Handle | undefined;
    if (handle) {
      active = { kind: 'resize', handle, base: { ...rect } };
    } else if (target === sel) {
      active = { kind: 'move', base: { ...rect } };
    } else {
      return; // 点在框外（按钮等）不处理
    }
    dragStart = { x: e.clientX, y: e.clientY };
  }
});

window.addEventListener('pointermove', (e) => {
  if (phase === 'draft') {
    rect = clampRect(normalizeDrag(dragStart.x, dragStart.y, e.clientX, e.clientY), bounds);
    render();
  } else if (phase === 'adjust' && active) {
    const dx = e.clientX - dragStart.x, dy = e.clientY - dragStart.y;
    rect = active.kind === 'move'
      ? applyMove(active.base, dx, dy, bounds)
      : applyResize(active.base, active.handle, dx, dy, MIN_SIZE, bounds);
    render();
  }
});

window.addEventListener('pointerup', () => {
  if (phase === 'draft') {
    if (rect.w < MIN_DRAG || rect.h < MIN_DRAG) {
      cancel(); // 误触：整个操作取消
      return;
    }
    enterAdjust();
  }
  active = null;
});

window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') cancel();
  if (e.key === 'Enter' && phase === 'adjust') confirm();
});

window.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  cancel();
});

confirmBtn.addEventListener('pointerdown', (e) => e.stopPropagation());
confirmBtn.addEventListener('click', confirm);

invoke<OverlayInit>('overlay_ready', { label }).then((payload) => {
  init = payload;
  const dpr = window.devicePixelRatio;
  bounds = { x: 0, y: 0, w: init.monitor.width / dpr, h: init.monitor.height / dpr };
  render();
});
```

- [ ] **Step 3: 自动化验证**

Run: `pnpm test && pnpm build`
Expected: 全绿/成功。

- [ ] **Step 4: 手动验证（核心验收）**

Run: `pnpm tauri dev`
Expected:
1. 主窗口"开始圈选"→ 整屏压暗、光标十字；拖出矩形，选区内透明，左上角显示物理像素尺寸
2. 松手 → 进入调整：8 个白色手柄 + ✓ 确认按钮（框右下外侧；框贴底时翻到上方；框贴右时收进框内）
3. 拖角/拖边可拉宽拉高（不小于 10×10、不越屏）；框内拖动可整体移动
4. Enter 或点 ✓ → 圈选层消失，出现红色边框标记窗（当前 mark.html 还是固定样式）
5. Esc、右键：圈选中与调整中都会取消，不留框
6. 轻点一下（<5px 拖动）松手 → 操作取消
7. 有旧标记时再次圈选：旧标记保持可见，直到新框确认那一刻才被替换；中途 Esc 旧标记保留

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: 圈选覆盖层完整交互（拖拽/8手柄调整/移动/确认/取消/误触保护）"
```

---

### Task 7: 标记框真实样式与设置热更新

**Files:**
- Modify: `src/mark/mark.ts`、`src-tauri/src/windows.rs`（spawn_mark 透传设置不需要——mark 页面自行 invoke）

**Interfaces:**
- Consumes: `get_settings`（Task 2）、`settings-updated` 事件（Task 2/5 已发出）。
- Produces: 最终视觉：标记框按设置渲染边框颜色/粗细/圆角，改动主窗口设置即时生效。

- [ ] **Step 1: mark.ts 实现**

`src/mark/mark.ts` 整体替换为：

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Settings } from '../shared/types';

const box = document.getElementById('box')!;

function apply(s: Settings) {
  box.style.borderWidth = `${s.borderWidth}px`;
  box.style.borderColor = s.borderColor;
  box.style.borderRadius = `${s.borderRadius}px`;
}

invoke<Settings>('get_settings').then(apply);
void listen<Settings>('settings-updated', (e) => apply(e.payload));
```

- [ ] **Step 2: 自动化验证**

Run: `pnpm test && pnpm build`
Expected: 全绿/成功。

- [ ] **Step 3: 手动验证**

Run: `pnpm tauri dev`
Expected:
1. 圈选确认 → 标记框按当前设置渲染（默认红 3px 直角）
2. 不关闭任何窗口，在主窗口改颜色/粗细/圆角 → 标记框即时变化
3. 穿透验证（macOS 开发近似验证，最终以 Windows 为准）：鼠标移到标记框边框和中心区域，目标应用的光标/点击/滚轮均不受影响；标记框不出现在任务栏/Dock，不抢焦点（点它下面的窗口照常激活）
4. 标记框始终置顶：被其他窗口盖住后仍显示在最上层

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat: 标记框按设置渲染并支持热更新"
```

---

### Task 8: GitHub Actions 发布流水线

**Files:**
- Create: `.github/workflows/release.yml`

**Interfaces:**
- Consumes: 仓库现有构建体系（pnpm + cargo + tauri.conf.json，bundle targets 需含 nsis）。
- Produces: 推送 `v*` tag 时产出 NSIS 安装包与免安装 exe 并附到 GitHub Release。

- [ ] **Step 1: 确认 bundle 配置**

`src-tauri/tauri.conf.json` 的 `bundle` 段确保为：

```json
"bundle": {
  "active": true,
  "targets": ["nsis"],
  "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
}
```

- [ ] **Step 2: 写 workflow**

`.github/workflows/release.yml`：

```yaml
name: release
on:
  push:
    tags: ['v*']
permissions:
  contents: write
jobs:
  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 10 }
      - uses: actions/setup-node@v4
        with: { node-version: 22, cache: pnpm }
      - uses: dtolnay/rust-toolchain@stable
      - uses: swatinem/rust-cache@v2
        with: { workspaces: src-tauri }
      - run: pnpm install --frozen-lockfile
      - run: pnpm test
      - run: pnpm tauri build
      - name: 打包免安装版
        shell: pwsh
        run: Compress-Archive -Path src-tauri/target/release/markbox.exe -DestinationPath markbox-portable.zip
      - uses: actions/upload-artifact@v4
        with:
          name: markbox-windows
          path: |
            src-tauri/target/release/bundle/nsis/*.exe
            markbox-portable.zip
      - uses: softprops/action-gh-release@v2
        with:
          files: |
            src-tauri/target/release/bundle/nsis/*.exe
            markbox-portable.zip
```

- [ ] **Step 3: 语法校验（本地可做的部分）**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml'))" && echo OK
```

Expected: 输出 OK。

- [ ] **Step 4: Commit 并推送**

```bash
git add -A
git commit -m "ci: Windows 发布流水线（NSIS + 免安装包，tag 触发）"
git push
```

- [ ] **Step 5: 首个发布验证（Windows 验收总闸）**

在 Windows 机器（或确认 mac 侧全部手动清单通过后）：

```bash
git tag v0.1.0 && git push origin v0.1.0
```

Expected: Actions 构建成功，Release 附带 `MarkBox_0.1.0_x64-setup.exe` 与 `markbox-portable.zip`；安装/解包后按 spec 手动验收清单全项通过（唤起→圈选→调整→确认→穿透→替换→清除→Esc 各阶段取消）。

---

## Self-Review 记录

- **Spec 覆盖**：唤起（主窗口/托盘）T4/T5；压暗+选区透明+尺寸标签 T6；手柄/移动/确认按钮翻转/Enter T3+T6；Esc/右键任意取消 T6；误触<5px 与最小 10×10 T3+T6；标记窗穿透/置顶/不抢焦点/不进任务栏 T5；单框替换（确认瞬间换、取消保留旧框）T5/T6；清除 T5/T4；设置三项+热更新+损坏回退 T2/T4/T7；单实例/关闭到托盘/托盘菜单 T5；多屏每屏 overlay+物理像素 T5；拔屏保护 T5（confirm 校验）+ Destroyed 兜底；overlay 崩溃兜底 T5；CI T8。无缺口。
- **占位符扫描**：无 TBD/TODO；所有代码步骤含完整代码。
- **类型一致性**：`Settings`/`MonitorRect`/`OverlayInit`/`PhysRect`/`ConfirmPayload`/`MarkState` 前后端命名一致（serde camelCase 对应 TS interface）；命令名/事件名/窗口 label 与 Global Constraints 一致；`MonitorInfo`（Rust 内部）与 `OverlayInit.monitor` 字段对应。
