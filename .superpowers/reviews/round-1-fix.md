# Round 1 Fix Report

Implementer: Round 1 fix-wave agent
Base: `d97f91e` (main) → Head: `6836230`
Findings source: `.superpowers/reviews/round-1-findings.md`（Critical 0 / Important 6 / Minor 21 全部处理；Won't-fix 段未动）

## Commits

| SHA | Subject | 范围 |
|---|---|---|
| `782398d` | fix: 圈选全链路健壮性（重入守卫/中途失败自愈/overlay 焦点/promise 兜底/滑块防抖/0×0 标签隐藏） | I1–I6 + M-rust-1/5/6/8/12/13 + M-fe-4/5/6 |
| `f724fea` | refactor: 代码整洁专项（命令归位/可见性收敛/MonitorRect 去重/设置 u16+原子写/魔法数命名/死字段清理/测试补强） | M-rust-2/3/4/7/9/10/11/14 + M-fe-1/2/3/7/8/9/10 + M-test-1/2 |
| `6836230` | chore: 配置与文档修缮（版本单源化/能力描述/发布并发守卫/pnpm 固定/README 与 LICENSE/计划横幅） | M-conf-1..5 + M-meta-1..5 + M-doc-1 + spec 措辞 |

每个提交均独立通过全部门禁（提交前逐批验证）。

## Per-finding status

### Important（6/6 FIXED）

- **I1 FIXED** — `src-tauri/src/windows.rs:19-84`：`AppState` 新增 `selecting: Mutex<bool>`（`lib.rs:15`），加锁 check-then-act 消除双击/主窗口+托盘并发竞态；创建体收进闭包，任一步失败 → `end_selection`（Destroyed 兜底重入幂等）+ `show_main` 后错误上抛；先枚举显示器再隐主窗口（顺带修 M-rust-13）；托盘路径错误经 `eprintln!` 上报（`commands.rs` `handle_tray`）。
- **I2 FIXED** — `src-tauri/src/commands.rs` `confirm_selection`：`spawn_mark` 失败时 `windows::show_main(&app)` 恢复主窗口并保留 Err，不再黑屏。
- **I3 FIXED** — 新增 `src/shared/report.ts`（`report(label, p)` 兜底 catch + `console.error`）；main/overlay/mark 三入口全部 `invoke`/`listen` 均经 `report` 或 try/catch；设置保存失败时以 `get_settings` 持久化真值回滚表单（`src/main/main.ts` `saveNow`）。
- **I4 FIXED** — `src/main/main.ts` `saveDebounced`：滑块 150ms 尾部防抖 + `saveSeq` 序号守卫拒绝乱序回填；`syncOutputs()` 本地即时刷新 output，拖动跟手不丢反馈；颜色选择保持立即保存路径。
- **I5 FIXED** — `src-tauri/src/windows.rs` `begin_selection`：`show()` 后用 `app.cursor_position()`（桌面全局物理坐标）命中光标所在 overlay 并 `set_focus`，未命中兜底最后建出的一个；聚焦失败 `eprintln!` 不阻断圈选。
- **I6 FIXED** — `src/overlay/overlay.ts` `render()`：`size.style.display = rect.w === 0 || rect.h === 0 ? 'none' : ''`，0×0 不再显示。

### Minor（21 项：19 FIXED，1 变体实现，1 显式不改）

`src-tauri/src/commands.rs`
1. **M-rust-1 FIXED** — `ConfirmPayload` 删除 `label` 与 `#[allow(dead_code)]`；`overlay.ts` 载荷改 `{ rect: phys } satisfies ConfirmPayload`；`types.ts` 同步。
2. **M-rust-2 FIXED** — `MonitorRect` 收敛为 `#[derive(Debug, Clone, Copy, Serialize)]`（Copy 为 M-rust-11 合并所需，`overlay_init` 直接 `*info`）。
3. **M-rust-8 FIXED** — 托盘 `"select"` 错误 `eprintln!("[markbox] 发起圈选失败: {e}")`。

`src-tauri/src/lib.rs`
4. **M-rust-4 FIXED** — `expect("app_config_dir must be valid")`（settings_path，随 M-rust-7 迁至 settings.rs）、`expect("default window icon must exist")`（托盘图标）。
5. **M-rust-5 FIXED** — `windows::show_main` 统一"显示+聚焦"，单实例回调/托盘 show/I1/I2 三处共用，错误带日志。
6. **M-rust-6 FIXED** — `clear_mark` 直接 `emit_to("main", "mark-state", { hasMark: false })`（含原因注释）；`emit_mark_state` 仅保留在 confirm 的拔屏分支（该处 mark 未动，查询语义仍正确）。
7. **M-rust-7 FIXED（含小扩展）** — `get_settings`/`save_settings` 迁入 commands.rs，lib.rs 只剩装配；**扩展**：`settings_path` 未留在 lib.rs 也未进 commands.rs，而是迁到其天然归属 `settings.rs`（"one home per concern" 的延伸，见 Deviations）。
8. **M-rust-9 FIXED** — 除 `run()`（main.rs 跨 bin/lib 消费）外全部 `pub(crate)`（AppState 及字段、commands/windows/settings 全部项）。
9. **M-rust-10 FIXED** — setup 内 `settings_path` 只算一次；并实现精确版：`settings::load_or_repair` 返回 `(Settings, bool)`，仅缺失/损坏时重建，正常启动不再无谓重写磁盘。
10. **M-rust-12 FIXED** — 全部吞错路径统一 `eprintln!("[markbox] …")`：hide/show/set_focus/destroy/emit_to/重建设置文件；无新依赖。

