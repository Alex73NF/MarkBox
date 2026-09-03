# Round 1 Review Findings

Reviewer: Senior code review (architecture / Tauri 2 + Rust / TypeScript / Windows desktop)
Scope: every tracked file at HEAD `d97f91e` (main, clean tree) + spec/plan docs cross-check
Spec: `docs/superpowers/specs/2026-09-03-markbox-design.md` (binding authority)

## Summary

**Verdict: ship-quality v0.1.0 core with no critical defects — the architecture is faithful to the spec (physical-pixel discipline, single-frame semantics, tray lifecycle, click-through mark window are all correct), but 6 important robustness/UX gaps remain, most of them being exactly the previously deferred items, which on re-examination I judge worth fixing now; plus ~21 minor cleanliness items for the 代码洁癖 pass.**

All previously deferred items were re-examined with fresh judgment:

| Deferred item | Fresh verdict |
|---|---|
| slider debounce/out-of-order refill | **Real — Important (I4)** |
| missing `.catch` on invoke/listen | **Real — Important (I3)** |
| `clear_mark` stale `hasMark` emit | **Real but Minor (M-rust-6)** — self-corrects on second click; trivial fix |
| `begin_selection` mid-loop orphan cleanup | **Real — Important (I1)**, and the re-entry guard is also racy (double-click) |
| cross-monitor pointerup loss | **Likely non-issue on Windows** — Win32/Chromium holds implicit mouse capture during a drag, so pointermove/up keep flowing to the capturing overlay even over another monitor/HWND; coordinates are clamped by `clampRect`. Recommend a one-time manual verification on Windows; `setPointerCapture` optional hardening (Minor M-fe-5) |
| "0×0" size label before first drag | **Real — Important (I6)** — visible on *every* invocation, trivial fix |
| confirm button estimated size | Minor (M-fe-2) — 84×30 overestimates the real ~60×26 button; only effect is conservative edge-flipping |
| u8 disk overflow → whole-file default fallback | **Real — Minor (M-rust-3)** but worth fixing: one JSON hand-edit of `borderWidth: 300` silently discards a valid saved color |
| `settings_path` unwrap | Minor (M-rust-4) — practically infallible (identifier is static/valid); prefer `expect(msg)` / propagate |
| settings-vs-window commands split | **Real — Minor (M-rust-7)** — move `get_settings`/`save_settings` into `commands.rs` |
| capability description stale | **Real — Minor (M-conf-1)** |
| README 免安装 exe vs zip | **Real — Minor (M-doc-2)** (also in spec line 129) |
| About-markbox menu remnant | **Not found** — no About menu exists in code; macOS default app menu is runtime-provided, not a repo defect |
| `markbox_lib` naming vs package `markbox` | **Won't-fix** — canonical Tauri template pattern, required by cargo-rs/cargo#8519 on Windows |
| allowBuilds key standardization | **Already correct** — `allowBuilds: {esbuild: true}` is the official pnpm 11 key (pnpm.io/cli/approve-builds; pnpm 11.0 replaced `onlyBuiltDependencies` with `allowBuilds`). Local pnpm 11.21.0 == CI `version: 11`. Only gap: no `packageManager` pin (M-meta-1) |

