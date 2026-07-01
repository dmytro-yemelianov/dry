import type { Segment, Toolpath } from '@sdk/ops';
// @ts-expect-error - shared plain-JS module.
import { splinePoints } from '@webspline';

export interface ViewBox {
  scale: number;
  ox: number;
  oy: number;
}

export type ViewPreset = 'xy' | 'xz' | 'yz' | 'iso';

export interface DrawIrOptions {
  view?: ViewPreset;
  zoom?: number;
  panX?: number;
  panY?: number;
  rotationDeg?: number;
}

function projectPoint(p: (number | null)[], view: ViewPreset): [number, number] {
  const x = p[0] ?? 0;
  const y = p[1] ?? 0;
  const z = p[2] ?? 0;
  switch (view) {
    case 'xz':
      return [x, z];
    case 'yz':
      return [y, z];
    case 'iso':
      return [x - y, (x + y) * 0.45 - z * 2.5];
    case 'xy':
    default:
      return [x, y];
  }
}

function points(seg: Segment, view: ViewPreset): [number, number][] {
  if (seg.kind === 'spline') {
    const sampled = (splinePoints(seg) as number[][] | null) ?? [seg.start as number[], seg.end as number[]];
    return sampled.map((p) => projectPoint(p, view));
  }
  return [projectPoint(seg.start, view), projectPoint(seg.end, view)];
}

export function computeViewBox(segs: Segment[], w: number, h: number, pad = 12, view: ViewPreset = 'xy'): ViewBox {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;

  for (const seg of segs) {
    for (const [x, y] of points(seg, view)) {
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

export function drawIr(ctx: CanvasRenderingContext2D, ir: Toolpath, w: number, h: number, options: DrawIrOptions = {}): void {
  ctx.clearRect(0, 0, w, h);
  const view = options.view ?? 'xy';
  const vb = computeViewBox(ir.segments, w, h, 20, view);
  const zoom = Math.max(0.2, Math.min(10, options.zoom ?? 1));
  const panX = options.panX ?? 0;
  const panY = options.panY ?? 0;
  const theta = ((options.rotationDeg ?? 0) * Math.PI) / 180;
  const cos = Math.cos(theta);
  const sin = Math.sin(theta);
  const cx = w / 2;
  const cy = h / 2;
  const tx = (x: number, y: number): [number, number] => {
    const baseX = vb.ox + x * vb.scale;
    const baseY = h - (vb.oy + y * vb.scale);
    const dx = baseX - cx;
    const dy = baseY - cy;
    return [
      cx + (dx * cos - dy * sin) * zoom + panX,
      cy + (dx * sin + dy * cos) * zoom + panY,
    ];
  };

  for (const seg of ir.segments) {
    const pts = points(seg, view);
    if (pts.length < 2) continue;
    ctx.beginPath();
    const first = tx(pts[0][0], pts[0][1]);
    ctx.moveTo(first[0], first[1]);
    for (let i = 1; i < pts.length; i++) {
      const p = tx(pts[i][0], pts[i][1]);
      ctx.lineTo(p[0], p[1]);
    }
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
