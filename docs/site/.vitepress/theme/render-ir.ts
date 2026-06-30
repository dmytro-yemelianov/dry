import type { Segment, Toolpath } from '@sdk/ops';
// @ts-expect-error - shared plain-JS module.
import { splinePoints } from '@webspline';

export interface ViewBox {
  scale: number;
  ox: number;
  oy: number;
}

function points(seg: Segment): [number, number][] {
  if (seg.kind === 'spline') {
    const sampled = (splinePoints(seg) as number[][] | null) ?? [seg.start as number[], seg.end as number[]];
    return sampled.map((p) => [p[0] ?? 0, p[1] ?? 0]);
  }
  return [
    [seg.start[0] ?? 0, seg.start[1] ?? 0],
    [seg.end[0] ?? 0, seg.end[1] ?? 0],
  ];
}

export function computeViewBox(segs: Segment[], w: number, h: number, pad = 12): ViewBox {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const seg of segs) {
    for (const [x, y] of points(seg)) {
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    }
  }

  if (!Number.isFinite(minX)) {
    minX = 0;
    minY = 0;
    maxX = 1;
    maxY = 1;
  }

  const spanX = Math.max(1e-6, maxX - minX);
  const spanY = Math.max(1e-6, maxY - minY);
  const innerW = Math.max(1, w - 2 * pad);
  const innerH = Math.max(1, h - 2 * pad);
  const scale = Math.min(innerW / spanX, innerH / spanY);
  const ox = pad - minX * scale + (innerW - spanX * scale) / 2;
  const oy = pad - minY * scale + (innerH - spanY * scale) / 2;
  return { scale, ox, oy };
}

export function drawIr(ctx: CanvasRenderingContext2D, ir: Toolpath, w: number, h: number): void {
  ctx.clearRect(0, 0, w, h);
  const vb = computeViewBox(ir.segments, w, h);
  const tx = (x: number) => vb.ox + x * vb.scale;
  const ty = (y: number) => h - (vb.oy + y * vb.scale);

  for (const seg of ir.segments) {
    const pts = points(seg);
    if (pts.length < 2) continue;
    ctx.beginPath();
    ctx.moveTo(tx(pts[0][0]), ty(pts[0][1]));
    for (let i = 1; i < pts.length; i++) ctx.lineTo(tx(pts[i][0]), ty(pts[i][1]));
    if (seg.travel) {
      ctx.strokeStyle = 'rgba(120,140,170,0.45)';
      ctx.setLineDash([4, 4]);
      ctx.lineWidth = 1;
    } else {
      ctx.strokeStyle = '#3aa0ff';
      ctx.setLineDash([]);
      ctx.lineWidth = 2;
    }
    ctx.stroke();
  }
  ctx.setLineDash([]);
}
