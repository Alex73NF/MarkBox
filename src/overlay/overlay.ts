import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { ConfirmPayload, OverlayInit } from '../shared/types';
import {
  applyMove, applyResize, clampRect, confirmButtonOffset, cssBounds, normalizeDrag,
  sizeLabelOffset, toPhys, type Handle, type Rect,
} from '../shared/geometry';
import { report } from '../shared/report';

const MIN_DRAG = 5;                                   // 松手时小于此值视为误触
const MIN_SIZE = { w: 10, h: 10 };                    // 手柄调整的最小尺寸
const CONFIRM_BTN_FALLBACK = { w: 84, h: 30 };        // 按钮进入调整态前的估算值（进入后以实测为准）
const label = getCurrentWebviewWindow().label;

let init: OverlayInit;                                // 本屏物理几何
let bounds: Rect;                                     // 本屏 CSS 像素边界 {0,0,w,h}
let phase: 'idle' | 'draft' | 'adjust' | 'closing' = 'idle';
let rect: Rect = { x: 0, y: 0, w: 0, h: 0 };
let dragStart = { x: 0, y: 0 };
let active: { kind: 'move'; base: Rect } | { kind: 'resize'; handle: Handle; base: Rect } | null = null;
let btnW = CONFIRM_BTN_FALLBACK.w;
let btnH = CONFIRM_BTN_FALLBACK.h;

function cancel() {
  if (phase === 'closing') return; // 确认/取消已触发、窗口收尾进行中，忽略重复 Esc/右键
  // 与 confirm 对称进入 closing：destroy 是异步窗口期，期间屏蔽 Enter/✓，
  // 防同帧 Esc+Enter 连按在取消后仍确认出框。
  // closing 是一次性锁存：两条命令的业务失败面都伴随覆盖层销毁，仅剩 IPC 整体故障会让窗口滞留，
  // 届时应用本身已在退出边缘
  phase = 'closing';
  report('cancel_selection', invoke('cancel_selection'));
}

function confirm() {
  if (phase !== 'adjust') return; // 防回车连发/按钮双击重复确认
  phase = 'closing';
  report('confirm_selection', invoke('confirm_selection', {
    payload: { rect: toPhys(rect, init.monitor, window.devicePixelRatio) } satisfies ConfirmPayload,
  }));
}

// 先注册两条不依赖 DOM 元素与初始化结果的退出通道：即使下方元素查找或 overlay_ready 失败，
// Esc/右键也能取消（cancel_selection 在 Rust 侧销毁全部覆盖层）
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') cancel();
  if (e.key === 'Enter' && phase === 'adjust') confirm();
});

window.addEventListener('contextmenu', (e) => {
  e.preventDefault();
  cancel();
});

const sel = document.getElementById('sel')!;
const size = document.getElementById('size')!;
const confirmBtn = document.getElementById('confirm')!;

function render() {
  sel.style.left = `${rect.x}px`;
  sel.style.top = `${rect.y}px`;
  sel.style.width = `${rect.w}px`;
  sel.style.height = `${rect.h}px`;
  const dpr = window.devicePixelRatio;
  size.style.display = rect.w === 0 || rect.h === 0 ? 'none' : ''; // 0×0（尚未拖出）不显示尺寸标签
  size.textContent = `${Math.round(rect.w * dpr)} × ${Math.round(rect.h * dpr)}`;
  const labelPos = sizeLabelOffset(rect, bounds, size.offsetWidth);
  size.style.top = `${labelPos.top}px`;
  size.style.left = `${labelPos.left}px`;
  const btnPos = confirmButtonOffset(rect, bounds, btnW, btnH);
  confirmBtn.style.left = `${btnPos.x}px`;
  confirmBtn.style.top = `${btnPos.y}px`;
}

function enterAdjust() {
  phase = 'adjust';
  sel.classList.add('adjusting'); // 先亮出按钮才能量到真实尺寸
  btnW = confirmBtn.offsetWidth;
  btnH = confirmBtn.offsetHeight;
  document.body.style.cursor = 'default';
  render();
}

window.addEventListener('pointerdown', (e) => {
  if (e.button !== 0) return;
  if (!init) return; // overlay_ready 未返回前 bounds 未就绪，不响应
  // 触屏多指已知取舍：不按 pointerId 归属手势。draft 中第二指按下视为重新起拖
  // （兼作指针事件丢失的自恢复出口，占用 pointerId 会把它堵死）；adjust 中第二指
  // 会顶替 active，行为有界。鼠标单指针为主用场景，不为触屏引入额外状态机。
  const target = e.target as HTMLElement;
  if (phase === 'idle' || phase === 'draft') {
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
  // 跨屏拖动兜底：显式捕获指针，pointerup 不因指针移出窗口而丢失
  document.body.setPointerCapture(e.pointerId);
});

const releaseCapture = (e: PointerEvent) => {
  if (document.body.hasPointerCapture(e.pointerId)) {
    document.body.releasePointerCapture(e.pointerId);
  }
};

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

window.addEventListener('pointerup', (e) => {
  releaseCapture(e);
  if (phase === 'draft') {
    if (rect.w < MIN_DRAG || rect.h < MIN_DRAG) {
      cancel(); // 误触：整个操作取消
      return;
    }
    enterAdjust();
  }
  active = null;
});

window.addEventListener('pointercancel', (e) => {
  // 指针被系统接管（等效松手丢失）：退出拖拽态，draft 直接按取消收敛
  releaseCapture(e);
  active = null;
  if (phase === 'draft') cancel();
});

confirmBtn.addEventListener('pointerdown', (e) => e.stopPropagation());
confirmBtn.addEventListener('click', confirm);

invoke<OverlayInit>('overlay_ready', { label })
  .then((payload) => {
    init = payload;
    bounds = cssBounds(init.monitor, window.devicePixelRatio);
    render();
  })
  .catch((err) => {
    console.error('[markbox:overlay_ready]', err);
    // 拿不到本屏几何即不可用：自愈式整单取消（否则只剩一块吞点击的全屏暗层）；
    // 若 IPC 整体故障连取消也发不出，Esc/右键通道随应用退出而终
    report('cancel_selection', invoke('cancel_selection'));
  });
