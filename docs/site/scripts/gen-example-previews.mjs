#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { Design, tpms } from '@dry/sdk';

const siteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outputDir = path.join(siteRoot, 'public/reference/previews');
fs.mkdirSync(outputDir, { recursive: true });

const examples = [
  {
    slug: 'author',
    title: 'Author a path',
    ir: () => new Design()
      .geometry(0.6, 0.2).extruder(true)
      .point(10, 0, 0.2)
      .arc({ cx: 0, cy: 0, x: 0, y: 10 })
      .point(0, 20, 0.2)
      .ir(),
  },
  {
    slug: 'lower',
    title: 'Lower to the Dry IR',
    ir: () => new Design()
      .geometry(0.6, 0.2).extruder(true)
      .point(0, 0, 0.2).point(20, 0, 0.2).point(20, 20, 0.2).point(0, 20, 0.2).point(0, 0, 0.2)
      .ir(),
  },
  {
    slug: 'simulate',
    title: 'Simulate',
    ir: () => new Design()
      .geometry(0.6, 0.2).extruder(true).speed(1800)
      .point(0, 0, 0.2).point(50, 0, 0.2).point(50, 50, 0.2).point(0, 50, 0.2).point(0, 0, 0.2)
      .ir(),
  },
  {
    slug: 'verify',
    title: 'Verify',
    ir: () => new Design()
      .geometry(0.6, 0.2).extruder(true)
      .point(0, 0, 0.2).point(300, 0, 0.2)
      .ir(),
  },
  {
    slug: 'optimize',
    title: 'Optimize',
    ir: () => new Design()
      .geometry(0.6, 0.2).extruder(true)
      .point(0, 0, 0.2).arc({ cx: 25, cy: 0, x: 50, y: 0 }).point(50, 50, 0.2)
      .balancedIr('generic', { max_acceleration_mm_s2: 3000, max_junction_velocity_mm_s: 8 }),
  },
  {
    slug: 'generative',
    title: 'Generative',
    ir: () => tpms({ surface: 'gyroid', cellSize: 10, cellsX: 2, cellsY: 2, cellsZ: 1, layerHeight: 0.2 }).ir(),
  },
];

