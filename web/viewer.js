// Shared viewer module for the Dry browser demo.
//
// Extracted verbatim (behaviour-preserving) from the original index.html: the three.js scene,
// the width+height-accurate bead mesh + reveal shader, the simulated playback, and the
// g-code panel (clickable / synced / explained). Both index.html (the gallery) and
// blocks.html (the Blockly authoring page) import `createViewer` from here so there is a
// single source of truth for the viewport — no copy-paste between the two pages.
//
// Usage:
//   import { createViewer } from './viewer.js';
//   const v = createViewer({ viewportEl, gcodeEl, explainEl, metricsEl, optimizeEl, verifyEl,
//                            gcodeMetaEl, playEl, scrubEl, clockEl, speedsEl, wasm, params });
//   v.show(ops, relativeE);   // resolve + render everything for an L1 ops array
//
// `wasm` is the object of resolver fns: { resolve_gcode, resolve_metrics, resolve_ir,
// resolve_optimized_ir, resolve_verify }. `params` is RESOLVE_PARAMS. The verify thresholds
// are read from optional getters in the config (getMaxFlow / getMinTemp); panels that aren't
// passed an element are simply skipped.

import * as THREE from 'three';
import { OrbitControls } from './vendor/OrbitControls.js';

const TAU = Math.PI * 2;
const SPLINE_SAMPLES = 16;
const SPEEDS = [0.25, 0.5, 1, 4, 16, 64];
const AUTO_MESH_MOVE_LIMIT = 12000;
const ROUNDED_BEAD_SEGMENTS = 10;
const MAX_GCODE_DOM_ROWS = 8000;
const GCODE_FOCUS_HEAD_ROWS = 800;
const GCODE_FOCUS_TAIL_ROWS = 800;
const GCODE_FOCUS_CENTER_ROWS = MAX_GCODE_DOM_ROWS - GCODE_FOCUS_HEAD_ROWS - GCODE_FOCUS_TAIL_ROWS;
const VIEW_PANELS = [
  { key: 'iso', label: 'Iso' },
  { key: 'top', label: 'Top' },
  { key: 'front', label: 'Front' },
  { key: 'side', label: 'Side' },
];
const fmt = (v, d = 3) => (typeof v === 'number' ? v.toFixed(d) : v);
const cleanClass = (v) => String(v || '').replace(/[^a-z0-9_-]/gi, '');

function formatDuration(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0 s';
  if (seconds < 1) return `${seconds.toFixed(2)} s`;
  const rounded = Math.round(seconds);
  const h = Math.floor(rounded / 3600);
  const m = Math.floor((rounded % 3600) / 60);
  const s = rounded % 60;
  if (h > 0) return `${h} h ${String(m).padStart(2, '0')} min${s ? ` ${String(s).padStart(2, '0')} s` : ''}`;
  if (m > 0) return `${m} min ${String(s).padStart(2, '0')} s`;
  return `${s} s`;
}

function appendMetric(parent, label, value, unit = '') {
  const dt = document.createElement('dt');
  dt.className = 'metric-title';
  dt.textContent = label;
  const dd = document.createElement('dd');
  const number = document.createElement('span');
  number.className = 'metric-number';
  number.textContent = String(value);
  const unitEl = document.createElement('span');
  unitEl.className = 'metric-unit';
  unitEl.textContent = unit ? `[${unit}]` : '';
  dd.append(number, unitEl);
  parent.append(dt, dd);
}

function measure(profile, key, fn) {
  const t0 = performance.now();
  const value = fn();
  profile[key] = performance.now() - t0;
  return value;
}

// ---- g-code explanation ----
const CMD_DESC = {
  G0: 'rapid travel — reposition without extruding',
  G1: 'linear move — extrude in a straight line',
  G2: 'clockwise circular arc',
  G3: 'counter-clockwise circular arc',
  G4: 'dwell — pause in place',
};
const PARAM_DESC = {
  F: ['feedrate', 'mm/min'], X: ['target X', 'mm'], Y: ['target Y', 'mm'], Z: ['target Z', 'mm'],
  E: ['extruder position / amount', 'mm of filament'],
  I: ['arc centre ΔX from start', 'mm'], J: ['arc centre ΔY from start', 'mm'],
  A: ['rotary A axis', 'deg'], B: ['rotary B axis', 'deg'], C: ['rotary C axis', 'deg'],
  S: ['dwell time', 's'],
};

function parseGcodeTokens(line) {
  return String(line || '').trim().split(/\s+/).filter(Boolean);
}

function parseGcodeParam(line, key) {
  const token = parseGcodeTokens(line).find((part) => part[0] === key);
  if (!token) return null;
  const value = Number.parseFloat(token.slice(1));
  return Number.isFinite(value) ? value : null;
}

function gcodeCommandClass(cmd) {
  if (cmd === 'G0') return 'travel';
  if (cmd === 'G1') return 'linear';
  if (cmd === 'G2' || cmd === 'G3') return 'arc';
  if (cmd === 'G4') return 'dwell';
  return 'other';
}

function buildGcodeSections(gcode) {
  const sections = [];
  let currentZ = null;
  let layer = 0;
  const push = (line, label) => {
    if (sections[sections.length - 1]?.line === line) {
      sections[sections.length - 1].label = label;
      return;
    }
    sections.push({ line, label });
  };
  for (let i = 0; i < gcode.length; i++) {
    const z = parseGcodeParam(gcode[i], 'Z');
    if (z == null) {
      if (i === 0) push(0, 'Start');
      continue;
    }
    const key = Math.round(z * 1000) / 1000;
    if (currentZ == null || Math.abs(key - currentZ) > 1e-6) {
      currentZ = key;
      layer += 1;
      push(i, `Layer ${layer} [Z ${fmt(key, 3)} mm]`);
    }
  }
  if (!sections.length) sections.push({ line: 0, label: 'Start' });
  return sections;
}

function normalizeFindingMessage(message) {
  return String(message || '').replace(/[-+]?\d*\.?\d+(?:e[-+]?\d+)?/gi, '#');
}

function formatSegmentRanges(segments, maxRanges = 5) {
  const values = [...new Set(segments.filter(Number.isFinite).map((v) => Math.trunc(v)))]
    .sort((a, b) => a - b);
  if (!values.length) return '';
  const ranges = [];
  let start = values[0], prev = values[0];
  for (const value of values.slice(1)) {
    if (value === prev + 1) {
      prev = value;
      continue;
    }
    ranges.push([start, prev]);
    start = value;
    prev = value;
  }
  ranges.push([start, prev]);
  const shown = ranges.slice(0, maxRanges)
    .map(([a, b]) => (a === b ? String(a) : `${a}-${b}`));
  const hidden = ranges.length - shown.length;
  return `seg ${shown.join(', ')}${hidden > 0 ? ` +${hidden} ranges` : ''}`;
}

function groupFindings(findings) {
  const groups = new Map();
  for (const finding of findings || []) {
    const rule = String(finding.rule || 'finding');
    const severity = String(finding.severity || 'warning');
    const normalized = normalizeFindingMessage(finding.message);
    const key = `${severity}\u0000${rule}\u0000${normalized}`;
    if (!groups.has(key)) {
      groups.set(key, {
        rule,
        severity,
        normalized,
        count: 0,
        segments: [],
        samples: [],
        messages: new Set(),
      });
    }
    const group = groups.get(key);
    group.count += 1;
    if (Number.isFinite(finding.segment)) group.segments.push(finding.segment);
    const sampleKey = `${finding.segment ?? ''}\u0000${finding.message || ''}`;
    if (group.samples.length < 12 && !group.messages.has(sampleKey)) {
      group.samples.push({
        segment: finding.segment,
        message: finding.message || '',
      });
      group.messages.add(sampleKey);
    }
  }
  return [...groups.values()].sort((a, b) =>
    (b.severity === 'error') - (a.severity === 'error') ||
    b.count - a.count ||
    a.rule.localeCompare(b.rule));
}

