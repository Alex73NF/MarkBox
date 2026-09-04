import { describe, expect, it } from 'vitest';
import {
  applyMove, applyResize, clampRect, confirmButtonOffset, cssBounds, normalizeDrag,
  sizeLabelOffset, toPhys, type Rect,
} from './geometry';

const B: Rect = { x: 0, y: 0, w: 1000, h: 800 };
const MIN = { w: 10, h: 10 };

describe('normalizeDrag', () => {
  it('向右下拖出常规矩形', () => {
    expect(normalizeDrag(100, 100, 300, 200)).toEqual({ x: 100, y: 100, w: 200, h: 100 });
  });
  it('向左上拖自动归一化', () => {
    expect(normalizeDrag(300, 200, 100, 100)).toEqual({ x: 100, y: 100, w: 200, h: 100 });
  });
  it('起点终点重合得 0 尺寸', () => {
    expect(normalizeDrag(100, 100, 100, 100)).toEqual({ x: 100, y: 100, w: 0, h: 0 });
  });
});

describe('applyResize', () => {
  const r: Rect = { x: 100, y: 100, w: 200, h: 100 };
  it('se 手柄向右下扩', () => {
    expect(applyResize(r, 'se', 50, 20, MIN, B)).toEqual({ x: 100, y: 100, w: 250, h: 120 });
  });
  it('nw 手柄向左上收（右下角锚定）', () => {
    expect(applyResize(r, 'nw', -50, -30, MIN, B)).toEqual({ x: 50, y: 70, w: 250, h: 130 });
  });
  it('w 手柄拖过右边界时钳到最小宽', () => {
    expect(applyResize(r, 'w', 500, 0, MIN, B)).toEqual({ x: 290, y: 100, w: 10, h: 100 });
  });
  it('e 手柄不越屏', () => {
    const out = applyResize(r, 'e', 5000, 0, MIN, B);
    expect(out.x + out.w).toBe(1000);
  });
  it('e 手柄向左拖过最小宽时钳到最小宽', () => {
    expect(applyResize(r, 'e', -500, 0, MIN, B)).toEqual({ x: 100, y: 100, w: 10, h: 100 });
  });
  it('s 手柄不小于最小高', () => {
    const out = applyResize(r, 's', 0, -500, MIN, B);
    expect(out.h).toBe(10);
  });
  it('s 手柄不越屏底', () => {
    expect(applyResize({ x: 100, y: 600, w: 300, h: 150 }, 's', 0, 500, MIN, B))
      .toEqual({ x: 100, y: 600, w: 300, h: 200 });
  });
  // 退化区间（选区 < 最小尺寸且贴屏缘，钳制上下界反序）：屏幕边界优先于最小尺寸，选区不出屏
  it('w 手柄：贴左缘的小于最小宽选区不出屏', () => {
    expect(applyResize({ x: 0, y: 0, w: 6, h: 6 }, 'w', 2, 0, MIN, B)).toEqual({ x: 0, y: 0, w: 6, h: 6 });
  });
  it('n 手柄：贴顶缘的小于最小高选区不出屏', () => {
    expect(applyResize({ x: 0, y: 0, w: 6, h: 6 }, 'n', 0, 2, MIN, B)).toEqual({ x: 0, y: 0, w: 6, h: 6 });
  });
  it('e 手柄：贴右缘的小于最小宽选区不出屏', () => {
    expect(applyResize({ x: 994, y: 794, w: 6, h: 6 }, 'e', -2, 0, MIN, B)).toEqual({ x: 994, y: 794, w: 6, h: 6 });
  });
});

describe('applyMove', () => {
  it('整体平移并钳在屏内', () => {
    const r: Rect = { x: 990, y: 790, w: 50, h: 50 };
    expect(applyMove(r, 30, 30, B)).toEqual({ x: 950, y: 750, w: 50, h: 50 });
  });
});

describe('clampRect', () => {
  it('越界矩形与屏求交', () => {
    expect(clampRect({ x: -50, y: -50, w: 200, h: 200 }, B)).toEqual({ x: 0, y: 0, w: 150, h: 150 });
  });
});

describe('toPhys', () => {
  it('dpr=2 各分量独立取整（半像素进位）', () => {
    expect(toPhys({ x: 101.4, y: 0.5, w: 33.6, h: 20.5 }, { x: 0, y: 0, width: 3840, height: 2160 }, 2))
      .toEqual({ x: 203, y: 1, w: 67, h: 41 });
  });
  it('dpr=1.5 负原点屏：原点平移后再取整', () => {
    expect(toPhys({ x: 10.2, y: 0, w: 10.5, h: 100 }, { x: -1920, y: -200, width: 2880, height: 1620 }, 1.5))
      .toEqual({ x: -1905, y: -200, w: 16, h: 150 });
  });
});

