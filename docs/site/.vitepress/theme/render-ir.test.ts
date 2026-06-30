import { test, expect } from 'vitest';
import { computeViewBox, drawIr } from './render-ir';
import type { Segment, Toolpath } from '@sdk/ops';

const seg = (sx: number, sy: number, ex: number, ey: number, travel = false): Segment =>
  ({
    start: [sx, sy, 0],
    end: [ex, ey, 0],
    travel,
    kind: 'line',
    speed: 0,
    length: 0,
    volume: 0,
    filament: 0,
    width: 0.4,
    height: 0.2,
    centre: null,
    clockwise: false,
  }) as Segment;

test('computeViewBox fits all segment endpoints into the canvas with padding', () => {
  const vb = computeViewBox([seg(0, 0, 100, 50)], 200, 200, 10);
  expect(vb.scale).toBeCloseTo(1.8, 5);
});

test('drawIr issues stroke calls for each segment without throwing on a minimal ctx', () => {
  const calls: string[] = [];
  const ctx = new Proxy({}, {
    get: (_t, p) => (typeof p === 'string' && p.endsWith('Style')) ? '' :
      (..._a: unknown[]) => { calls.push(String(p)); return undefined; },
    set: () => true,
  }) as unknown as CanvasRenderingContext2D;
  const ir: Toolpath = { version: 1, segments: [seg(0, 0, 10, 0), seg(10, 0, 10, 10, true)] };
  drawIr(ctx, ir, 100, 100);
  expect(calls.filter((c) => c === 'stroke').length).toBeGreaterThanOrEqual(2);
});
