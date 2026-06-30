// Shared Catmull-Rom spline sampler for the Dry browser demo.
//
// Extracted verbatim (behaviour-preserving) from viewer.js and thumb.js so the 3D bead
// geometry and the 2D thumbnails sample splines identically — SPLINE_SAMPLES, catmullRom,
// and splinePoints are the single source of truth here. Changing the sample count or the
// algorithm alters rendering in both consumers, so keep this module stable.

export const SPLINE_SAMPLES = 16;

export function catmullRom(p0, p1, p2, p3, t) {
  const t2 = t * t, t3 = t2 * t, out = [0, 0, 0];
  for (let a = 0; a < 3; a++) {
    out[a] = 0.5 * ((2 * p1[a]) + (-p0[a] + p2[a]) * t +
      (2 * p0[a] - 5 * p1[a] + 4 * p2[a] - p3[a]) * t2 +
      (-p0[a] + 3 * p1[a] - 3 * p2[a] + p3[a]) * t3);
  }
  return out;
}

export function splinePoints(s) {
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