`src-tauri/src/windows.rs`
11. **M-rust-11 FIXED** — 删除 `MonitorInfo`，`commands::MonitorRect` 成为唯一屏幕矩形结构（AppState 存储 + OverlayInit 载荷 + overlay_init 直接拷贝）。
12. **M-fe-5 FIXED** — `overlay.ts` pointerdown 对 `document.body` `setPointerCapture`，pointerup/pointercancel 经 `hasPointerCapture` 守卫释放；pointercancel 在 draft 阶段按取消收敛。跨显示器拖拽的一次性 Windows 人工验证仍留给 owner（静态代码无法替代）。
13. **M-rust-13 FIXED** — 枚举 `available_monitors()` 成功后才隐藏主窗口。

`src-tauri/src/settings.rs`
14. **M-rust-3 FIXED** — `border_width`/`border_radius` 改 `u16`（附注释说明动机），normalize 钳制 1–10 / 0–16；新增测试：`0→1`、`300→10`、小写 `#ff4d4f` 合法、`#FFF`/`#GGGGGG` 回默认、`{}` 逐字段默认、`borderWidth:300` 不再连累合法颜色。
15. **M-rust-14 FIXED** — `expect("settings serialize")`；`settings.json.tmp` 写入后 `rename` 原子替换（Windows `fs::rename` = MOVEFILE_REPLACE_EXISTING）。

配置
16. **M-conf-1 FIXED** — capability description 改为覆盖三窗口的准确描述。
17. **M-conf-2 FIXED** — `withGlobalTauri: false`。
18. **M-conf-3 FIXED** — `tauri.conf.json` 删除 `version`（Cargo 为唯一版本源）；可选的 CI tag==版本 断言未加（findings 明示 optional）。
19. **M-conf-4 FIXED** — `crate-type = ["rlib"]`（附注释：staticlib/cdylib 是移动端所需）。
20. **M-conf-5 FIXED** — macos-private-api 行加注释：macOS dev 透明窗口必需、Windows 无副作用、勿清理。
21. **M-conf-6 NO-CHANGE（reviewer 原话 "not a change I'd require"）** — `csp: null` 保留；JSON 无注释位，决策在此记录：离线应用 + inline-style 架构本就要求 `style-src 'unsafe-inline'`，维持现状。

前端
22. **M-fe-1 FIXED** — `enterAdjust` 加类后以 `offsetWidth/offsetHeight` 实测按钮尺寸并立即 `render()`；估算常量仅作 draft 阶段占位。
23. **M-fe-2 FIXED** — `GAP`/`SIZE_FLIP_Y`/`LABEL_H`/`CONFIRM_BTN_FALLBACK` 命名常量。
24. **M-fe-3 FIXED** — `size.style.left` JS 赋值删除（CSS `#size` 已有 `left:0`）。
25. **M-fe-4 FIXED** — pointerdown 顶部 `if (!init) return;`。
26. **M-fe-6 FIXED** — draft 阶段再次 pointerdown 视为重新起拖。
27. **M-fe-7 FIXED（变体）** — `mark.html` `#box` 移除硬编码边框，改 `border-style:solid; visibility:hidden`，`mark.ts` 首次 `apply()` 后才 `visible`。效果同"窗口不可见直到首次 apply"，但不依赖 window show API（避免 core:default 权限不确定性与 Rust/FE 时序耦合），见 Deviations。
28. **M-fe-8 FIXED** — `types.ts` 顶部命名约定注释（width/height=屏幕级物理矩形对齐 Rust `MonitorRect`；w/h=选区矩形）。
29. **M-fe-9 FIXED** — `scaleFactor` 自 TS `OverlayInit`、Rust `OverlayInit`/`MonitorInfo` 及 `m.scale_factor()` 管线全部移除；overlay 继续以 `devicePixelRatio` 为准。
30. **M-fe-10 FIXED** — `index.html` 加"与 Rust Settings::default() 保持一致，仅作加载前占位"注释。

测试
31. **M-test-1 FIXED** — `geometry.test.ts` 新增负原点显示器 `B2={x:-1920,y:-200,w:1000,h:800}` 套件：normalizeDrag 负坐标/起点终点重合、`ne`/`sw` 角钳制、applyMove y/右缘钳制、clampRect 部分相交与完全不相交（w/h→0）。9→16 用例。