function esc(value) {
  return String(value).replace(/[&<>"]/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[char]));
}

function pointFrom(value) {
  return [value?.[0] ?? 0, value?.[1] ?? 0, value?.[2] ?? 0];
}

function segmentPoints(seg) {
  if (seg.kind === 'arc' && seg.centre) return arcPoints(seg);
  if (seg.kind === 'spline' && Array.isArray(seg.control_points) && seg.control_points.length) {
    return [pointFrom(seg.start), ...seg.control_points.map(pointFrom), pointFrom(seg.end)];
  }
  return [pointFrom(seg.start), pointFrom(seg.end)];
}

function arcPoints(seg) {
  const start = pointFrom(seg.start);
  const end = pointFrom(seg.end);
  const cx = seg.centre?.[0] ?? 0;
  const cy = seg.centre?.[1] ?? 0;
  const radius = Math.hypot(start[0] - cx, start[1] - cy);
  if (!Number.isFinite(radius) || radius <= 0) return [start, end];
  const a0 = Math.atan2(start[1] - cy, start[0] - cx);
  let a1 = Math.atan2(end[1] - cy, end[0] - cx);
  if (seg.clockwise && a1 > a0) a1 -= Math.PI * 2;
  if (!seg.clockwise && a1 < a0) a1 += Math.PI * 2;
  const points = [];
  for (let i = 0; i <= 36; i += 1) {
    const t = i / 36;
    const a = a0 + (a1 - a0) * t;
    points.push([cx + Math.cos(a) * radius, cy + Math.sin(a) * radius, start[2] + (end[2] - start[2]) * t]);
  }
  return points;
}

function boundsFor(segments) {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const seg of segments) {
    for (const [x, y] of segmentPoints(seg)) {
      minX = Math.min(minX, x);
      minY = Math.min(minY, y);
      maxX = Math.max(maxX, x);
      maxY = Math.max(maxY, y);
    }
  }
  if (!Number.isFinite(minX)) return { minX: 0, minY: 0, maxX: 1, maxY: 1 };
  return { minX, minY, maxX, maxY };
}

function renderPreview({ slug, title, ir }) {
  const toolpath = ir();
  const segments = toolpath.segments || [];
  const width = 760;
  const height = 430;
  const pad = 54;
  const bounds = boundsFor(segments);
  const spanX = Math.max(1e-6, bounds.maxX - bounds.minX);
  const spanY = Math.max(1e-6, bounds.maxY - bounds.minY);
  const scale = Math.min((width - pad * 2) / spanX, (height - pad * 2 - 36) / spanY);
  const ox = pad - bounds.minX * scale + ((width - pad * 2) - spanX * scale) / 2;
  const oy = pad - bounds.minY * scale + ((height - pad * 2 - 36) - spanY * scale) / 2 + 20;
  const tx = ([x, y]) => [ox + x * scale, height - (oy + y * scale)];
  const limited = segments.slice(0, 1200);
  const truncated = limited.length < segments.length;

  const lines = [];
  for (const seg of limited) {
    const pts = segmentPoints(seg).map(tx);
    if (pts.length < 2) continue;
    const d = pts.map(([x, y], index) => `${index === 0 ? 'M' : 'L'}${x.toFixed(2)} ${y.toFixed(2)}`).join(' ');
    const cls = seg.travel ? 'travel' : 'print';
    lines.push(`<path class="${cls}" d="${d}" />`);
  }

  const start = segments[0] ? tx(pointFrom(segments[0].start)) : [pad, height - pad];
  const endSeg = segments[segments.length - 1];
  const end = endSeg ? tx(pointFrom(endSeg.end)) : [width - pad, pad];
  const note = truncated ? `${limited.length}/${segments.length} segments shown` : `${segments.length} segments`;

  return `<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 ${width} ${height}">
  <title id="title">${esc(title)} preview</title>
  <desc id="desc">Rendered Dry toolpath preview for the ${esc(slug)} example.</desc>
  <defs>
    <linearGradient id="bg" x1="0" x2="1" y1="0" y2="1">
      <stop offset="0" stop-color="#101826" />
      <stop offset="1" stop-color="#07101b" />
    </linearGradient>
    <pattern id="grid" width="32" height="32" patternUnits="userSpaceOnUse">
      <path d="M32 0H0V32" fill="none" stroke="#26384f" stroke-width="1" opacity="0.42" />
    </pattern>
    <style>
      .title{font:700 24px ui-sans-serif,system-ui,-apple-system,Segoe UI,sans-serif;fill:#eff6ff}.meta{font:500 13px ui-monospace,SFMono-Regular,Menlo,monospace;fill:#91a4bd}.print{fill:none;stroke:#38a0ff;stroke-width:4;stroke-linecap:round;stroke-linejoin:round}.travel{fill:none;stroke:#8ca0b8;stroke-width:2;stroke-dasharray:7 7;stroke-linecap:round;opacity:.62}.dot-start{fill:#73e6a4}.dot-end{fill:#ffd166}.axis{stroke:#45617f;stroke-width:1.5;opacity:.7}
    </style>
  </defs>
  <rect width="${width}" height="${height}" rx="24" fill="url(#bg)" />
  <rect x="18" y="18" width="${width - 36}" height="${height - 36}" rx="18" fill="url(#grid)" opacity="0.95" />
  <path class="axis" d="M52 ${height - 52}H132M52 ${height - 52}V${height - 132}" />
  <text class="title" x="34" y="52">${esc(title)}</text>
  <text class="meta" x="34" y="76">${esc(note)} · generated from Dry IR</text>
  <g>${lines.join('\n    ')}</g>
  <circle class="dot-start" cx="${start[0].toFixed(2)}" cy="${start[1].toFixed(2)}" r="5" />
  <circle class="dot-end" cx="${end[0].toFixed(2)}" cy="${end[1].toFixed(2)}" r="5" />
</svg>
`;
}

for (const example of examples) {
  fs.writeFileSync(path.join(outputDir, `${example.slug}.svg`), renderPreview(example), 'utf8');
}

console.log(`generated ${examples.length} reference previews`);