// ---- turn the resolved IR into timed moves (each tagged with its source segment / g-code line) ----
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
  const start = [
    s.start[0] ?? 0,
    s.start[1] ?? 0,
    s.start[2] ?? 0,
  ];
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

function buildMoves(ir) {
  const moves = [];
  let t = 0, line = 0;
  const v3 = (a) => [a[0], a[1], a[2]];
  ir.segments.forEach((s, si) => {
    const from = s.start.some((c) => c == null) ? null : v3(s.start);
    const speed = s.speed || 0;
    if (s.kind === 'dwell') {
      const dt = s.dwell_s || 0;
      moves.push({ from, to: v3(s.end), travel: true, t0: t, t1: t + dt, seg: si, line });
      t += dt; line++; return;
    }
    let pts;
    if (s.kind === 'arc' && s.centre) {
      const [cx, cy] = s.centre, [sx, sy] = s.start, [ex, ey] = s.end;
      const sz = s.start[2] ?? 0, ez = s.end[2] ?? sz, r = Math.hypot(sx - cx, sy - cy);
      let a0 = Math.atan2(sy - cy, sx - cx), a1 = Math.atan2(ey - cy, ex - cx);
      let sweep = s.clockwise ? a0 - a1 : a1 - a0;
      sweep = ((sweep % TAU) + TAU) % TAU || TAU;
      const steps = Math.max(8, Math.ceil((sweep / TAU) * 64));
      pts = [];
      for (let i = 0; i <= steps; i++) {
        const f = i / steps, a = a0 + (s.clockwise ? -1 : 1) * sweep * f;
        pts.push([cx + r * Math.cos(a), cy + r * Math.sin(a), sz + (ez - sz) * f]);
      }
    } else if (s.kind === 'spline') {
      pts = splinePoints(s);
      if (!pts || pts.length <= 1) return;
    } else {
      pts = from ? [from, v3(s.end)] : [v3(s.end)];
    }
    if (pts.length === 1) { moves.push({ from: null, to: pts[0], travel: s.travel, t0: t, t1: t, seg: si, line }); line++; return; }
    const subLen = []; let sum = 0;
    for (let i = 1; i < pts.length; i++) {
      const L = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1], pts[i][2] - pts[i - 1][2]);
      subLen.push(L); sum += L;
    }
    const dtTotal = speed > 0 && (s.length || 0) > 0 ? ((s.length) / speed) * 60 : 0;
    for (let i = 1; i < pts.length; i++) {
      const dt = sum > 0 ? dtTotal * (subLen[i - 1] / sum) : 0;
      moves.push({ from: pts[i - 1], to: pts[i], travel: s.travel, t0: t, t1: t + dt, seg: si, line, w: s.width, h: s.height });
      t += dt;
      if (s.kind === 'spline') line++;
    }
    if (s.kind !== 'spline') line++;
  });
  return { moves, totalT: t };
}

// small vector helpers
const vsub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const vcross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const vlen = (a) => Math.hypot(a[0], a[1], a[2]);
const vnorm = (a) => { const l = vlen(a) || 1; return [a[0] / l, a[1] / l, a[2] / l]; };
const vmad = (p, d, s) => [p[0] + d[0] * s, p[1] + d[1] * s, p[2] + d[2] * s];
const lerp = (a, b, f) => [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];

