/** 主窗视口自愈的决策核心（供 main.ts 的 fitWindowToContent 与 vitest 共用）。
 *  内容装不下视口时返回目标 CSS 视口（各维不小于当前视口，天然只放大不缩小），
 *  装得下返回 null。调用方以 目标×devicePixelRatio 取物理尺寸落窗：ceil 保证
 *  floor(物理/DPR) ≥ 目标（目标为整数 CSS px），仿真与现实都一步收敛、不振荡 */
export function fitViewport(
  innerW: number, innerH: number,
  scrollW: number, scrollH: number,
): { w: number; h: number } | null {
  const w = Math.max(innerW, Math.ceil(scrollW));
  const h = Math.max(innerH, Math.ceil(scrollH));
  if (w === innerW && h === innerH) return null;
  return { w, h };
}