Counts: **Critical 0 · Important 6 · Minor 21** (+ 7 won't-fix suggestions).

---

## Critical (Must Fix)

None. Core flows (selection lifecycle, physical-pixel coordinate chain `clientX/Y → ×dpr → +monitor origin → Position::Physical`, negative-coordinate handling via i32 `Position::Physical`, click-through via `focusable(false)` + `set_ignore_cursor_events(true)`, close-to-tray, single-instance, settings normalize/fallback/self-heal rewrite at startup) are correctly implemented. No crashes, data loss, or spec violations found.

---

## Important (Should Fix)

### I1. Selection lifecycle robustness: racy re-entry guard + mid-loop failure leaves orphans, hidden main window, and a swallowed error

- **File:** `src-tauri/src/windows.rs:18-55` (`begin_selection`), `src-tauri/src/commands.rs:23-25` (`start_selection`), `src-tauri/src/commands.rs:60` (tray `"select"`).
- **What:**
  1. The guard at `windows.rs:20` is check-then-act with no lock: two near-simultaneous invocations (double-click on 开始圈选 fires two `click` events → two `start_selection` invokes; or main-button + tray 圈选) can both pass and both start building → the second hits a duplicate-label `WebviewWindowBuilder::build()` error.
  2. If `build()?`/`set_position?`/`show()?` fails mid-loop (monitor i>0), overlays already created stay on screen forever (dark fullscreen, always-on-top), `AppState.monitors` lists labels that partially don't exist, the main window is already hidden (`windows.rs:23-25`), and the `Err` is returned to a frontend invoke with no `.catch` (see I3) — the user is stranded on a dark screen with no visible recovery except the tray.
  3. The tray path (`commands.rs:60`) additionally discards the error entirely (`let _ =`).
- **Why:** spec's error-handling section promises the Rust side always cleans up ("非预期的 overlay 崩溃 → Rust 侧兜底清理"); a partially-failed *creation* is the same class of failure and is currently not cleaned. The Destroyed fallback (`lib.rs:77-82`) only fires for windows that were successfully created *and then* destroyed.
- **Fix:** add a `selecting: Mutex<bool>` (or `AtomicBool`) to `AppState`, and wrap the creation body so any error tears down and restores UI:

```rust
pub fn begin_selection(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<crate::AppState>();
    {
        let mut selecting = state.selecting.lock().unwrap();
        if *selecting { return Ok(()); }   // 已在圈选中则忽略
        *selecting = true;
    }
    let result = (|| -> tauri::Result<()> {
        if let Some(main) = app.get_webview_window("main") { let _ = main.hide(); }
        let infos: Vec<(String, MonitorInfo)> = app.available_monitors()?.iter().enumerate()
            .map(|(i, m)| (format!("overlay-{i}"), MonitorInfo { /* 同现实现 */ }))
            .collect();
        *state.monitors.lock().unwrap() = infos.clone();
        for (label, info) in &infos { /* build → set_position → set_size → show，同现实现 */ }
        Ok(())
    })();
    if result.is_err() {
        end_selection(app);      // 销毁已建出的 overlay（Destroyed 兜底重入是幂等的）
        show_main(app);          // 见 M-rust-5：抽取的 show_main 助手
    }
    *state.selecting.lock().unwrap() = false;
    result
}
```

and in `commands.rs::handle_tray`, surface the error at least via `eprintln!`/log (see M-rust-8) instead of `let _ =`.

### I2. `confirm_selection` destroys overlays before spawning the mark; if `spawn_mark` fails the user is stranded

- **File:** `src-tauri/src/commands.rs:33-44` (order at lines 36→38), `src-tauri/src/windows.rs:65-97`.
- **What:** `end_selection` runs first; if `spawn_mark` then returns `Err` (the retry at `windows.rs:81-90` only covers the stale-destroy case), all overlays are gone, no new mark exists, the main window is still hidden (it was hidden at selection start and is only re-shown by the tray), and the `Err` string is returned to an uncaught frontend promise.
- **Why:** worst realistic recovery path in the app: a transient window-creation failure converts a completed selection into a "black hole" state.
- **Fix (minimal):** on `spawn_mark` failure, restore the main window and keep the error:

```rust
if ok {
    if let Err(e) = windows::spawn_mark(&app, r.x, r.y, r.w, r.h) {
        windows::show_main(&app);          // 别把用户丢在黑屏里
        return Err(e.to_string());
    }
}
```

(Alternative, slightly more invasive: spawn the mark before `end_selection` — "先建后拆" — so a failure leaves the overlays up and the user can Esc/retry; the `destroy_mark` retry logic already tolerates the old mark coexisting briefly. Either is acceptable; the minimal patch is enough for v0.1.x.)

### I3. Missing `.catch` on every `invoke`/`listen` promise in all three frontends

- **Files:** `src/main/main.ts:23,28,33,34,36`; `src/overlay/overlay.ts:23,35,120`; `src/mark/mark.ts:13,14`.
- **What:** every `invoke(...)` and both `listen(...)` calls return promises that are never caught. Any command failure (I1/I2, disk error in `save_settings`, WebView teardown racing a confirm) becomes an unhandled promise rejection: silent in release builds, and in the `listen` case it means the 清除标记 button would silently never update.
- **Why:** the settings form is `async` with `await` and no try/catch — a `save_settings` disk failure leaves the form showing a value that was never persisted, with no signal.
- **Fix:** one tiny helper per entry (or in `src/shared/`), used everywhere:

```ts
// src/shared/fp.ts 或各入口顶部
export const report = (label: string, p: Promise<unknown>): void => {
  p.catch((err) => console.error(`[markbox:${label}]`, err));
};

report('start_selection', invoke('start_selection'));
report('mark-state:listen', listen<MarkState>('mark-state', (e) => { ... }));
```

For the settings input handler, add try/catch (or `.catch`) around the `await invoke('save_settings', ...)` and re-`fillForm` from `get_settings` on failure so the form reverts to the persisted truth.

### I4. Slider `input` handler saves to disk on every tick and refills from possibly out-of-order responses

- **File:** `src/main/main.ts:25-31`.
- **What:** dragging the 粗细/圆角 slider fires `input` per pixel → one `save_settings` (sync file write + event emit) per tick. The `await`ed responses can resolve out of order, and `fillForm(saved)` then writes a *stale* value back into the controls — the thumb visibly jumps backward while dragging; on Windows, AV/Defender scan of the settings JSON amplifies both latency and jitter. (This was deferred; re-examined: still real, and the fix is cheap.)
- **Fix:** debounce trailing + sequence guard:

```ts
let timer: number | undefined;
let seq = 0;
el.addEventListener('input', () => {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => {
    const mine = ++seq;
    invoke<Settings>('save_settings', { settings: readForm() })
      .then((saved) => { if (mine === seq) fillForm(saved); })
      .catch(() => {}); // 或统一 report()
  }, 150);
});
```

(`normalize` is identity for in-range values, so skipping the immediate `fillForm` on slider events loses nothing; the color picker can keep the immediate path.)

### I5. Overlay keyboard focus is never established — Esc/Enter (spec'd cancel/confirm paths) may be dead until the first click

- **File:** `src-tauri/src/windows.rs:40-53` (no `set_focus` anywhere for overlays).
- **What:** overlays are built with `.visible(false)` (so the builder-default `focused(true)` has nothing to focus), then `show()` is called. On Windows, `ShowWindow` only activates when the calling thread's process is foreground — true right after a main-window click, **not reliably true after a tray-menu 圈选**. If no overlay holds keyboard focus, pressing Esc/Enter goes to whatever app the user was in; the spec says "圈选或调整的任何阶段按 Esc… → 整个操作取消".
- **Why:** cheap to make deterministic, and it's a spec'd interaction path.
- **Fix:** after `show()`, focus the overlay that contains the cursor (fallback: the last built):

```rust
win.show()?;
if is_cursor_on(info) { win.set_focus()?; }   // 或统一对最后一个 overlay set_focus()
```

Cursor check via `app.cursor_position()` (tauri 2 `PhysicalPosition`) compared against `info.x/y/width/height`. Any overlay receiving Esc cancels all overlays, so focusing exactly one is sufficient. Verify on Windows in the manual pass (start from tray, press Esc immediately).

### I6. "0 × 0" size chip is visible in the top-left corner of the darkened screen on every invocation before the first drag

- **Files:** `src/overlay/overlay.ts:44` (`size.textContent = ...` unconditionally), `overlay.html:26-29` (`#size` has no `display:none` default), called from `overlay.ts:124` (`render()` at init) and `overlay.ts:69` (pointerdown sets a 0-size rect).
- **What:** the darkened fullscreen shows a floating black chip reading `0 × 0` at the top-left until the user drags ≥1px. Cosmetic but 100% visible on every use.
- **Fix (one line in `render()`):**

```ts
size.style.display = rect.w === 0 || rect.h === 0 ? 'none' : '';
```

(or CSS-first: `#size { display: none }` + `#sel.adjusting #size, #sel.drafting #size { display: block }` — but the JS gate also covers the pre-move draft tick.)

---

## Minor (Cleanliness/Polish)

Owner stated these WILL be fixed; grouped per file, all specific.

### `src-tauri/src/commands.rs`

1. **M-rust-1 — `ConfirmPayload.label` is dead weight with `#[allow(dead_code)]`** (`commands.rs:20`). The frontend sends the overlay label; Rust intentionally derives everything from the rect. Dead contract fields + `allow` attributes are smell. **Fix:** remove `label` from `ConfirmPayload` (`commands.rs:20`), from the payload in `overlay.ts:35` (`invoke('confirm_selection', { payload: { rect: phys } satisfies ConfirmPayload })`), and from `ConfirmPayload` in `src/shared/types.ts:10`. (If you prefer keeping it for validation, then actually validate it against `AppState.monitors` and drop the `allow`.)
2. **M-rust-2 — `MonitorRect` derives `Deserialize` but is never deserialized** (`commands.rs:6-8`). Trim to `#[derive(Debug, Clone, Serialize)]`. (Or merge with `MonitorInfo`, see M-rust-11.)
3. **M-rust-8 — tray `"select"` swallows errors** (`commands.rs:60`): `let _ = windows::begin_selection(app);` — at minimum `if let Err(e) = ... { eprintln!("begin_selection failed: {e}"); }`; better, once I1 lands, the error path already self-heals.

### `src-tauri/src/lib.rs`

4. **M-rust-4 — two `unwrap()`s without messages**: `settings_path` at `lib.rs:20` (`app_config_dir().unwrap()`) and `default_window_icon().unwrap()` at `lib.rs:58`. Practically infallible (static valid identifier; icons configured in bundle), but replace with `expect("app_config_dir must be valid")` / `expect("default window icon must exist")` so a future config change panics with a reason instead of a bare "called Option::unwrap() on None".
5. **M-rust-5 — triplicated "show main window" logic**: single-instance callback (`lib.rs:40-43`), tray `"show"` (`commands.rs:62-66`), and needed again by I1/I2. Extract `pub fn show_main(app: &AppHandle)` into `windows.rs` and call from all three.
6. **M-rust-6 — `clear_mark` re-queries instead of emitting the known state** (`commands.rs:51-55` + `windows.rs:109-111`): `destroy_mark` → `emit_mark_state` — `win.destroy()` is processed on the event loop, so `mark_exists` right after can still be `true` and the stale `hasMark:true` is emitted (the 清除标记 button stays enabled; self-corrects on a second click because the Destroyed fallback… actually there is no mark-Destroyed emit, only the second clear click fixes it). **Fix:** emit the known outcome directly:

```rust
#[tauri::command]
pub fn clear_mark(app: AppHandle) {
    windows::destroy_mark(&app);
    let _ = app.emit_to("main", "mark-state", serde_json::json!({ "hasMark": false }));
}
```

   (`spawn_mark`'s hardcoded `hasMark: true` at `windows.rs:95` is correct as-is since it only runs after a successful build.)
7. **M-rust-7 — commands split across `lib.rs` and `commands.rs`**: `get_settings`/`save_settings` live in `lib.rs:23-35` while the other five commands live in `commands.rs`. Move the two settings commands into `commands.rs` (they only need `crate::settings` + `crate::AppState`), leaving `lib.rs` with wiring only — one home per concern.
8. **M-rust-9 — `pub` visibility wider than needed**: only `run()` is consumed by `main.rs`; `AppState`, `settings_path`, and everything in `windows.rs`/`commands.rs` are crate-internal. Downgrade to `pub(crate)`.
9. **M-rust-10 — `settings_path` computed twice in setup** (`lib.rs:46,48`): bind `let path = settings_path(app.handle());` once. Also note the unconditional rewrite at `lib.rs:48` rewrites an unchanged file every launch — harmless, but if you want it precise, only persist when `load_from` fell back (return an indicator from a combined `load_or_repair` helper).
10. **M-rust-12 — no logging anywhere in the Rust layer**: every failure path is `let _ =` or error-strings into the void. One `eprintln!` (or the `log` facade without a plugin) on each swallowed error would make Windows field-debugging possible. Kept deliberately dependency-free is fine — but then be consistent about `eprintln!`.

### `src-tauri/src/windows.rs`

11. **M-rust-11 — `MonitorInfo` ≈ `commands::MonitorRect` duplication + dead derive** (`windows.rs:8-16`): `MonitorInfo` derives `serde::Serialize` (with camelCase) but is never serialized — `overlay_init` hand-converts to `MonitorRect`. Either drop the `Serialize` derive and keep the mapping, or collapse to one struct (`MonitorRect` + `scale_factor`) shared by both files.
12. **M-fe-5 — cross-monitor pointer hardening (deferred item, downgraded)**: Windows keeps implicit mouse capture during a drag, so the pointerup loss is unlikely there. Optional belt-and-braces in `overlay.ts` `pointerdown`: `sel.setPointerCapture(e.pointerId)` (and release on `pointerup`/`pointercancel`); also handles the case where a drag continues over the confirm button. Pair with a one-time Windows manual test: start a drag on monitor A, sweep across B, release on B — selection should complete, not freeze.
13. **M-rust-13 — `begin_selection` hides main before the first build** (`windows.rs:23-25`): if monitor enumeration itself fails (`available_monitors()?` at line 27), main is already hidden. Reorder: enumerate/build first, hide main only once creation is guaranteed to proceed (I1's closure structure fixes this naturally).

### `src-tauri/src/settings.rs`

14. **M-rust-3 — u8 deserialization overflow nukes the whole file** (`settings.rs:8-9`): a hand-edited `"borderWidth": 300` (or `-1`) fails u8 parsing → `load_from` falls back to *all* defaults, silently discarding the valid `borderColor`. `normalize`'s per-field clamp intent argues for clamping instead. **Fix:** widen the storage fields to `u16` and let `normalize` clamp (1–10 / 0–16); serde emits the same JSON numbers, so TS is unaffected:

```rust
pub struct Settings {
    pub border_color: String,
    pub border_width: u16,    // u8 会让 300/-1 整文件反序列化失败
    pub border_radius: u16,
}
```

    Add tests: `border_width: 0 → 1`, `border_width: 300 → 10` (post-fix), lowercase `#ff4d4f` valid, `"#FFF"`/`"#GGGGGG"` → default, `{}` → defaults (container-level `#[serde(default)]` path).
15. **M-rust-14 — `save_to` details** (`settings.rs:42`): `serde_json::to_string_pretty(s).unwrap()` is fine (infallible for this struct) but `expect("settings serialize")` reads better; and `std::fs::write` is non-atomic — a crash mid-write corrupts the file. The startup self-heal rewrite (`lib.rs:47-48`) already recovers, so this is optional: write `settings.json.tmp` then `rename` for a truly robust cycle.

### `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` / capabilities

16. **M-conf-1 — capability description stale** (`src-tauri/capabilities/default.json:4`): says "Capability for the main window" but `windows` covers `main`, `overlay-*`, `mark`. Fix: `"description": "All app windows: main UI, per-monitor selection overlays, and the click-through mark border."`
17. **M-conf-2 — `withGlobalTauri: true` unused** (`tauri.conf.json:13`): the frontend imports `@tauri-apps/api` from npm; `window.__TAURI__` is never referenced (grepped). Set `false` — drops the injected global bundle from all five webviews.
18. **M-conf-3 — version triple duplication**: `0.1.0` appears in `package.json:4`, `Cargo.toml:3`, and `tauri.conf.json:4`. `tauri.conf.json` inherits the Cargo package version when `version` is omitted — delete it there and let Cargo be the source of truth (npm version is inert for the exe). Optional: a CI assertion that tag `vX` == Cargo version.
19. **M-conf-4 — `crate-type = ["staticlib", "cdylib", "rlib"]`** (`Cargo.toml:15`): staticlib/cdylib exist for Tauri mobile, which this desktop-only spec explicitly excludes. `crate-type = ["rlib"]` (keep `name = "markbox_lib"` — see won't-fix) shortens clean builds.
20. **M-conf-5 — `macos-private-api` uncommented** (`Cargo.toml:21`): it's required for transparent windows during macOS `tauri dev` (matching `macOSPrivateApi: true` in the conf) and is a no-op on Windows — add one comment line saying exactly that so nobody "cleans it up" and breaks macOS dev.
21. **M-conf-6 — `"csp": null`** (`tauri.conf.json:19`): template default. For a fully-offline app the risk is minimal, and the inline `<style>`/`style=` usage forces `style-src 'unsafe-inline'` anyway — listed so the decision is explicit; a comment-worthy trade-off, not a change I'd require.

### Frontend

22. **M-fe-1 — confirm button estimated size** (`overlay.ts:48`): `btnW = 84, btnH = 30` vs actual ~60×26 (`padding: 4px 12px`, `font: 13px/1.4`). Overestimate only causes premature edge-flipping; fix by measuring in `enterAdjust` (`confirmBtn.offsetWidth/offsetHeight`) or tighten the constants.
23. **M-fe-2 — magic numbers in `render()`** (`overlay.ts:46-54`): `8` (gap), `30` (top flip threshold), `26` (label height), `84/30` (button) — name them (`GAP`, `LABEL_H`, `FLIP_MARGIN`, …) or hoist to consts alongside `MIN_DRAG`/`MIN_SIZE`.
24. **M-fe-3 — `size.style.left = '0px'` re-assigned every render** (`overlay.ts:45`): it's constant — belongs in the CSS block for `#size`.
25. **M-fe-4 — pre-init race** (`overlay.ts:83-94`): a pointerdown landing before `overlay_ready` resolves leaves `bounds === undefined`; the first `pointermove` throws `TypeError` inside the listener (rect frozen, mis-tap cancel still recovers). Guard: `if (!init) return;` at the top of `pointerdown` (or make `bounds` `Rect = { x: 0, y: 0, w: 0, h: 0 }` until init).
26. **M-fe-6 — overlay draft state trap** (`overlay.ts:63-81`): if the pointerup is ever genuinely lost (the macOS-dev case), a stuck `draft` phase ignores all subsequent `pointerdown`s (neither `idle` nor `adjust` branch matches) — only Esc/right-click escape. Optional: treat a new `pointerdown` during `draft` as a fresh drag start.
27. **M-fe-7 — mark.html FOUC-class nit** (`mark.html:4`): the inline default border (`3px solid #FF4D4F`) flashes before `get_settings` resolves if settings differ. Invisible in practice (window is shown after webview load); if it ever bothers, have `spawn_mark` pass current settings or keep the window invisible until first `apply`.
28. **M-fe-8 — rect-shape naming zoo** (`src/shared/types.ts:6-8` vs `src/shared/geometry.ts:1`): `MonitorRect{width,height}` vs `PhysRect{w,h}` vs `Rect{w,h}`. Each mirrors its Rust counterpart faithfully, but add one comment line in `types.ts` stating that convention explicitly (`width/height` = 屏幕级物理矩形，对齐 Rust `MonitorInfo`；`w/h` = 选区矩形) so future readers don't "unify" it wrongly.
29. **M-fe-9 — `OverlayInit.scaleFactor` is dead data** (`types.ts:7`, `commands.rs:12`, `windows.rs:15`, `windows.rs:33`): overlay.ts uses `window.devicePixelRatio` everywhere (correct — it's what the webview actually renders with). Remove `scaleFactor` from `OverlayInit`, `MonitorInfo`, and `m.scale_factor()` plumbing — or invert the decision and use it instead of `devicePixelRatio`; carrying both is the only wrong answer.
30. **M-fe-10 — index.html duplicates Rust defaults** (`index.html:30-31`): `<output>` values `3`/`0` and the `disabled` clear button hardcode what `Settings::default()` owns. Acceptable (it's the pre-`get_settings` flash state), but add a one-line comment `<!-- 与 Settings::default() 保持一致，仅作加载前占位 -->` so the coupling is visible.

### Tests

31. **M-test-1 — geometry tests only exercise origin-at-0 screens** (`src/shared/geometry.test.ts`): every `max` is `{0,0,…}`, yet production runs on negative-origin monitors (left-of-primary). The functions are translate-correct but untested for it. Add: `clampRect`/`applyMove`/`applyResize` against `B2 = { x: -1920, y: -200, w: 1000, h: 800 }`; `clampRect` disjoint case (w/h → 0); `applyMove` y-clamp; a corner `ne`/`sw` clamp case; `normalizeDrag` identical start/end.

### CI / meta / docs

32. **M-meta-1 — pin pnpm via `packageManager`** (`package.json`): add `"packageManager": "pnpm@11.21.0"`; then `pnpm/action-setup` can drop its floating `version: 11` (release.yml:13) and derive from the field — removes local/CI drift as the pnpm major moves.
33. **M-meta-2 — release workflow lacks a concurrency guard** (`.github/workflows/release.yml`): two tags pushed in quick succession race two release jobs. Add:

```yaml
concurrency:
  group: release-${{ github.ref_name }}
```

34. **M-meta-3 — README stale branch reference** (`README.md:8`): "（feat/mvp 分支）" — the code is on `main`; drop the parenthetical.
35. **M-meta-4 — README/spec "免安装 exe" wording** (`README.md:13`; same wording in spec line 129): the artifact is `markbox-portable.zip` containing `markbox.exe`. Say "免安装版 zip（内含 markbox.exe，解压即用）" in both. While in README: add prerequisites (Node ≥ 20, pnpm 11, Rust stable) and a LICENSE — a public repo with releases and no license is "all rights reserved" by default.
36. **M-doc-1 — plan checkboxes all unchecked** (`docs/superpowers/plans/2026-09-03-markbox-mvp.md`): every `- [ ]` step shipped. Historical doc, but if it's meant to reflect reality, tick them or add a "已全部完成 @ v0.1.0" banner at the top.
37. **M-meta-5 — comment-language consistency**: `src-tauri/src/main.rs:1` keeps the English template shout ("DO NOT REMOVE!!") amid an otherwise Chinese-commented codebase. Content is worth keeping (it prevents exactly the kind of "cleanup" that breaks Windows release builds); optionally translate the surrounding sentence while keeping the warning.
38. **M-test-2 — `tsconfig.json` could go one notch stricter**: `noUncheckedIndexedAccess: true` would flag `target.dataset['h']`-style accesses; optional given the codebase is tiny and the one index access is already cast.

---

## Won't-fix suggestions (explicitly beyond v0.1.x scope)

- **Keep `[lib] name = "markbox_lib"`** — deferred-item re-examined: this is the canonical Tauri template workaround for cargo-rs/cargo#8519 (Windows lib/bin name collision), not naming drift. Keep; optionally note it in Cargo.toml (the template comment already explains it).
- **`allowBuilds` key** — re-examined: already the correct pnpm 11 standard (pnpm 11.0 replaced `onlyBuiltDependencies` with `allowBuilds`; `pnpm approve-builds` writes exactly this shape). No change; the only gap is the `packageManager` pin (M-meta-1).
- **Global structured logging (`tauri-plugin-log`)** — plan forbids plugins beyond single-instance; `eprintln!` (M-rust-12) covers v0.1 needs.
- **CSP hardening** — requires `style-src 'unsafe-inline'` given the inline-style architecture; near-zero value for an offline tool. Revisit only if remote content ever enters the webviews.
- **Cross-platform releases / macOS activation-policy polish, code signing, auto-update** — spec non-goals (Windows-only, no updater).
- **Persisting mark/settings across restarts beyond settings.json** — spec non-goal; mark is session-scoped by design.
- **Actions SHA-pinning / Dependabot / release notes generation** — supply-chain and release ergonomics for a single-maintainer tag flow; fine to add whenever, not a v0.1 defect.

---

## Verification notes

- Read every tracked file listed in the mandate, plus `docs/superpowers/plans/2026-09-03-markbox-mvp.md`, `.vscode/extensions.json`, `.superpowers/sdd/.gitignore`, both `.gitignore`s; cross-checked against spec/plan for drift.
- `git log` (12 commits), `git status` (clean), `git show d97f91e` (final-commit diff: Enter re-entry guard, 先存后建, visible(false)→show anti-flash, settings write-back, README/CI).
- `pnpm test` → **9/9 passed** (vitest 4.1.11). `cargo test` (src-tauri) → **4/4 passed**.
- `pnpm --version` → 11.21.0; `pnpm-lock.yaml` lockfileVersion 9.0; `src-tauri/gen/` untracked (gitignored) — confirmed via `git ls-files`.
- Greps: no `TODO/FIXME/TBD/占位/placeholder` in tracked code; `__TAURI__` unreferenced (withGlobalTauri unused); `markbox_lib` appears only in `main.rs`/`Cargo.toml` (consistent).
- Locked `tauri 2.11.5`; `Emitter`/`Manager` imports all live (final commit's emit removal left no dead import).
- pnpm `allowBuilds` semantics verified against pnpm.io docs (approve-builds / 11.0 release notes / settings/build).
- Not run (already green per owner, no mutation allowed beyond reads): `pnpm build`, `cargo check`.