// Build one merged mesh of bead boxes: each extruding move becomes a rectangular prism of its bead
// cross-section (width × height) oriented along the move, carrying a per-vertex `aTime` so the reveal
// shader can light up the printed portion.
function buildBoxBeads(moves) {
  const pos = [], nrm = [], tim = [], UP = [0, 0, 1];
  const push = (p, n, t) => { pos.push(p[0], p[1], p[2]); nrm.push(n[0], n[1], n[2]); tim.push(t); };
  const quad = (a, b, c, d, n, ta, tb, tc, td) => {
    push(a, n, ta); push(b, n, tb); push(c, n, tc); push(a, n, ta); push(c, n, tc); push(d, n, td);
  };
  for (const m of moves) {
    if (!m.from || m.travel) continue;
    const p0 = m.from, p1 = m.to, d = vsub(p1, p0), len = vlen(d);
    if (len < 1e-9) continue;
    const dir = [d[0] / len, d[1] / len, d[2] / len];
    let side = vcross(dir, UP); if (vlen(side) < 1e-6) side = vcross(dir, [1, 0, 0]);
    side = vnorm(side); const vn = vnorm(vcross(side, dir));
    const hw = (m.w || 0.6) / 2, hh = (m.h || 0.2) / 2, t0 = m.t0, t1 = m.t1;
    const C = (e, ss, uu) => vmad(vmad(e, side, hw * ss), vn, hh * uu);
    const a = { mm: C(p0, -1, -1), pm: C(p0, 1, -1), pp: C(p0, 1, 1), mp: C(p0, -1, 1) };
    const b = { mm: C(p1, -1, -1), pm: C(p1, 1, -1), pp: C(p1, 1, 1), mp: C(p1, -1, 1) };
    const neg = (v) => [-v[0], -v[1], -v[2]];
    quad(a.pm, b.pm, b.pp, a.pp, side, t0, t1, t1, t0);        // +side
    quad(a.mm, a.mp, b.mp, b.mm, neg(side), t0, t0, t1, t1);   // -side
    quad(a.mp, a.pp, b.pp, b.mp, vn, t0, t0, t1, t1);          // top
    quad(a.mm, b.mm, b.pm, a.pm, neg(vn), t0, t1, t1, t0);     // bottom
    quad(a.mm, a.pm, a.pp, a.mp, neg(dir), t0, t0, t0, t0);    // start cap
    quad(b.mm, b.mp, b.pp, b.pm, dir, t1, t1, t1, t1);         // end cap
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  g.setAttribute('aTime', new THREE.Float32BufferAttribute(tim, 1));
  return g;
}

function buildRoundedBeads(moves, radialSegments = ROUNDED_BEAD_SEGMENTS) {
  const pos = [], nrm = [], tim = [], UP = [0, 0, 1];
  const push = (p, n, t) => { pos.push(p[0], p[1], p[2]); nrm.push(n[0], n[1], n[2]); tim.push(t); };
  const tri = (a, b, c, na, nb, nc, ta, tb, tc) => {
    push(a, na, ta); push(b, nb, tb); push(c, nc, tc);
  };
  const rings = Math.max(6, radialSegments);
  for (const m of moves) {
    if (!m.from || m.travel) continue;
    const p0 = m.from, p1 = m.to, d = vsub(p1, p0), len = vlen(d);
    if (len < 1e-9) continue;
    const dir = [d[0] / len, d[1] / len, d[2] / len];
    let side = vcross(dir, UP); if (vlen(side) < 1e-6) side = vcross(dir, [1, 0, 0]);
    side = vnorm(side); const vn = vnorm(vcross(side, dir));
    const hw = (m.w || 0.6) / 2, hh = (m.h || 0.2) / 2;
    const ring0 = [], ring1 = [], normals = [];
    for (let i = 0; i < rings; i++) {
      const a = (i / rings) * TAU;
      const sx = Math.cos(a), uz = Math.sin(a);
      const normal = vnorm([side[0] * sx + vn[0] * uz, side[1] * sx + vn[1] * uz, side[2] * sx + vn[2] * uz]);
      normals.push(normal);
      const offset = [side[0] * hw * sx + vn[0] * hh * uz, side[1] * hw * sx + vn[1] * hh * uz, side[2] * hw * sx + vn[2] * hh * uz];
      ring0.push([p0[0] + offset[0], p0[1] + offset[1], p0[2] + offset[2]]);
      ring1.push([p1[0] + offset[0], p1[1] + offset[1], p1[2] + offset[2]]);
    }
    for (let i = 0; i < rings; i++) {
      const j = (i + 1) % rings;
      tri(ring0[i], ring1[i], ring1[j], normals[i], normals[i], normals[j], m.t0, m.t1, m.t1);
      tri(ring0[i], ring1[j], ring0[j], normals[i], normals[j], normals[j], m.t0, m.t1, m.t0);
      tri(p0, ring0[j], ring0[i], [-dir[0], -dir[1], -dir[2]], [-dir[0], -dir[1], -dir[2]], [-dir[0], -dir[1], -dir[2]], m.t0, m.t0, m.t0);
      tri(p1, ring1[i], ring1[j], dir, dir, dir, m.t1, m.t1, m.t1);
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  g.setAttribute('aTime', new THREE.Float32BufferAttribute(tim, 1));
  return g;
}

function buildTimedLineGeometry(moves, predicate) {
  const pos = [], endTimes = [];
  for (const m of moves) {
    if (!m.from || !predicate(m)) continue;
    pos.push(...m.from, ...m.to);
    endTimes.push(m.t1);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  return { geometry: g, endTimes, vertexCount: pos.length / 3 };
}

function completedTimedSegments(endTimes, t) {
  let lo = 0, hi = endTimes.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (endTimes[mid] <= t) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

function zLayerKey(value) {
  return Math.round((value || 0) * 1000) / 1000;
}

function moveLayerKey(move) {
  const z = move.to?.[2] ?? move.from?.[2] ?? 0;
  return zLayerKey(z);
}

function buildLayerKeys(moves) {
  const keys = new Set();
  for (const move of moves) {
    if (!move.from || move.travel) continue;
    keys.add(moveLayerKey(move));
  }
  if (!keys.size) {
    for (const move of moves) keys.add(moveLayerKey(move));
  }
  return [...keys].sort((a, b) => a - b);
}

function nearestLayerKey(keys, z) {
  if (!keys.length) return null;
  const target = zLayerKey(z);
  let best = keys[0];
  let bestDistance = Math.abs(best - target);
  for (const key of keys.slice(1)) {
    const distance = Math.abs(key - target);
    if (distance < bestDistance) {
      best = key;
      bestDistance = distance;
    }
  }
  return best;
}

export function createViewer(cfg) {
  const {
    viewportEl, gcodeEl, explainEl, metricsEl, optimizeEl, verifyEl, gcodeMetaEl,
    gcodeToolsEl, playEl, scrubEl, clockEl, speedsEl, resetViewEl, renderControlsEl, renderProfileEl, viewModeEl, wasm, params,
    getMaxFlow = () => 0, getMinTemp = () => 0,
  } = cfg;

  const clock = (s) => formatDuration(s);
  const R = {
    printed: true,
    planned: true,
    travel: true,
    travelLayer: false,
    toolhead: true,
    bed: true,
    mode: 'auto',
    effectiveMode: 'bead',
    viewMode: 'grid',
  };

  function showExplain(line) {
    if (!explainEl) return;
    explainEl.replaceChildren();
    if (!line) {
      const hint = document.createElement('span');
      hint.className = 'hint';
      hint.textContent = 'Hover a line, or press play, to explain it.';
      explainEl.appendChild(hint);
      return;
    }
    const toks = line.trim().split(/\s+/);
    const cmd = toks[0];
    const title = document.createElement('div');
    const cmdEl = document.createElement('span');
    cmdEl.className = 'cmd';
    cmdEl.textContent = cmd;
    title.append(cmdEl, ` — ${CMD_DESC[cmd] || 'g-code command'}`);
    explainEl.appendChild(title);
    if (toks.length <= 1) return;
    const table = document.createElement('table');
    for (const t of toks.slice(1)) {
      const k = t[0], v = t.slice(1), d = PARAM_DESC[k];
      const row = document.createElement('tr');
      const key = document.createElement('td');
      key.className = 'k';
      key.textContent = t;
      const desc = document.createElement('td');
      desc.className = 'd';
      const value = document.createElement('b');
      value.textContent = v;
      desc.append(`${d ? `${d[0]} (${d[1]})` : 'parameter'} = `, value);
      row.append(key, desc);
      table.appendChild(row);
    }
    explainEl.appendChild(table);
  }

  // ---- playback state ----
  const P = { t: 0, totalT: 0, playing: false, speed: 1, moves: [], moveEndTimes: [], segStart: [], segEnd: [], activeRow: null };
  let GLINES = [], GROWS = [], GSECTIONS = [], GMATCHES = [], GSEARCH = '', GMATCH_INDEX = -1;

  // ---- three.js scene ----
  const V = { ready: false, modelCenter: [0, 0, 0], modelSize: 10 };
  function initScene() {
    const el = viewportEl;
    if (!el.hasAttribute('tabindex')) el.tabIndex = 0;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 1.5));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.05;
    el.appendChild(renderer.domElement);
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x101820);
    const cameras = new Map(VIEW_PANELS.map(({ key }) => {
      const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100000);
      camera.up.set(0, 0, 1);
      return [key, camera];
    }));
    const controls = new OrbitControls(cameras.get('iso'), el);
    controls.enableDamping = true;
    controls.enablePan = true;
    controls.enableRotate = true;
    controls.enableZoom = true;
    controls.screenSpacePanning = true;
    const inputStats = { pointerdown: 0, pointermove: 0, wheel: 0 };
    el.addEventListener('pointerdown', (event) => {
      inputStats.pointerdown++;
      if (event.cancelable) event.preventDefault();
      if (typeof el.focus === 'function') el.focus({ preventScroll: true });
    }, { capture: true });
    el.addEventListener('pointermove', (event) => {
      if (!el.classList.contains('is-dragging')) return;
      inputStats.pointermove++;
      if (event.cancelable) event.preventDefault();
    }, { capture: true });
    el.addEventListener('wheel', (event) => {
      inputStats.wheel++;
      if (event.cancelable) event.preventDefault();
    }, { capture: true, passive: false });
    controls.addEventListener('start', () => el.classList.add('is-dragging'));
    controls.addEventListener('end', () => el.classList.remove('is-dragging'));
    controls.addEventListener('change', () => { V.needsRender = true; exposeDebugState(); });
    scene.add(new THREE.HemisphereLight(0xddeeff, 0x17202a, 0.82));
    const dl = new THREE.DirectionalLight(0xffffff, 1.15); dl.position.set(0.5, -1, 1.6); scene.add(dl);
    const fill = new THREE.DirectionalLight(0x8ecbff, 0.28); fill.position.set(-1.4, 0.7, 0.8); scene.add(fill);

    const beadUniforms = {
      uTime: { value: 0 },
      uPrinted: { value: new THREE.Color(0x58a6ff) },
      uGhost: { value: new THREE.Color(0x8ecbff) },
      uPrintedAlpha: { value: 1 },
      uGhostAlpha: { value: 0.2 },
    };
    const beadMat = new THREE.MeshStandardMaterial({
      color: 0xffffff,
      roughness: 0.58,
      metalness: 0.02,
      side: THREE.DoubleSide,
      transparent: true,
      depthWrite: false,
    });
    beadMat.onBeforeCompile = (sh) => {
      sh.uniforms.uTime = beadUniforms.uTime; sh.uniforms.uPrinted = beadUniforms.uPrinted; sh.uniforms.uGhost = beadUniforms.uGhost;
      sh.uniforms.uPrintedAlpha = beadUniforms.uPrintedAlpha; sh.uniforms.uGhostAlpha = beadUniforms.uGhostAlpha;
      sh.vertexShader = 'attribute float aTime;\nvarying float vTime;\n' +
        sh.vertexShader.replace('#include <begin_vertex>', '#include <begin_vertex>\n vTime = aTime;');
      sh.fragmentShader = 'uniform float uTime;\nuniform vec3 uPrinted;\nuniform vec3 uGhost;\nuniform float uPrintedAlpha;\nuniform float uGhostAlpha;\nvarying float vTime;\n' +
        sh.fragmentShader.replace(
          '#include <color_fragment>',
          '#include <color_fragment>\n float isPrinted = step(vTime, uTime);\n diffuseColor.rgb = mix(uGhost, uPrinted, isPrinted);\n diffuseColor.a *= mix(uGhostAlpha, uPrintedAlpha, isPrinted);',
        );
    };
    const beads = new THREE.Mesh(new THREE.BufferGeometry(), beadMat);
    const extrudeGhost = new THREE.LineSegments(new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0x8ecbff, transparent: true, opacity: 0.22 }));
    const extrudePrint = new THREE.LineSegments(new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0x58a6ff, transparent: true, opacity: 0.92 }));

    const ghostT = new THREE.LineSegments(new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0xf85149, transparent: true, opacity: 0.18 }));
    const printT = new THREE.LineSegments(new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0xf85149, transparent: true, opacity: 0.6 }));
    const head = new THREE.Mesh(new THREE.SphereGeometry(1, 16, 16),
      new THREE.MeshBasicMaterial({ color: 0x3fb950 }));
    scene.add(beads, extrudeGhost, extrudePrint, ghostT, printT, head);

    const viewGrid = document.createElement('div');
    viewGrid.className = 'view-grid-labels';
    viewGrid.setAttribute('aria-hidden', 'true');
    for (const panel of VIEW_PANELS) {
      const cell = document.createElement('div');
      cell.className = `view-grid-cell view-grid-cell-${panel.key}`;
      const label = document.createElement('span');
      label.textContent = panel.label;
      cell.appendChild(label);
      viewGrid.appendChild(cell);
    }
    el.appendChild(viewGrid);

    function resize() {
      const w = el.clientWidth, h = el.clientHeight;
      if (!w || !h) return;
      renderer.setSize(w, h, false);
      V.needsRender = true;
    }
    const resizeObserver = new ResizeObserver(() => resize());
    resizeObserver.observe(el);
    Object.assign(V, {
      ready: true, renderer, scene, cameras, controls, beads, beadUniforms,
      extrudeGhost, extrudePrint, ghostT, printT, head, grid: null, resize, inputStats,
      viewGrid,
      extrudeEndTimes: [], travelEndTimes: [], renderStats: null, showProfile: null,
      needsRender: true,
    });
    syncViewGrid();
    positionViewCameras({ saveState: true });
    let last = performance.now();
    let lastRenderedT = -1;
    function frame(now) {
      const dtReal = (now - last) / 1000; last = now;
      let timeChanged = false;
      if (P.playing && P.totalT > 0) {
        P.t += dtReal * P.speed;
        if (P.t >= P.totalT) { P.t = P.totalT; P.playing = false; if (playEl) playEl.textContent = '▶'; }
        syncPlayUI();
        timeChanged = true;
      }
      const controlsChanged = controls.update();
      if (timeChanged || Math.abs(P.t - lastRenderedT) > 1e-6) {
        updatePrinted();
        updateActiveLine();
        lastRenderedT = P.t;
        V.needsRender = true;
      }
      if (V.needsRender || controlsChanged) {
        V.needsRender = false;
        renderViews();
      }
      requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
  }

  function activeViewPanels() {
    return R.viewMode === 'iso' ? VIEW_PANELS.slice(0, 1) : VIEW_PANELS;
  }

  function cameraRect(index, width, height, total = VIEW_PANELS.length) {
    if (total === 1) return { x: 0, y: 0, w: width, h: height };
    const leftW = Math.floor(width / 2);
    const rightW = width - leftW;
    const bottomH = Math.floor(height / 2);
    const topH = height - bottomH;
    const col = index % 2;
    const row = Math.floor(index / 2);
    return {
      x: col === 0 ? 0 : leftW,
      y: row === 0 ? bottomH : 0,
      w: col === 0 ? leftW : rightW,
      h: row === 0 ? topH : bottomH,
    };
  }

  function renderViews() {
    if (!V.ready) return;
    const t0 = performance.now();
    const width = V.renderer.domElement.width;
    const height = V.renderer.domElement.height;
    if (!width || !height) return;
    const panels = activeViewPanels();
    V.renderer.setScissorTest(true);
    for (let i = 0; i < panels.length; i++) {
      const rect = cameraRect(i, width, height, panels.length);
      const camera = V.cameras.get(panels[i].key);
      camera.aspect = rect.w / Math.max(rect.h, 1);
      camera.updateProjectionMatrix();
      V.renderer.setViewport(rect.x, rect.y, rect.w, rect.h);
      V.renderer.setScissor(rect.x, rect.y, rect.w, rect.h);
      V.renderer.render(V.scene, camera);
    }
    V.renderer.setScissorTest(false);
    V.lastRenderMs = performance.now() - t0;
    if (V.profileDirty) {
      V.profileDirty = false;
      updateRenderProfile();
    }
    window.__viewPanels = panels.map(({ key }) => key);
    exposeDebugState();
  }

  function exposeDebugState() {
    if (!V.ready) return;
    const iso = V.cameras.get('iso');
    const target = V.controls ? V.controls.target : null;
    window.__viewerDebug = {
      panels: activeViewPanels().map(({ key }) => key),
      viewMode: R.viewMode,
      isoCameraPosition: iso ? iso.position.toArray() : null,
      target: target ? target.toArray() : null,
      distance: iso && target ? iso.position.distanceTo(target) : null,
      controlsElement: V.controls ? (V.controls.domElement.id || V.controls.domElement.tagName) : null,
      inputStats: V.inputStats || null,
      modelRevision: V.modelRevision || 0,
      render: V.renderStats || null,
    };
  }

  function moveIndexAt(t) {
    const times = P.moveEndTimes;
    if (!times.length) return -1;
    let lo = 0, hi = times.length - 1;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (t <= times[mid]) hi = mid;
      else lo = mid + 1;
    }
    return lo;
  }

  function positionViewCameras(options = {}) {
    if (!V.ready) return;
    const saveState = typeof options === 'boolean' ? options : Boolean(options.saveState);
    const preserveIso = typeof options === 'object' && Boolean(options.preserveIso);
    const [cx, cy, cz] = V.modelCenter;
    const size = Math.max(V.modelSize || 1, 1);
    const distance = size * 2.4;
    if (!preserveIso) V.controls.target.set(cx, cy, cz);
    const poses = {
      iso: { pos: [cx + size * 1.3, cy - size * 1.6, cz + size * 1.1], up: [0, 0, 1] },
      top: { pos: [cx, cy, cz + distance], up: [0, 1, 0] },
      front: { pos: [cx, cy - distance, cz], up: [0, 0, 1] },
      side: { pos: [cx + distance, cy, cz], up: [0, 0, 1] },
    };
    for (const [key, pose] of Object.entries(poses)) {
      const camera = V.cameras.get(key);
      if (!camera) continue;
      if (!(preserveIso && key === 'iso')) {
        camera.up.set(...pose.up);
        camera.position.set(...pose.pos);
      }
      camera.lookAt(V.controls.target);
      camera.near = Math.max(size * 0.02, 0.05);
      camera.far = size * 12;
      camera.updateProjectionMatrix();
    }
    V.controls.update();
    if (saveState && typeof V.controls.saveState === 'function') V.controls.saveState();
    V.needsRender = true;
    window.__viewPanels = activeViewPanels().map(({ key }) => key);
    exposeDebugState();
  }

  function syncViewGrid() {
    if (!V.viewGrid) return;
    const active = new Set(activeViewPanels().map(({ key }) => key));
    V.viewGrid.classList.toggle('is-iso-only', R.viewMode === 'iso');
    for (const panel of VIEW_PANELS) {
      const cell = V.viewGrid.querySelector(`.view-grid-cell-${panel.key}`);
      if (cell) cell.hidden = !active.has(panel.key);
    }
  }

  function resetView() {
    if (!V.ready) return;
    const wasDamping = V.controls.enableDamping;
    V.controls.enableDamping = false;
    V.controls.update();
    positionViewCameras({ saveState: true });
    if (typeof V.controls.reset === 'function') V.controls.reset();
    V.controls.update();
    V.controls.enableDamping = wasDamping;
    updatePrinted();
    V.needsRender = true;
    renderViews();
  }

  function replaceGeometry(obj, geometry) {
    if (obj.geometry && obj.geometry !== geometry) obj.geometry.dispose();
    obj.geometry = geometry;
  }

  function chooseRenderMode(moves) {
    const extrudingMoves = moves.reduce((count, m) => count + (m.from && !m.travel ? 1 : 0), 0);
    if (R.mode === 'auto') return extrudingMoves > AUTO_MESH_MOVE_LIMIT ? 'fast' : 'bead';
    return R.mode;
  }

  function rebuildMotionGeometry(moves) {
    const profile = {};
    const effectiveMode = chooseRenderMode(moves);
    R.effectiveMode = effectiveMode;
    const extrudingMoves = moves.reduce((count, m) => count + (m.from && !m.travel ? 1 : 0), 0);
    const travelMoves = moves.reduce((count, m) => count + (m.from && m.travel ? 1 : 0), 0);
    V.layerKeys = buildLayerKeys(moves);
    V.activeTravelLayerKey = null;

    if (effectiveMode === 'fast') {
      replaceGeometry(V.beads, new THREE.BufferGeometry());
      const extrude = measure(profile, 'extrudeLineGeometryMs', () => buildTimedLineGeometry(moves, (m) => !m.travel));
      replaceGeometry(V.extrudeGhost, extrude.geometry);
      const extrudePrint = buildTimedLineGeometry(moves, (m) => !m.travel);
      replaceGeometry(V.extrudePrint, extrudePrint.geometry);
      V.extrudeEndTimes = extrude.endTimes;
    } else {
      replaceGeometry(V.extrudeGhost, new THREE.BufferGeometry());
      replaceGeometry(V.extrudePrint, new THREE.BufferGeometry());
      V.extrudeEndTimes = [];
      replaceGeometry(V.beads, measure(profile, 'beadGeometryMs', () =>
        effectiveMode === 'realistic' ? buildRoundedBeads(moves) : buildBoxBeads(moves)));
      V.beadUniforms.uTime.value = P.t;
    }

    rebuildTravelGeometry(moves, profile);

    const beadVerts = V.beads.geometry.attributes.position ? V.beads.geometry.attributes.position.count : 0;
    const extrudeLineVerts = V.extrudeGhost.geometry.attributes.position ? V.extrudeGhost.geometry.attributes.position.count : 0;
    const travelLineVerts = V.ghostT.geometry.attributes.position ? V.ghostT.geometry.attributes.position.count : 0;
    V.renderStats = {
      requestedMode: R.mode,
      effectiveMode,
      moves: moves.length,
      extrudingMoves,
      travelMoves,
      visibleTravelMoves: V.travelEndTimes.length,
      travelLayer: V.activeTravelLayerKey,
      beadVerts,
      extrudeLineVerts,
      travelLineVerts,
      ...profile,
    };
    applyRenderVisibility();
    updateRenderProfile();
  }

  function rebuildTravelGeometry(moves, profile = {}) {
    const head = headPos();
    const layerKey = R.travelLayer ? nearestLayerKey(V.layerKeys || [], head[2]) : null;
    V.activeTravelLayerKey = layerKey;
    const predicate = (m) => m.travel && (!R.travelLayer || moveLayerKey(m) === layerKey);
    const travel = measure(profile, 'travelGeometryMs', () => buildTimedLineGeometry(moves, predicate));
    const travelPrint = buildTimedLineGeometry(moves, predicate);
    replaceGeometry(V.ghostT, travel.geometry);
    replaceGeometry(V.printT, travelPrint.geometry);
    V.travelEndTimes = travel.endTimes;
    if (V.renderStats) {
      V.renderStats.visibleTravelMoves = V.travelEndTimes.length;
      V.renderStats.travelLineVerts = V.ghostT.geometry.attributes.position ? V.ghostT.geometry.attributes.position.count : 0;
      V.renderStats.travelLayer = layerKey;
      Object.assign(V.renderStats, profile);
    }
    V.needsRender = true;
  }

  function applyRenderVisibility() {
    if (!V.ready) return;
    const meshMode = R.effectiveMode !== 'fast';
    V.beads.visible = meshMode && (R.printed || R.planned);
    V.beadUniforms.uPrintedAlpha.value = R.printed ? 1 : 0;
    V.beadUniforms.uGhostAlpha.value = R.planned ? 0.2 : 0;
    V.extrudeGhost.visible = !meshMode && R.planned;
    V.extrudePrint.visible = !meshMode && R.printed;
    V.ghostT.visible = R.travel && R.planned;
    V.printT.visible = R.travel && R.printed;
    V.head.visible = R.toolhead;
    if (V.grid) V.grid.visible = R.bed;
    V.needsRender = true;
  }

  function updateRenderProfile() {
    if (!renderProfileEl || !V.renderStats) return;
    const s = V.renderStats;
    const mode = s.requestedMode === 'auto' ? `auto → ${s.effectiveMode}` : s.effectiveMode;
    const geomMs = (s.beadGeometryMs ?? s.extrudeLineGeometryMs ?? 0) + (s.travelGeometryMs ?? 0);
    const renderMs = Number.isFinite(V.lastRenderMs) ? ` · frame ${V.lastRenderMs.toFixed(1)} ms` : '';
    const show = V.showProfile || {};
    const resolveMs = (show.resolveGcodeMs || 0) + (show.resolveMetricsMs || 0) +
      (show.resolveIrMs || 0) + (show.resolveOptimizedIrMs || 0) + (show.resolveVerifyMs || 0);
    const handling = resolveMs > 0 ? ` · resolve ${resolveMs.toFixed(1)} ms · g-code UI ${(show.gcodeDomMs || 0).toFixed(1)} ms` : '';
    const travelLayer = R.travelLayer && s.travelLayer != null ? ` · travel layer Z ${fmt(s.travelLayer, 3)}` : '';
    renderProfileEl.textContent =
      `render ${mode} · ${s.moves.toLocaleString()} moves · ${s.extrudingMoves.toLocaleString()} print / ` +
      `${(s.visibleTravelMoves ?? s.travelMoves).toLocaleString()} visible travel` +
      `${travelLayer} · geometry ${geomMs.toFixed(1)} ms${handling}${renderMs}`;
  }

  function setModel(ir, options = {}) {
    const profile = {};
    const preserveState = Boolean(options.preserveState && V.hasModel);
    const previousRatio = P.totalT > 0 ? Math.min(1, Math.max(0, P.t / P.totalT)) : 0;
    const wasPlaying = P.playing;
    const previousCenter = V.modelCenter ? [...V.modelCenter] : [0, 0, 0];
    const { moves, totalT } = measure(profile, 'buildMovesMs', () => buildMoves(ir));
    P.moves = moves;
    P.moveEndTimes = moves.map((m) => m.t1);
    P.totalT = totalT;
    P.t = preserveState ? previousRatio * totalT : 0;
    P.playing = preserveState && wasPlaying && totalT > 0 && P.t < totalT;
    P.activeRow = null;
    if (playEl) playEl.textContent = P.playing ? '⏸' : '▶';
    P.segStart = []; P.segEnd = [];
    for (const m of moves) { if (P.segStart[m.line] === undefined) P.segStart[m.line] = m.t0; P.segEnd[m.line] = m.t1; }

    rebuildMotionGeometry(moves);
    V.renderStats = { ...(V.renderStats || {}), ...profile };
    V.beadUniforms.uTime.value = P.t;

    const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
    const see = (p) => { for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], p[k]); hi[k] = Math.max(hi[k], p[k]); } };
    for (const m of moves) { if (m.from) see(m.from); see(m.to); }

    if (lo[0] === Infinity) { lo.fill(0); hi.fill(10); }
    const c = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
    const size = Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], 1);
    V.modelCenter = c;
    V.modelSize = size;
    if (V.grid) V.scene.remove(V.grid);
    const grid = new THREE.GridHelper(Math.ceil(size * 1.6), 16, 0x30363d, 0x21262d);
    grid.rotation.x = Math.PI / 2;
    const plateZ = lo[2] > 1e-6 ? 0 : lo[2] - size * 0.01;
    grid.position.set(c[0], c[1], plateZ);
    V.grid = grid; V.scene.add(grid);
    V.head.scale.setScalar(size * 0.012 + 0.05);
    if (V.grid) V.grid.visible = R.bed;
    if (preserveState) {
      const delta = [
        V.modelCenter[0] - previousCenter[0],
        V.modelCenter[1] - previousCenter[1],
        V.modelCenter[2] - previousCenter[2],
      ];
      V.controls.target.add(new THREE.Vector3(delta[0], delta[1], delta[2]));
      const iso = V.cameras.get('iso');
      if (iso) iso.position.add(new THREE.Vector3(delta[0], delta[1], delta[2]));
    }
    positionViewCameras({ saveState: !preserveState, preserveIso: preserveState });
    V.hasModel = true;
    V.modelRevision = (V.modelRevision || 0) + 1;
    V.profileDirty = true;
    V.resize(); updatePrinted(); syncPlayUI();
  }

  function headPos() {
    const ms = P.moves; if (!ms.length) return [0, 0, 0];
    const m = ms[Math.max(0, moveIndexAt(P.t))] || ms[ms.length - 1];
    const to = m.to, from = m.from || to;
    const f = m.t1 > m.t0 ? Math.min(1, Math.max(0, (P.t - m.t0) / (m.t1 - m.t0))) : 1;
    return lerp(from, to, f);
  }
  function activeSegAt(t) {
    const ms = P.moves; if (!ms.length) return null;
    const idx = moveIndexAt(t);
    return idx >= 0 ? ms[idx].line : ms[ms.length - 1].line;
  }

  function keepLineVisible(row) {
    if (!row || !gcodeEl) return;
    const rowTop = row.offsetTop;
    const rowBottom = rowTop + row.offsetHeight;
    const viewTop = gcodeEl.scrollTop;
    const viewBottom = viewTop + gcodeEl.clientHeight;
    if (rowTop < viewTop) gcodeEl.scrollTop = rowTop;
    else if (rowBottom > viewBottom) gcodeEl.scrollTop = rowBottom - gcodeEl.clientHeight;
  }

  function updatePrinted() {
    const t = P.t;
    V.beadUniforms.uTime.value = t;
    const h = headPos();
    if (R.travelLayer) {
      const layerKey = nearestLayerKey(V.layerKeys || [], h[2]);
      if (layerKey !== V.activeTravelLayerKey) rebuildTravelGeometry(P.moves);
    }
    const completedExtrude = completedTimedSegments(V.extrudeEndTimes || [], t);
    const completedTravel = completedTimedSegments(V.travelEndTimes || [], t);
    V.extrudePrint.geometry.setDrawRange(0, completedExtrude * 2);
    V.printT.geometry.setDrawRange(0, completedTravel * 2);
    V.head.position.set(h[0], h[1], h[2]);
    window.__lines = { beadVerts: V.beads.geometry.attributes.position ? V.beads.geometry.attributes.position.count : 0,
                       uTime: t, printedTravel: completedTravel, plateZ: V.grid ? V.grid.position.z : null,
                       extrudePreview: R.effectiveMode === 'fast',
                       ghostAlpha: V.beadUniforms.uGhostAlpha.value,
                       printedAlpha: V.beadUniforms.uPrintedAlpha.value };
  }
  function updateActiveLine() {
    const seg = activeSegAt(P.t);
    if (seg === P.activeRow) return;
    if (P.activeRow != null && GROWS[P.activeRow]) GROWS[P.activeRow].classList.remove('active');
    P.activeRow = seg;
    if (seg != null) {
      if (GROWS[seg]) {
        GROWS[seg].classList.add('active');
        keepLineVisible(GROWS[seg]);
      }
      showExplain(GLINES[seg]);
    }
  }

  function syncPlayUI() {
    if (scrubEl) scrubEl.value = P.totalT > 0 ? Math.round((P.t / P.totalT) * 1000) : 0;
    if (clockEl) clockEl.textContent = `${clock(P.t)} / ${clock(P.totalT)}`;
    window.__play = {
      t: P.t,
      ratio: P.totalT > 0 ? P.t / P.totalT : 0,
      totalT: P.totalT,
      playing: P.playing,
      speed: P.speed,
      activeLine: P.activeRow,
      modelRevision: V.modelRevision || 0,
    };
    exposeDebugState();
  }
  function seekToLine(i) {
    P.playing = false; if (playEl) playEl.textContent = '▶';
    P.t = P.segEnd[i] ?? P.segStart[i] ?? 0;
    updatePrinted(); updateActiveLine(); syncPlayUI();
  }

  function buildSpeedButtons() {
    if (!speedsEl) return;
    const select = document.createElement('select');
    select.className = 'speed-select';
    select.setAttribute('aria-label', 'Playback speed');
    for (const sp of SPEEDS) {
      const option = document.createElement('option');
      option.value = String(sp);
      option.textContent = `${sp}×`;
      select.appendChild(option);
    }
    select.value = String(P.speed);
    select.addEventListener('change', () => {
      P.speed = Number.parseFloat(select.value) || 1;
      syncPlayUI();
    });
    speedsEl.appendChild(select);
  }

  function bindRenderControls() {
    if (!renderControlsEl) return;
    const mode = renderControlsEl.querySelector('[data-render-mode]');
    const sync = (rebuild = false) => {
      for (const input of renderControlsEl.querySelectorAll('[data-render-layer]')) {
        R[input.dataset.renderLayer] = input.checked;
      }
      if (mode) R.mode = mode.value || 'auto';
      if (rebuild && V.hasModel) {
        rebuildMotionGeometry(P.moves);
        V.profileDirty = true;
      } else if (V.hasModel) {
        rebuildTravelGeometry(P.moves);
      }
      applyRenderVisibility();
      updatePrinted();
      renderViews();
      syncPlayUI();
    };
    for (const input of renderControlsEl.querySelectorAll('[data-render-layer]')) {
      input.checked = R[input.dataset.renderLayer] !== false;
      input.addEventListener('change', () => sync(false));
    }
    if (mode) {
      mode.value = R.mode;
      mode.addEventListener('change', () => sync(true));
    }
    sync(false);
  }

  function bindViewMode() {
    if (!viewModeEl) return;
    const sync = () => {
      R.viewMode = viewModeEl.value === 'iso' ? 'iso' : 'grid';
      syncViewGrid();
      V.needsRender = true;
      renderViews();
    };
    viewModeEl.value = R.viewMode;
    viewModeEl.addEventListener('input', sync);
    viewModeEl.addEventListener('change', sync);
    sync();
  }

  function renderGcodeLine(row, line) {
    const tokens = parseGcodeTokens(line);
    if (!tokens.length) return;
    const cmd = tokens[0];
    const cmdEl = document.createElement('span');
    cmdEl.className = `g-token g-cmd g-cmd-${cleanClass(gcodeCommandClass(cmd))}`;
    cmdEl.textContent = cmd;
    cmdEl.title = CMD_DESC[cmd] || 'g-code command';
    row.appendChild(cmdEl);
    for (const token of tokens.slice(1)) {
      const key = token[0];
      const value = token.slice(1);
      const desc = PARAM_DESC[key];
      const pill = document.createElement('span');
      pill.className = `g-token g-param g-param-${cleanClass(key)}`;
      pill.title = desc ? `${desc[0]} [${desc[1]}]` : 'parameter';
      const keyEl = document.createElement('span');
      keyEl.className = 'g-param-key';
      keyEl.textContent = key;
      const valueEl = document.createElement('span');
      valueEl.className = 'g-param-value';
      valueEl.textContent = value;
      pill.append(keyEl, valueEl);
      row.appendChild(pill);
    }
  }

  function addRange(ranges, start, end, total) {
    const s = Math.max(0, Math.min(total, Math.floor(start)));
    const e = Math.max(s, Math.min(total, Math.ceil(end)));
    if (e > s) ranges.push([s, e]);
  }

  function mergedRanges(gcode, focusLine) {
    const total = gcode.length;
    if (total <= MAX_GCODE_DOM_ROWS) return [[0, total]];
    const ranges = [];
    if (focusLine == null) {
      const keepHead = Math.floor(MAX_GCODE_DOM_ROWS * 0.58);
      addRange(ranges, 0, keepHead, total);
      addRange(ranges, total - (MAX_GCODE_DOM_ROWS - keepHead), total, total);
    } else {
      const half = Math.floor(GCODE_FOCUS_CENTER_ROWS / 2);
      addRange(ranges, 0, GCODE_FOCUS_HEAD_ROWS, total);
      addRange(ranges, focusLine - half, focusLine + half, total);
      addRange(ranges, total - GCODE_FOCUS_TAIL_ROWS, total, total);
    }
    ranges.sort((a, b) => a[0] - b[0]);
    const merged = [];
    for (const range of ranges) {
      const last = merged[merged.length - 1];
      if (last && range[0] <= last[1]) last[1] = Math.max(last[1], range[1]);
      else merged.push([...range]);
    }
    return merged;
  }

  function appendGcodeOmitted(fragment, count) {
    if (count <= 0) return;
    const omitted = document.createElement('div');
    omitted.className = 'gline omitted';
    omitted.textContent = `... ${count.toLocaleString()} generated lines omitted in the browser panel ...`;
    fragment.appendChild(omitted);
  }

  function renderGcodeLines(gcode, focusLine = null) {
    if (!gcodeEl) return;
    gcodeEl.replaceChildren();
    GROWS = [];
    const fragment = document.createDocumentFragment();
    const sectionByLine = new Map(GSECTIONS.map((section) => [section.line, section]));
    const addLine = (i) => {
      const section = sectionByLine.get(i);
      if (section) {
        const header = document.createElement('button');
        header.type = 'button';
        header.className = 'g-section';
        header.dataset.i = String(i);
        header.textContent = section.label;
        fragment.appendChild(header);
      }
      const row = document.createElement('div');
      row.className = 'gline';
      row.dataset.i = String(i);
      row.title = gcode[i];
      const ln = document.createElement('span');
      ln.className = 'ln';
      ln.textContent = String(i + 1);
      row.appendChild(ln);
      renderGcodeLine(row, gcode[i]);
      GROWS[i] = row;
      fragment.appendChild(row);
    };
    let cursor = 0;
    for (const [start, end] of mergedRanges(gcode, focusLine)) {
      appendGcodeOmitted(fragment, start - cursor);
      for (let i = start; i < end; i++) addLine(i);
      cursor = end;
    }
    appendGcodeOmitted(fragment, gcode.length - cursor);
    gcodeEl.appendChild(fragment);
    applyGcodeSearchMarks();
  }

  function setSearchStatus(text) {
    if (!gcodeToolsEl) return;
    const status = gcodeToolsEl.querySelector('[data-gcode-search-status]');
    if (status) status.textContent = text;
  }

  function applyGcodeSearchMarks() {
    if (!GSEARCH) {
      for (const row of GROWS) if (row) row.classList.remove('match', 'selected-match');
      setSearchStatus('');
      return;
    }
    for (let i = 0; i < GROWS.length; i++) {
      const row = GROWS[i];
      if (!row) continue;
      const isMatch = GLINES[i]?.toLowerCase().includes(GSEARCH);
      row.classList.toggle('match', Boolean(isMatch));
      row.classList.toggle('selected-match', GMATCHES[GMATCH_INDEX] === i);
    }
    setSearchStatus(GMATCHES.length ? `${GMATCH_INDEX + 1}/${GMATCHES.length}` : '0/0');
  }

  function gotoGcodeLine(line) {
    if (!Number.isFinite(line) || line < 0 || line >= GLINES.length) return;
    if (!GROWS[line]) renderGcodeLines(GLINES, line);
    seekToLine(line);
    applyGcodeSearchMarks();
  }

  function runGcodeSearch(query) {
    GSEARCH = String(query || '').trim().toLowerCase();
    GMATCHES = [];
    GMATCH_INDEX = -1;
    if (GSEARCH) {
      for (let i = 0; i < GLINES.length; i++) {
        if (GLINES[i].toLowerCase().includes(GSEARCH)) GMATCHES.push(i);
      }
      if (GMATCHES.length) {
        GMATCH_INDEX = 0;
        gotoGcodeLine(GMATCHES[GMATCH_INDEX]);
      }
    }
    applyGcodeSearchMarks();
  }

  function moveGcodeSearch(delta) {
    if (!GMATCHES.length) return;
    GMATCH_INDEX = (GMATCH_INDEX + delta + GMATCHES.length) % GMATCHES.length;
    gotoGcodeLine(GMATCHES[GMATCH_INDEX]);
  }

  function renderGcodeTools() {
    if (!gcodeToolsEl) return;
    gcodeToolsEl.replaceChildren();
    const jump = document.createElement('select');
    jump.className = 'gcode-jump';
    jump.setAttribute('aria-label', 'Jump to G-code section');
    for (const section of GSECTIONS) {
      const option = document.createElement('option');
      option.value = String(section.line);
      option.textContent = section.label;
      jump.appendChild(option);
    }
    jump.addEventListener('change', () => gotoGcodeLine(Number.parseInt(jump.value, 10)));

    const search = document.createElement('input');
    search.type = 'search';
    search.className = 'gcode-search';
    search.placeholder = 'Search G-code';
    search.setAttribute('aria-label', 'Search G-code');
    search.addEventListener('input', () => runGcodeSearch(search.value));

    const prev = document.createElement('button');
    prev.type = 'button';
    prev.className = 'gcode-search-button';
    prev.textContent = 'Prev';
    prev.addEventListener('click', () => moveGcodeSearch(-1));

    const next = document.createElement('button');
    next.type = 'button';
    next.className = 'gcode-search-button';
    next.textContent = 'Next';
    next.addEventListener('click', () => moveGcodeSearch(1));

    const status = document.createElement('span');
    status.className = 'gcode-search-status';
    status.dataset.gcodeSearchStatus = 'true';

    gcodeToolsEl.append(jump, search, prev, next, status);
  }

  function renderFindingGroup(group) {
    const row = document.createElement('div');
    row.classList.add('finding');
    const severity = cleanClass(group.severity);
    if (severity) row.classList.add(severity);

    const head = document.createElement('div');
    head.className = 'finding-head';

    const rule = document.createElement('span');
    rule.className = 'rule';
    rule.textContent = group.rule;
    head.appendChild(rule);

    if (group.count > 1) {
      const count = document.createElement('span');
      count.className = 'finding-count';
      count.textContent = `${group.count.toLocaleString()} findings`;
      head.appendChild(count);
    }

    const segmentSummary = formatSegmentRanges(group.segments);
    if (segmentSummary) {
      const seg = document.createElement('span');
      seg.className = 'finding-segments';
      seg.textContent = segmentSummary;
      head.appendChild(seg);
    }

    const msg = document.createElement('span');
    msg.className = 'msg';
    msg.textContent = group.samples[0]?.message || group.normalized;
    row.append(head, msg);

    if (group.count > 1) {
      const details = document.createElement('details');
      details.className = 'finding-details';
      const summary = document.createElement('summary');
      summary.textContent = `show ${group.samples.length.toLocaleString()} examples`;
      details.appendChild(summary);
      const list = document.createElement('div');
      list.className = 'finding-samples';
      for (const sample of group.samples) {
        const item = document.createElement('div');
        item.className = 'finding-sample';
        if (sample.segment != null) {
          const seg = document.createElement('span');
          seg.className = 'finding-sample-seg';
          seg.textContent = `seg ${sample.segment}`;
          item.appendChild(seg);
        }
        const text = document.createElement('span');
        text.textContent = sample.message;
        item.appendChild(text);
        list.appendChild(item);
      }
      if (group.count > group.samples.length) {
        const omitted = document.createElement('div');
        omitted.className = 'finding-sample omitted';
        omitted.textContent = `${(group.count - group.samples.length).toLocaleString()} more grouped findings omitted from the browser panel`;
        list.appendChild(omitted);
      }
      details.appendChild(list);
      row.appendChild(details);
    }

    return row;
  }

  // ---- resolve an ops array + render every panel ----
  function show(ops, relativeE = true) {
    const profile = { ops: ops.length };
    const opsJson = JSON.stringify(ops), paramsJson = JSON.stringify(params);
    const gcode = measure(profile, 'resolveGcodeMs', () => wasm.resolve_gcode(opsJson, paramsJson, relativeE, false, false, 'ab'));
    const m = measure(profile, 'resolveMetricsMs', () => JSON.parse(wasm.resolve_metrics(opsJson, paramsJson)));
    const ir = measure(profile, 'resolveIrMs', () => JSON.parse(wasm.resolve_ir(opsJson, paramsJson)));
    const optimizedIr = measure(profile, 'resolveOptimizedIrMs', () => JSON.parse(wasm.resolve_optimized_ir(opsJson, paramsJson)));

    GLINES = gcode;
    GSECTIONS = buildGcodeSections(gcode);
    GMATCHES = [];
    GSEARCH = '';
    GMATCH_INDEX = -1;
    renderGcodeTools();
    measure(profile, 'gcodeDomMs', () => renderGcodeLines(gcode));
    if (gcodeMetaEl) {
      const shown = Math.min(gcode.length, MAX_GCODE_DOM_ROWS);
      gcodeMetaEl.textContent = `${gcode.length.toLocaleString()} motion lines` +
        (shown < gcode.length ? ` · showing ${shown.toLocaleString()}` : '') +
        ` · ${relativeE ? 'relative' : 'absolute'} E`;
    }
    showExplain(gcode[0]);

    if (metricsEl) {
      metricsEl.replaceChildren();
      appendMetric(metricsEl, 'segments', m.segment_count.toLocaleString(), 'count');
      appendMetric(metricsEl, 'print time', formatDuration(m.print_time_s), 'time');
      appendMetric(metricsEl, 'travel time', formatDuration(m.travel_time_s), 'time');
      appendMetric(metricsEl, 'total time', formatDuration(m.total_time_s), 'time');
      appendMetric(metricsEl, 'extruded vol', fmt(m.extruded_volume), 'mm³');
      appendMetric(metricsEl, 'filament', fmt(m.filament_length), 'mm');
      appendMetric(metricsEl, 'extrude dist', fmt(m.extruding_distance), 'mm');
      appendMetric(metricsEl, 'max flow', fmt(m.max_flow_rate), 'mm³/s');
    }

    measure(profile, 'setModelMs', () => setModel(ir, { preserveState: V.hasModel }));

    if (optimizeEl) {
      const raw = ir.segments.length, opt = optimizedIr.segments.length, saved = raw - opt;
      optimizeEl.replaceChildren();
      optimizeEl.append(`segments: ${raw} → ${opt} `);
      const note = document.createElement('span');
      note.className = saved > 0 ? 'delta' : 'none';
      note.textContent = saved > 0 ? `(−${saved})` : '(nothing to merge)';
      optimizeEl.appendChild(note);
    }

    let report = null;
    if (verifyEl) {
      const maxFlow = getMaxFlow() || 0, minTemp = getMinTemp() || 0;
      const bounds = cfg.getBounds ? cfg.getBounds() : '';
      const monotonicZ = cfg.getMonotonicZ ? cfg.getMonotonicZ() : false;
      const speedRange = cfg.getSpeedRange ? cfg.getSpeedRange() : '';
      report = measure(profile, 'resolveVerifyMs', () =>
        JSON.parse(wasm.resolve_verify(opsJson, paramsJson, maxFlow, minTemp, bounds, monotonicZ, speedRange)));
      const findings = report.findings || [];
      verifyEl.replaceChildren();
      if (findings.length) {
        for (const group of groupFindings(findings)) {
          verifyEl.appendChild(renderFindingGroup(group));
        }
      } else {
        const ok = document.createElement('div');
        ok.className = 'ok';
        ok.textContent = '✓ no findings';
        verifyEl.appendChild(ok);
      }
    }

    window.__dry = { gcode, metrics: m, ir, optimizedIr,
                     rawSegments: ir.segments.length, optimizedSegments: optimizedIr.segments.length, report };
    window.__dryProfile = { ...(V.renderStats || {}), ...profile };
    V.showProfile = window.__dryProfile;
    updateRenderProfile();
    return window.__dry;
  }

  // ---- wire up interaction ----
  initScene();
  buildSpeedButtons();
  bindRenderControls();
  bindViewMode();
  if (gcodeEl) {
    gcodeEl.addEventListener('click', (e) => {
      const section = e.target.closest('.g-section[data-i]');
      if (section) {
        gotoGcodeLine(+section.dataset.i);
        return;
      }
      const r = e.target.closest('.gline[data-i]');
      if (r) seekToLine(+r.dataset.i);
    });
    gcodeEl.addEventListener('mouseover', (e) => { const r = e.target.closest('.gline[data-i]'); if (r) showExplain(GLINES[+r.dataset.i]); });
  }
  if (playEl) playEl.addEventListener('click', () => {
    if (P.totalT <= 0) return;
    if (P.t >= P.totalT) P.t = 0;
    P.playing = !P.playing; playEl.textContent = P.playing ? '⏸' : '▶';
  });
  if (scrubEl) scrubEl.addEventListener('input', (e) => {
    P.playing = false; if (playEl) playEl.textContent = '▶';
    P.t = (e.target.value / 1000) * P.totalT; updatePrinted(); updateActiveLine(); syncPlayUI();
  });
  if (resetViewEl) resetViewEl.addEventListener('click', resetView);

  return { show, seekToLine, setView: positionViewCameras, _P: P, _V: V };
}