CI / meta / docs
32. **M-meta-1 FIXED** — `package.json` `"packageManager": "pnpm@11.21.0"`（本地实测 11.21.0）；`release.yml` 去掉浮动 `version: 11`，由 action 读取该字段。
33. **M-meta-2 FIXED** — `concurrency: group: release-${{ github.ref_name }}`。
34. **M-meta-3 FIXED** — README 删除"（feat/mvp 分支）"。
35. **M-meta-4 FIXED** — README 与 spec L129 改为"免安装版 zip（内含 markbox.exe，解压即用）"；README 增前置要求（Node ≥ 20、pnpm 11、Rust stable）；新增 **MIT LICENSE**（MIT 为默认选择，owner 如有他意可直接换文件）。
36. **M-doc-1 FIXED** — plan 顶部加"**状态：已全部完成 @ v0.1.0。** 历史文档，勾选项不再逐个回填"横幅。
37. **M-meta-5 FIXED** — main.rs 注释译为中文（"防止 Windows release 构建弹出额外的控制台窗口，此行勿删！"），警告语义保留。
38. **M-test-2 FIXED** — tsconfig 加 `"noUncheckedIndexedAccess": true`（`pnpm build`/tsc 全绿）。

Won't-fix 段（markbox_lib 命名、allowBuilds、log 插件、CSP 加固、跨平台发布等）：**一律未动**，与 findings 一致。

## Deviations / 变体（2 项变体 + 3 项注记，0 项跳过）

1. **M-rust-7 扩展**：`settings_path` 未按"留在 lib.rs"处理，也未塞进 commands.rs，而是迁入 `settings.rs`——设置路径属于设置关注点，且满足"lib.rs 只剩装配"的目标。
2. **M-fe-7 变体实现**：以元素级 `visibility:hidden → 首次 apply 可见` 替代"窗口不可见直到 apply/spawn_mark 传设置"。原因：前端驱动窗口 show 需依赖 `core:default` 是否含 `allow-show`（不确定，有回归 v0.1.0 风险）；Rust 传设置则与 get_settings 双源。变体零权限/零时序风险，FOUC 消除效果等同。
3. **M-rust-3 注记**：按 findings 指定改 `u16`，可吸收 `300` 这类超范围值；负数（如 `-1`）仍无法被无符号类型反序列化，走整文件回退默认——这是指定方案的固有边界，默认回退行为本身符合 spec。
4. **M-conf-3 注记**：可选的 CI tag==Cargo 版本断言未实现（findings 标注 optional）。
5. **M-conf-6 注记**：`csp: null` 显式维持现状（reviewer 明言非必改；JSON 无法承载注释，决策记录于此）。

## Gate outputs（最终态，HEAD=6836230）

| 门禁 | 结果 |
|---|---|
| `pnpm test` | **16/16 passed**（1 file；原 9 + 新 7） |
| `pnpm build` | ✓ built in 72ms（tsc strict + `noUncheckedIndexedAccess` 零错误，三入口产出） |
| `cargo test`（src-tauri） | **8/8 passed**（原 4 + 新 4；其余 target 0 tests） |
| `cargo check`（src-tauri） | **0 warnings**（"Finished `dev` profile"） |
| release.yml | YAML 解析 OK；已无 `version: 11` 浮动 pin，含 concurrency 组 |
| package.json / tauri.conf.json / capabilities | JSON 解析 OK；`__TAURI__` 注入关闭 |

## Self-review notes

- 全量 `git diff d97f91e..HEAD` 逐文件复核：无 TODO/FIXME/console.log/dbg! 残留（`console.error` 仅用于 report/saveNow 的错误上报，属 I3 交付物）；无遗留引用 review 的注释。
- 提交切片按"健壮性 / 整洁 / 配置文档"三段，每段提交前独立跑过门禁（commit 1 曾抓出 overlay.ts 漏带 pointermove 监听——tsc 未用符号报错暴露，已修复后入库）。
- spec 固定契约复核：命令名 7 个、事件 `mark-state`/`settings-updated`、窗口 label `main`/`overlay-{i}`/`mark` 全部未变；`OverlayInit` 载荷缩减 scaleFactor 属删除死数据，monitor 字段与 camelCase 序列化不变。
- 行为回归评估：I1/I2 仅在既有失败路径上自愈（成功路径时序不变）；I5 新增 set_focus 不改变穿透/焦点模型（mark 窗仍 `focusable(false)`）；滑块防抖后持久化时机从即时变 150ms 尾部（findings 指定行为）；设置启动重写从"每次"变"仅损坏/缺失时"，归一化逻辑不变，符合 spec"损坏→回退默认并重建"。
- 首个提交曾误将未跟踪的 findings 文档带入，已 amend 移出并归入 docs 提交。
