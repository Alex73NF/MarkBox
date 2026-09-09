import { describe, expect, it } from 'vitest';
import { fitViewport } from './fit';

// 主窗内容实测需要 322×429 CSS px（Edge CDP 仿真量得，含字段集边框与最后一行底距）
const CONTENT = { w: 322, h: 429 };

/** 仿真"窗口物理尺寸 → CSS 视口"：WebView 恒按 floor(物理/DPR) 解释。
 *  ceil 落窗的性质：floor(ceil(t·dpr)/dpr) ≥ t（t 为整数），故落窗后必装得下 */
const viewportOf = (physW: number, physH: number, dpr: number) => ({
  innerW: Math.floor(physW / dpr),
  innerH: Math.floor(physH / dpr),
});
const applyFit = (t: { w: number; h: number }, dpr: number) => ({
  physW: Math.ceil(t.w * dpr),
  physH: Math.ceil(t.h * dpr),
});

describe('fitViewport', () => {
  it('装得下时返回 null（macOS 基准 420×460 视口不触发）', () => {
    expect(fitViewport(420, 460, CONTENT.w, CONTENT.h)).toBeNull();
    expect(fitViewport(322, 429, CONTENT.w, CONTENT.h)).toBeNull();
  });

  it('内容超出时返回目标，且各维不小于当前视口（天然只放大）', () => {
    const t = fitViewport(336, 368, CONTENT.w, CONTENT.h)!;
    expect(t).toEqual({ w: 336, h: 429 }); // 宽度本就装得下则原样保留
    const t2 = fitViewport(300, 368, CONTENT.w, CONTENT.h)!;
    expect(t2).toEqual({ w: CONTENT.w, h: 429 });
  });

  it('分数 scrollWidth 按 ceil 计入：0.4px 也算溢出，保守放大整 1px', () => {
    expect(fitViewport(420, 460, 420.4, 429)).toEqual({ w: 421, h: 460 });
    expect(fitViewport(421, 460, 420.6, 429)).toBeNull(); // 视口已 ≥ ceil(内容) 则不触发
  });

  it('从任意失配视口出发一步收敛，跨屏 DPR 变化重入不再振荡', () => {
    for (const dpr of [1, 1.25, 1.5, 1.75, 2]) {
      // 初始物理尺寸任意（覆盖"逻辑尺寸被按物理解释"等一切失配形态）
      for (const physW of [300, 336, 420, 483, 560]) {
        for (const physH of [350, 368, 460, 536, 640]) {
          let phys = { physW, physH };
          let applied = 0;
          for (let i = 0; i < 10; i++) {
            const v = viewportOf(phys.physW, phys.physH, dpr);
            const t = fitViewport(v.innerW, v.innerH, CONTENT.w, CONTENT.h);
            if (!t) break;
            phys = applyFit(t, dpr);
            applied++;
          }
          const final = viewportOf(phys.physW, phys.physH, dpr);
          expect(fitViewport(final.innerW, final.innerH, CONTENT.w, CONTENT.h)).toBeNull();
          expect(applied).toBeLessThanOrEqual(1); // 收敛判据：至多一次落窗
        }
      }
    }
  });

  it('评审构造的 k 因子振荡场景（跨 DPR 后视口缩小再放大）不再复现', () => {
    // 该场景曾让 k=420/innerWidth 方案陷入 483↔560 物理 2-循环：
    // DPR1.5 适配后视口 322 → 切 DPR2 视口变 241 → 重适配。物理直取下必须一步到位
    const physAfterDpr15 = { physW: 483, physH: 644 }; // 322×429 @1.5 的物理落窗
    const v2 = viewportOf(physAfterDpr15.physW, physAfterDpr15.physH, 2);
    expect(v2.innerW).toBe(241); // 视口缩水、内容装不下
    const t = fitViewport(v2.innerW, v2.innerH, CONTENT.w, CONTENT.h)!;
    const phys = applyFit(t, 2);
    const v = viewportOf(phys.physW, phys.physH, 2);
    expect(fitViewport(v.innerW, v.innerH, CONTENT.w, CONTENT.h)).toBeNull(); // 一步收敛
  });
});
