import type { Segment, Toolpath } from '@sdk/ops';
import { segmentPoints } from './segment-points';

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
  yawDeg?: number;
  pitchDeg?: number;
  rollDeg?: number;
  maxSegments?: number;
  activeSegment?: number;
}

interface ProjectOptions {
  view: ViewPreset;
  yawDeg: number;
  pitchDeg: number;
  rollDeg: number;
}

const PRESETS: Record<ViewPreset, { yawDeg: number; pitchDeg: number; rollDeg: number }> = {
  xy: { yawDeg: 0, pitchDeg: 0, rollDeg: 0 },
  xz: { yawDeg: 0, pitchDeg: -90, rollDeg: 0 },
  yz: { yawDeg: 90, pitchDeg: -90, rollDeg: 0 },
  iso: { yawDeg: -45, pitchDeg: 35, rollDeg: 0 },
};

export function presetAngles(view: ViewPreset): { yawDeg: number; pitchDeg: number; rollDeg: number } {
  return { ...PRESETS[view] };
}

function rad(deg: number): number {
  return (deg * Math.PI) / 180;
}

function projectPoint(p: (number | null)[], options: ProjectOptions): [number, number] {
  const x = p[0] ?? 0;
  const y = p[1] ?? 0;
  const z = p[2] ?? 0;

  const yaw = rad(options.yawDeg);
  const pitch = rad(options.pitchDeg);
  const roll = rad(options.rollDeg);

  const cosy = Math.cos(yaw);
  const siny = Math.sin(yaw);
  const x1 = x * cosy - y * siny;
  const y1 = x * siny + y * cosy;

  const cosp = Math.cos(pitch);
  const sinp = Math.sin(pitch);
  const y2 = y1 * cosp - z * sinp;

  const cosr = Math.cos(roll);
  const sinr = Math.sin(roll);
  return [x1 * cosr - y2 * sinr, x1 * sinr + y2 * cosr];
}

function projectOptions(view: ViewPreset, options: DrawIrOptions = {}): ProjectOptions {
  const preset = presetAngles(view);
  return {
    view,
    yawDeg: options.yawDeg ?? preset.yawDeg,
    pitchDeg: options.pitchDeg ?? preset.pitchDeg,
    rollDeg: options.rollDeg ?? preset.rollDeg,
  };
}

function points(seg: Segment, options: ProjectOptions): [number, number][] {
  return segmentPoints(seg).map((point) => projectPoint(point, options));
}

export function computeViewBox(
  segs: Segment[],
  w: number,
  h: number,
  pad = 12,
  view: ViewPreset = 'xy',
  options: DrawIrOptions = {}
): ViewBox {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const projection = projectOptions(view, options);

  for (const seg of segs) {
    for (const [x, y] of points(seg, projection)) {
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
  const vb = computeViewBox(ir.segments, w, h, 20, view, options);
  const projection = projectOptions(view, options);
  const zoom = Math.max(0.2, Math.min(10, options.zoom ?? 1));
  const panX = options.panX ?? 0;
  const panY = options.panY ?? 0;
  const cx = w / 2;
  const cy = h / 2;
  const tx = (x: number, y: number): [number, number] => {
    const baseX = vb.ox + x * vb.scale;
    const baseY = h - (vb.oy + y * vb.scale);
    const dx = baseX - cx;
    const dy = baseY - cy;
    return [cx + dx * zoom + panX, cy + dy * zoom + panY];
  };
  const limit = Math.min(ir.segments.length, Math.max(0, options.maxSegments ?? ir.segments.length));
  const active = options.activeSegment;

  for (let index = 0; index < limit; index++) {
    const seg = ir.segments[index];
    const pts = points(seg, projection);
    if (pts.length < 2) continue;
    ctx.beginPath();
    const first = tx(pts[0][0], pts[0][1]);
    ctx.moveTo(first[0], first[1]);
    for (let i = 1; i < pts.length; i++) {
      const p = tx(pts[i][0], pts[i][1]);
      ctx.lineTo(p[0], p[1]);
    }
    if (index === active) {
      ctx.strokeStyle = '#ffd166';
      ctx.setLineDash([]);
      ctx.lineWidth = 3;
    } else if (seg.travel) {
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
