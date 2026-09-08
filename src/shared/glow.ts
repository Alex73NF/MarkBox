/** 标记框内外双侧发光（各一层：边框内侧贴边亮芯 + 外侧短距柔晕）。
 *  外侧阴影最大外延 6px，必须 ≤ mark.html 的 #box inset 与 Rust 侧 MARK_GLOW_MARGIN_CSS（均 8px）——
 *  三处为跨文件契约，改动需同步，否则外侧发光会被窗口边缘裁掉 */
export function glowShadow(color: string): string {
  return `inset 0 0 5px 1px ${color}, 0 0 5px 1px ${color}`;
}
