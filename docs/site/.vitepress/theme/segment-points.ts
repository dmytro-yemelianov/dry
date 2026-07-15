import type { Segment } from '@sdk/ops';
// @ts-expect-error - shared plain-JS module.
import { splinePoints } from '@webspline';

export type Point3 = [number, number, number];

/** Sample a segment into display points shared by the 2D and Three.js renderers. */
export function segmentPoints(seg: Segment): Point3[] {
  if (seg.kind === 'spline') {
    const sampled = (splinePoints(seg) as number[][] | null) ?? [seg.start as number[], seg.end as number[]];
    return sampled.map(point3);
  }
  if (seg.kind === 'arc' && seg.centre) return arcPoints(seg);
  return [point3(seg.start), point3(seg.end)];
}

function point3(point: (number | null)[]): Point3 {
  return [point[0] ?? 0, point[1] ?? 0, point[2] ?? 0];
}

function arcPoints(seg: Segment): Point3[] {
  const start = point3(seg.start);
  const end = point3(seg.end);
  const cx = seg.centre?.[0] ?? 0;
  const cy = seg.centre?.[1] ?? 0;
  const radius = Math.hypot(start[0] - cx, start[1] - cy);
  if (!Number.isFinite(radius) || radius <= 0) return [start, end];

  const a0 = Math.atan2(start[1] - cy, start[0] - cx);
  let a1 = Math.atan2(end[1] - cy, end[0] - cx);
  if (seg.clockwise && a1 > a0) a1 -= Math.PI * 2;
  if (!seg.clockwise && a1 < a0) a1 += Math.PI * 2;

  return Array.from({ length: 33 }, (_, index) => {
    const t = index / 32;
    const angle = a0 + (a1 - a0) * t;
    return [
      cx + Math.cos(angle) * radius,
      cy + Math.sin(angle) * radius,
      start[2] + (end[2] - start[2]) * t,
    ];
  });
}
