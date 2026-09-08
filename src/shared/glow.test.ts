import { describe, expect, it } from 'vitest';
import markHtml from '../../mark.html?raw';
import windowsRs from '../../src-tauri/src/windows.rs?raw';
import { glowShadow } from './glow';

/** 发光跨文件契约：Rust 窗口外扩边距（windows.rs MARK_GLOW_MARGIN_CSS）、mark.html 的 #box inset、
 *  glow.ts 的外侧阴影最大外延，三处数值必须满足 外延 ≤ 边距 = inset，否则外侧发光被窗口边缘裁掉。
 *  Rust 侧有哨兵单测钉自己的常量；这里钉前端两处，并以 ?raw 回读 Rust 源码钉第三处——
 *  语言边界没有共享常量的手段，读源码是钉住契约的务实做法（任何一处单方面改动都会红） */
const MARGIN = 8;

describe('标记发光跨文件契约', () => {
  it('glowShadow 参数被钉住（改动需同步三处契约）', () => {
    expect(glowShadow('#FF4FA3')).toBe('inset 0 0 5px 1px #FF4FA3, 0 0 5px 1px #FF4FA3');
  });

  it('外侧阴影最大外延（blur+spread）≤ 窗口外扩边距', () => {
    const m = glowShadow('#000').split(', ')[1]!.match(/^0 0 (\d+)px (\d+)px/)!;
    expect(Number(m[1]) + Number(m[2])).toBeLessThanOrEqual(MARGIN);
  });

  it('mark.html 的 inset 与 Rust 的 MARK_GLOW_MARGIN_CSS 都等于钉住的边距', () => {
    expect(markHtml).toContain(`inset:${MARGIN}px`);
    expect(windowsRs).toContain(`MARK_GLOW_MARGIN_CSS: f64 = ${MARGIN}.0`);
  });
});
