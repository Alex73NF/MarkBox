import type { MonitorRect, PhysRect } from './types';

export interface Rect { x: number; y: number; w: number; h: number }
export type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';

const GAP = 8;            // 确认按钮与选区的间距
const SIZE_FLIP_Y = 30;   // 尺寸标签贴顶翻转阈值
const SIZE_FLIP_TOP = 4;  // 贴顶时标签距选区顶缘的偏移
const LABEL_H = 26;       // 尺寸标签高度

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** 起点(sx,sy)拖到(cx,cy)的归一化矩形（正 w/h） */
export function normalizeDrag(sx: number, sy: number, cx: number, cy: number): Rect {
  return { x: Math.min(sx, cx), y: Math.min(sy, cy), w: Math.abs(cx - sx), h: Math.abs(cy - sy) };
}

/** 手柄拖动：对边锚定，强制 min 尺寸并钳制在 max（屏幕）内。
 *  屏幕边界优先：选区暂小于 min（误触微拖后即松手进入调整态）且贴屏缘时，w/n 手柄的
 *  钳制区间会退化为 lo > hi，此时屏幕边界先于最小尺寸生效，防止把选区顶出屏幕；
 *  非退化输入下与「先钳 min 后钳 max」逐位等价 */
export function applyResize(r: Rect, handle: Handle, dx: number, dy: number, min: { w: number; h: number }, max: Rect): Rect {
  const a = { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h };
  let { left, top, right, bottom } = a;
  if (handle.includes('w')) left = Math.max(max.x, Math.min(a.left + dx, a.right - min.w));
  if (handle.includes('e')) right = Math.min(max.x + max.w, Math.max(a.right + dx, a.left + min.w));
  if (handle.includes('n')) top = Math.max(max.y, Math.min(a.top + dy, a.bottom - min.h));
  if (handle.includes('s')) bottom = Math.min(max.y + max.h, Math.max(a.bottom + dy, a.top + min.h));
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

/** CSS 像素选区 → 全局物理像素矩形（确认链路）：原点平移 + 各分量独立取整，
 *  单次换算、偏差 ≤1 物理px 无累积 */
export function toPhys(r: Rect, monitor: MonitorRect, dpr: number): PhysRect {
  return {
    x: monitor.x + Math.round(r.x * dpr),
    y: monitor.y + Math.round(r.y * dpr),
    w: Math.round(r.w * dpr),
    h: Math.round(r.h * dpr),
  };
}

/** 物理像素显示器 → 本屏 CSS 像素边界（overlay 自身的坐标系） */
export function cssBounds(monitor: MonitorRect, dpr: number): Rect {
  return { x: 0, y: 0, w: monitor.width / dpr, h: monitor.height / dpr };
}

/** 尺寸标签相对选区的偏移：默认上缘外侧，贴顶翻进框内；
 *  贴右时右缘对齐屏幕右缘（负值=左移进屏内） */
export function sizeLabelOffset(rect: Rect, bounds: Rect, labelW: number): { top: number; left: number } {
  return {
    top: rect.y < SIZE_FLIP_Y ? SIZE_FLIP_TOP : -LABEL_H,
    left: Math.min(0, bounds.w - rect.x - labelW),
  };
}

/** 确认按钮相对选区的偏移：框右下角外侧；贴底翻到框上方（贴顶翻不动时收进框内）；
 *  贴右收进框内右下，窄于按钮时右缘对齐屏幕右缘（负值=移到框左外侧），保证按钮完整可见 */
export function confirmButtonOffset(rect: Rect, bounds: Rect, btnW: number, btnH: number): { x: number; y: number } {
  let x = rect.w + GAP;
  if (rect.x + rect.w + btnW > bounds.w) x = Math.min(rect.w, bounds.w - rect.x) - btnW;
  let y = rect.h + GAP;
  if (rect.y + rect.h + btnH > bounds.h) y = -btnH - GAP;
  if (rect.y + y < 0) y = GAP;
  return { x, y };
}