describe('cssBounds', () => {
  it('物理显示器换算为本屏 CSS 边界', () => {
    expect(cssBounds({ x: 0, y: 0, width: 1920, height: 1080 }, 1.25)).toEqual({ x: 0, y: 0, w: 1536, h: 864 });
  });
});

describe('confirmButtonOffset', () => {
  const BTN = { w: 84, h: 30 };
  it('常规：框右下角外侧', () => {
    expect(confirmButtonOffset({ x: 100, y: 100, w: 300, h: 200 }, B, BTN.w, BTN.h)).toEqual({ x: 308, y: 208 });
  });
  it('贴右窄选区：右缘对齐屏幕（负偏移=移到框左外侧）', () => {
    expect(confirmButtonOffset({ x: 950, y: 100, w: 50, h: 200 }, B, BTN.w, BTN.h)).toEqual({ x: -34, y: 208 });
  });
  it('贴底：翻到框上方', () => {
    expect(confirmButtonOffset({ x: 100, y: 600, w: 300, h: 200 }, B, BTN.w, BTN.h)).toEqual({ x: 308, y: -38 });
  });
  it('贴顶全高：上翻越顶再收进框内', () => {
    expect(confirmButtonOffset({ x: 100, y: 0, w: 300, h: 800 }, B, BTN.w, BTN.h)).toEqual({ x: 308, y: 8 });
  });
  it('贴右下角双分支', () => {
    expect(confirmButtonOffset({ x: 950, y: 700, w: 50, h: 100 }, B, BTN.w, BTN.h)).toEqual({ x: -34, y: -38 });
  });
});

describe('sizeLabelOffset', () => {
  it('常规在上缘外侧，贴顶翻进框内', () => {
    expect(sizeLabelOffset({ x: 100, y: 100, w: 300, h: 200 }, B, 60)).toEqual({ top: -26, left: 0 });
    expect(sizeLabelOffset({ x: 100, y: 10, w: 300, h: 200 }, B, 60)).toEqual({ top: 4, left: 0 });
  });
  it('贴右时右缘对齐屏幕右缘（负值左移进屏）', () => {
    expect(sizeLabelOffset({ x: 990, y: 100, w: 5, h: 200 }, B, 60)).toEqual({ top: -26, left: -50 });
  });
});

// 左侧负原点显示器（主屏左边的扩展屏），生产环境真实存在
const B2: Rect = { x: -1920, y: -200, w: 1000, h: 800 };

describe('负原点显示器', () => {
  it('normalizeDrag 在负坐标下正常归一化', () => {
    expect(normalizeDrag(-1800, -100, -1600, 0)).toEqual({ x: -1800, y: -100, w: 200, h: 100 });
  });
  it('applyResize ne 手柄不越负原点屏的上缘', () => {
    const r: Rect = { x: -1800, y: -100, w: 200, h: 100 };
    expect(applyResize(r, 'ne', 0, -500, MIN, B2)).toEqual({ x: -1800, y: -200, w: 200, h: 200 });
  });
  it('applyResize sw 手柄不越负原点屏的左缘/下缘', () => {
    const r: Rect = { x: -1800, y: -100, w: 200, h: 100 };
    expect(applyResize(r, 'sw', -500, 500, MIN, B2)).toEqual({ x: -1920, y: -100, w: 320, h: 600 });
  });
  it('applyMove 在负原点屏内 y/右缘钳制', () => {
    const r: Rect = { x: -1500, y: -180, w: 100, h: 100 };
    expect(applyMove(r, 0, -500, B2)).toEqual({ x: -1500, y: -200, w: 100, h: 100 });
    expect(applyMove(r, 500, 0, B2)).toEqual({ x: -1020, y: -180, w: 100, h: 100 });
  });
  it('clampRect 部分相交按交集截取', () => {
    expect(clampRect({ x: -2000, y: -300, w: 200, h: 200 }, B2)).toEqual({ x: -1920, y: -200, w: 120, h: 100 });
  });
  it('clampRect 完全不相交时 w/h 归 0', () => {
    expect(clampRect({ x: 500, y: 700, w: 100, h: 100 }, B2)).toEqual({ x: 500, y: 700, w: 0, h: 0 });
  });
});
