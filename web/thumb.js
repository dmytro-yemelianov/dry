// Shared thumbnail renderer for the Dry browser demo.
//
// thumbnail(ops, wasm, params, size) resolves an L1 ops array to IR via the wasm engine and draws
// a tiny top-down (XY) 2D sketch of the toolpath to an offscreen canvas — extrude moves in blue,
// travel moves in red — returning a PNG data URL. No images are committed; thumbnails are generated
// at runtime from the same engine that drives the 3D viewport.
//
// `wasm` is the resolver object ({ resolve_ir, ... }); `params` is RESOLVE_PARAMS.

const EXTRUDE = '#58a6ff';
const TRAVEL = '#f85149';
const BG = '#161b22';
const SPLINE_SAMPLES = 16;

function catmullRom(p0, p1, p2, p3, t) {
  const t2 = t * t, t3 = t2 * t, out = [0, 0, 0];
  for (let a = 0; a < 3; a++) {
    out[a] = 0.5 * ((2 * p1[a]) + (-p0[a] + p2[a]) * t +
      (2 * p0[a] - 5 * p1[a] + 4 * p2[a] - p3[a]) * t2 +
      (-p0[a] + 3 * p1[a] - 3 * p2[a] + p3[a]) * t3);
  }
  return out;
}

function splinePoints(s) {
  if (!s.control_points || !s.control_points.length) return null;
  const start = [s.start[0] ?? 0, s.start[1] ?? 0, s.start[2] ?? 0];
  const through = [start, ...s.control_points.map((p) => [p[0], p[1], p[2]])];
  const pts = [start];
  for (let i = 0; i < through.length - 1; i++) {
    const p0 = through[Math.max(0, i - 1)];
    const p1 = through[i];
    const p2 = through[i + 1];
    const p3 = through[Math.min(through.length - 1, i + 2)];
    for (let step = 1; step <= SPLINE_SAMPLES; step++) {
      pts.push(step === SPLINE_SAMPLES ? p2 : catmullRom(p0, p1, p2, p3, step / SPLINE_SAMPLES));
    }
  }
  return pts;
}

// Flatten the IR segments into a list of { from:[x,y], to:[x,y], travel } in the XY plane.
// Arcs are tessellated so curves read as curves in the sketch.
function segmentsToXY(ir) {
  const out = [];
  const segs = (ir && ir.segments) || [];
  let cur = null; // running position (for segments whose start is inherited / null)
  for (const s of segs) {
    const start = s.start && s.start.every((c) => c != null) ? [s.start[0], s.start[1]] : cur;
    const end = s.end ? [s.end[0], s.end[1]] : null;
    if (!end || end.some((c) => c == null)) { if (end && end.every((c) => c != null)) cur = end; continue; }
    if (s.kind === 'dwell') { cur = end; continue; }
    if (s.kind === 'arc' && s.centre && start) {
      const [cx, cy] = s.centre, [sx, sy] = start, [ex, ey] = end;
      const r = Math.hypot(sx - cx, sy - cy);
      let a0 = Math.atan2(sy - cy, sx - cx), a1 = Math.atan2(ey - cy, ex - cx);
      const TAU = Math.PI * 2;
      let sweep = s.clockwise ? a0 - a1 : a1 - a0;
      sweep = ((sweep % TAU) + TAU) % TAU || TAU;
      const steps = Math.max(6, Math.ceil((sweep / TAU) * 48));
      let prev = start;
      for (let i = 1; i <= steps; i++) {
        const f = i / steps, a = a0 + (s.clockwise ? -1 : 1) * sweep * f;
        const p = [cx + r * Math.cos(a), cy + r * Math.sin(a)];
        out.push({ from: prev, to: p, travel: !!s.travel });
        prev = p;
      }
    } else if (s.kind === 'spline' && start) {
      const pts = splinePoints(s);
      if (pts) {
        for (let i = 1; i < pts.length; i++) {
          out.push({ from: [pts[i - 1][0], pts[i - 1][1]], to: [pts[i][0], pts[i][1]], travel: !!s.travel });
        }
      }
    } else if (start) {
      out.push({ from: start, to: end, travel: !!s.travel });
    }
    cur = end;
  }
  return out;
}

export function thumbnail(ops, wasm, params, size = 120) {
  const canvas = document.createElement('canvas');
  canvas.width = size; canvas.height = size;
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = BG; ctx.fillRect(0, 0, size, size);

  let ir;
  try { ir = JSON.parse(wasm.resolve_ir(JSON.stringify(ops), JSON.stringify(params))); }
  catch { return canvas.toDataURL('image/png'); }

  const lines = segmentsToXY(ir);
  if (!lines.length) return canvas.toDataURL('image/png');

  let lo = [Infinity, Infinity], hi = [-Infinity, -Infinity];
  const see = (p) => { for (let k = 0; k < 2; k++) { lo[k] = Math.min(lo[k], p[k]); hi[k] = Math.max(hi[k], p[k]); } };
  for (const l of lines) { see(l.from); see(l.to); }
  if (!isFinite(lo[0])) return canvas.toDataURL('image/png');

  const pad = size * 0.08;
  const span = Math.max(hi[0] - lo[0], hi[1] - lo[1], 1e-6);
  const scale = (size - 2 * pad) / span;
  const ox = (hi[0] + lo[0]) / 2, oy = (hi[1] + lo[1]) / 2;
  // map world (x,y) -> canvas, Y flipped (screen Y grows down), centred
  const X = (x) => size / 2 + (x - ox) * scale;
  const Y = (y) => size / 2 - (y - oy) * scale;

  ctx.lineWidth = Math.max(1, size / 120);
  ctx.lineJoin = 'round'; ctx.lineCap = 'round';
  // draw travels first (under), then extrude on top
  for (const want of [true, false]) {
    ctx.strokeStyle = want ? TRAVEL : EXTRUDE;
    ctx.globalAlpha = want ? 0.55 : 1;
    ctx.beginPath();
    for (const l of lines) {
      if (l.travel !== want) continue;
      ctx.moveTo(X(l.from[0]), Y(l.from[1]));
      ctx.lineTo(X(l.to[0]), Y(l.to[1]));
    }
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
  return canvas.toDataURL('image/png');
}
