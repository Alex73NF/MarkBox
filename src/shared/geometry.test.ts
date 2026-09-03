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
