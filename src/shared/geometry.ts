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
