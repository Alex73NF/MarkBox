import { describe, expect, it } from 'vitest';
import { applyMove, applyResize, clampRect, normalizeDrag, type Rect } from './geometry';

const B: Rect = { x: 0, y: 0, w: 1000, h: 800 };
const MIN = { w: 10, h: 10 };

describe('normalizeDrag', () => {
  it('向右下拖出常规矩形', () => {
    expect(normalizeDrag(100, 100, 300, 200)).toEqual({ x: 100, y: 100, w: 200, h: 100 });
  });
  it('向左上拖自动归一化', () => {
    expect(normalizeDrag(300, 200, 100, 100)).toEqual({ x: 100, y: 100, w: 200, h: 100 });
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
  it('s 手柄不小于最小高', () => {
    const out = applyResize(r, 's', 0, -500, MIN, B);
    expect(out.h).toBe(10);
  });
  // 退化区间（选区 < 最小尺寸且贴屏缘，钳制上下界反序）：屏幕边界优先于最小尺寸，选区不出屏
  it('w 手柄：贴左缘的小于最小宽选区不出屏', () => {
    expect(applyResize({ x: 0, y: 0, w: 6, h: 6 }, 'w', 2, 0, MIN, B)).toEqual({ x: 0, y: 0, w: 6, h: 6 });
  });
  it('n 手柄：贴顶缘的小于最小高选区不出屏', () => {
    expect(applyResize({ x: 0, y: 0, w: 6, h: 6 }, 'n', 0, 2, MIN, B)).toEqual({ x: 0, y: 0, w: 6, h: 6 });
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

// 左侧负原点显示器（主屏左边的扩展屏），生产环境真实存在
const B2: Rect = { x: -1920, y: -200, w: 1000, h: 800 };

describe('负原点显示器', () => {
  it('normalizeDrag 在负坐标下正常归一化', () => {
    expect(normalizeDrag(-1800, -100, -1600, 0)).toEqual({ x: -1800, y: -100, w: 200, h: 100 });
  });
  it('normalizeDrag 起点终点重合得 0 尺寸', () => {
    expect(normalizeDrag(100, 100, 100, 100)).toEqual({ x: 100, y: 100, w: 0, h: 0 });
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
