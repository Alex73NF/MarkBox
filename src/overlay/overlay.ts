import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { ConfirmPayload, OverlayInit, PhysRect } from '../shared/types';
import { applyMove, applyResize, clampRect, normalizeDrag, type Handle, type Rect } from '../shared/geometry';

const MIN_DRAG = 5;                                   // 松手时小于此值视为误触
const MIN_SIZE = { w: 10, h: 10 };                    // 手柄调整的最小尺寸
const label = getCurrentWebviewWindow().label;

let init: OverlayInit;                                // 本屏物理几何
let bounds: Rect;                                     // 本屏 CSS 像素边界 {0,0,w,h}
let phase: 'idle' | 'draft' | 'adjust' | 'closing' = 'idle';
let rect: Rect = { x: 0, y: 0, w: 0, h: 0 };
let dragStart = { x: 0, y: 0 };
let active: { kind: 'move'; base: Rect } | { kind: 'resize'; handle: Handle; base: Rect } | null = null;

const sel = document.getElementById('sel')!;
const size = document.getElementById('size')!;
const confirmBtn = document.getElementById('confirm')!;

const cancel = () => {
  if (phase === 'closing') return; // 确认已触发、teardown 进行中，忽略 Esc/右键
  invoke('cancel_selection');
};
const confirm = () => {
  if (phase !== 'adjust') return; // 防回车连发/按钮双击重复确认
  phase = 'closing';
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
