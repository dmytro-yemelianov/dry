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
const SPEEDS = [0.25, 0.5, 1, 4, 16, 64];
const fmt = (v, d = 3) => (typeof v === 'number' ? v.toFixed(d) : v);

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

// ---- turn the resolved IR into timed moves (each tagged with its source segment / g-code line) ----
function buildMoves(ir) {
  const moves = [];
  let t = 0;
  const v3 = (a) => [a[0], a[1], a[2]];
  ir.segments.forEach((s, si) => {
    const from = s.start.some((c) => c == null) ? null : v3(s.start);
    const speed = s.speed || 0;
    if (s.kind === 'dwell') {
      const dt = s.dwell_s || 0;
      moves.push({ from, to: v3(s.end), travel: true, t0: t, t1: t + dt, seg: si });
      t += dt; return;
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
    } else {
      pts = from ? [from, v3(s.end)] : [v3(s.end)];
    }
    if (pts.length === 1) { moves.push({ from: null, to: pts[0], travel: s.travel, t0: t, t1: t, seg: si }); return; }
    const subLen = []; let sum = 0;
    for (let i = 1; i < pts.length; i++) {
      const L = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1], pts[i][2] - pts[i - 1][2]);
      subLen.push(L); sum += L;
    }
    const dtTotal = speed > 0 && (s.length || 0) > 0 ? ((s.length) / speed) * 60 : 0;
    for (let i = 1; i < pts.length; i++) {
      const dt = sum > 0 ? dtTotal * (subLen[i - 1] / sum) : 0;
      moves.push({ from: pts[i - 1], to: pts[i], travel: s.travel, t0: t, t1: t + dt, seg: si, w: s.width, h: s.height });
      t += dt;
    }
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
function buildBeads(moves) {
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

export function createViewer(cfg) {
  const {
    viewportEl, gcodeEl, explainEl, metricsEl, optimizeEl, verifyEl, gcodeMetaEl,
    playEl, scrubEl, clockEl, speedsEl, wasm, params,
    getMaxFlow = () => 0, getMinTemp = () => 0,
  } = cfg;

  const clock = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;

  function showExplain(line) {
    if (!explainEl) return;
    if (!line) { explainEl.innerHTML = '<span class="hint">Hover a line, or press play, to explain it.</span>'; return; }
    const toks = line.trim().split(/\s+/);
    const cmd = toks[0];
    const rows = toks.slice(1).map((t) => {
      const k = t[0], v = t.slice(1), d = PARAM_DESC[k];
      return `<tr><td class="k">${t}</td><td class="d">${d ? `${d[0]} (${d[1]})` : 'parameter'} = <b>${v}</b></td></tr>`;
    }).join('');
    explainEl.innerHTML = `<div><span class="cmd">${cmd}</span> — ${CMD_DESC[cmd] || 'g-code command'}</div>` +
      (rows ? `<table>${rows}</table>` : '');
  }

  // ---- playback state ----
  const P = { t: 0, totalT: 0, playing: false, speed: 1, moves: [], segStart: [], segEnd: [], activeRow: null };
  let GLINES = [], GROWS = [];

  // ---- three.js scene ----
  const V = { ready: false };
  function initScene() {
    const el = viewportEl;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(window.devicePixelRatio || 1);
    el.appendChild(renderer.domElement);
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x161b22);
    const camera = new THREE.PerspectiveCamera(45, 1, 0.1, 100000);
    camera.up.set(0, 0, 1);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    scene.add(new THREE.AmbientLight(0xffffff, 0.7));
    const dl = new THREE.DirectionalLight(0xffffff, 0.85); dl.position.set(0.5, -1, 1.6); scene.add(dl);

    const beadUniforms = { uTime: { value: 0 }, uPrinted: { value: new THREE.Color(0x58a6ff) },
                           uGhost: { value: new THREE.Color(0x21324a) } };
    const beadMat = new THREE.MeshLambertMaterial({ color: 0xffffff, side: THREE.DoubleSide });
    beadMat.onBeforeCompile = (sh) => {
      sh.uniforms.uTime = beadUniforms.uTime; sh.uniforms.uPrinted = beadUniforms.uPrinted; sh.uniforms.uGhost = beadUniforms.uGhost;
      sh.vertexShader = 'attribute float aTime;\nvarying float vTime;\n' +
        sh.vertexShader.replace('#include <begin_vertex>', '#include <begin_vertex>\n vTime = aTime;');
      sh.fragmentShader = 'uniform float uTime;\nuniform vec3 uPrinted;\nuniform vec3 uGhost;\nvarying float vTime;\n' +
        sh.fragmentShader.replace('#include <color_fragment>', '#include <color_fragment>\n diffuseColor.rgb = (vTime <= uTime) ? uPrinted : uGhost;');
    };
    const beads = new THREE.Mesh(new THREE.BufferGeometry(), beadMat);

    const ghostT = new THREE.LineSegments(new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0xf85149, transparent: true, opacity: 0.18 }));
    const printT = new THREE.LineSegments(new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0xf85149, transparent: true, opacity: 0.6 }));
    const head = new THREE.Mesh(new THREE.SphereGeometry(1, 16, 16),
      new THREE.MeshBasicMaterial({ color: 0x3fb950 }));
    scene.add(beads, ghostT, printT, head);
    function resize() {
      const w = el.clientWidth, h = el.clientHeight;
      if (!w || !h) return;
      renderer.setSize(w, h, false); camera.aspect = w / h; camera.updateProjectionMatrix();
    }
    window.addEventListener('resize', resize);
    Object.assign(V, { ready: true, renderer, scene, camera, controls, beads, beadUniforms, ghostT, printT, head, grid: null, resize });
    let last = performance.now();
    function frame(now) {
      const dtReal = (now - last) / 1000; last = now;
      if (P.playing && P.totalT > 0) {
        P.t += dtReal * P.speed;
        if (P.t >= P.totalT) { P.t = P.totalT; P.playing = false; if (playEl) playEl.textContent = '▶'; }
        syncPlayUI();
      }
      updatePrinted(); updateActiveLine(); controls.update(); renderer.render(scene, camera);
      requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
  }

  function setLine(obj, flat) {
    obj.geometry.dispose();
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute(flat, 3));
    obj.geometry = g;
  }

  function setModel(ir) {
    const { moves, totalT } = buildMoves(ir);
    P.moves = moves; P.totalT = totalT; P.t = 0; P.playing = false; P.activeRow = null;
    if (playEl) playEl.textContent = '▶';
    P.segStart = []; P.segEnd = [];
    for (const m of moves) { if (P.segStart[m.seg] === undefined) P.segStart[m.seg] = m.t0; P.segEnd[m.seg] = m.t1; }

    V.beads.geometry.dispose();
    V.beads.geometry = buildBeads(moves);
    V.beadUniforms.uTime.value = 0;

    const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
    const see = (p) => { for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], p[k]); hi[k] = Math.max(hi[k], p[k]); } };
    for (const m of moves) { if (m.from) see(m.from); see(m.to); }

    if (lo[0] === Infinity) { lo.fill(0); hi.fill(10); }
    const c = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
    const size = Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2], 1);
    if (V.grid) V.scene.remove(V.grid);
    const grid = new THREE.GridHelper(Math.ceil(size * 1.6), 16, 0x30363d, 0x21262d);
    grid.rotation.x = Math.PI / 2;
    const plateZ = lo[2] > 1e-6 ? 0 : lo[2] - size * 0.01;
    grid.position.set(c[0], c[1], plateZ);
    V.grid = grid; V.scene.add(grid);
    V.head.scale.setScalar(size * 0.012 + 0.05);
    V.controls.target.set(c[0], c[1], c[2]);
    V.camera.position.set(c[0] + size * 1.3, c[1] - size * 1.6, c[2] + size * 1.1);
    V.camera.near = Math.max(size * 0.02, 0.05); V.camera.far = size * 12; V.camera.updateProjectionMatrix();
    V.controls.update(); V.resize(); updatePrinted(); syncPlayUI();
  }

  function headPos() {
    const ms = P.moves; if (!ms.length) return [0, 0, 0];
    for (const m of ms) {
      if (P.t <= m.t1 || m === ms[ms.length - 1]) {
        const to = m.to, from = m.from || to;
        const f = m.t1 > m.t0 ? Math.min(1, Math.max(0, (P.t - m.t0) / (m.t1 - m.t0))) : 1;
        return lerp(from, to, f);
      }
    }
    return ms[ms.length - 1].to;
  }
  function activeSegAt(t) {
    const ms = P.moves; if (!ms.length) return null;
    for (const m of ms) if (t <= m.t1) return m.seg;
    return ms[ms.length - 1].seg;
  }
  function updatePrinted() {
    const t = P.t;
    V.beadUniforms.uTime.value = t;
    const pt = [], rt = [];
    for (const m of P.moves) {
      if (!m.from || !m.travel) continue;
      if (m.t1 <= t) pt.push(...m.from, ...m.to);
      else if (m.t0 >= t) rt.push(...m.from, ...m.to);
      else { const f = m.t1 > m.t0 ? (t - m.t0) / (m.t1 - m.t0) : 1, mid = lerp(m.from, m.to, f); pt.push(...m.from, ...mid); rt.push(...mid, ...m.to); }
    }
    setLine(V.printT, pt); setLine(V.ghostT, rt);
    const h = headPos(); V.head.position.set(h[0], h[1], h[2]);
    window.__lines = { beadVerts: V.beads.geometry.attributes.position ? V.beads.geometry.attributes.position.count : 0,
                       uTime: t, printedTravel: pt.length / 6, plateZ: V.grid ? V.grid.position.z : null };
  }
  function updateActiveLine() {
    const seg = activeSegAt(P.t);
    if (seg === P.activeRow) return;
    if (P.activeRow != null && GROWS[P.activeRow]) GROWS[P.activeRow].classList.remove('active');
    P.activeRow = seg;
    if (seg != null && GROWS[seg]) { GROWS[seg].classList.add('active'); GROWS[seg].scrollIntoView({ block: 'nearest' }); showExplain(GLINES[seg]); }
  }

  function syncPlayUI() {
    if (scrubEl) scrubEl.value = P.totalT > 0 ? Math.round((P.t / P.totalT) * 1000) : 0;
    if (clockEl) clockEl.textContent = `${clock(P.t)} / ${clock(P.totalT)}`;
    window.__play = { t: P.t, totalT: P.totalT, playing: P.playing, speed: P.speed, activeLine: P.activeRow };
  }
  function seekToLine(i) {
    P.playing = false; if (playEl) playEl.textContent = '▶';
    P.t = P.segEnd[i] ?? P.segStart[i] ?? 0;
    updatePrinted(); updateActiveLine(); syncPlayUI();
  }

  function buildSpeedButtons() {
    if (!speedsEl) return;
    for (const sp of SPEEDS) {
      const b = document.createElement('button');
      b.textContent = sp === 1 ? '1× realtime' : sp + '×';
      b.dataset.speed = sp;
      if (sp === P.speed) b.classList.add('active');
      b.addEventListener('click', () => {
        P.speed = sp;
        [...speedsEl.querySelectorAll('button')].forEach((x) => x.classList.toggle('active', +x.dataset.speed === sp));
        syncPlayUI();
      });
      speedsEl.appendChild(b);
    }
  }

  // ---- resolve an ops array + render every panel ----
  function show(ops, relativeE = true) {
    const opsJson = JSON.stringify(ops), paramsJson = JSON.stringify(params);
    const gcode = wasm.resolve_gcode(opsJson, paramsJson, relativeE);
    const m = JSON.parse(wasm.resolve_metrics(opsJson, paramsJson));
    const ir = JSON.parse(wasm.resolve_ir(opsJson, paramsJson));
    const optimizedIr = JSON.parse(wasm.resolve_optimized_ir(opsJson, paramsJson));

    GLINES = gcode;
    if (gcodeEl) {
      gcodeEl.innerHTML = gcode.map((l, i) =>
        `<div class="gline" data-i="${i}"><span class="ln">${i + 1}</span>${l}</div>`).join('');
      GROWS = [...gcodeEl.querySelectorAll('.gline')];
    }
    if (gcodeMetaEl) gcodeMetaEl.textContent = `${gcode.length} motion lines · ${relativeE ? 'relative' : 'absolute'} E`;
    showExplain(gcode[0]);

    if (metricsEl) {
      metricsEl.innerHTML = [
        ['segments', m.segment_count], ['print time (s)', fmt(m.print_time_s)],
        ['travel time (s)', fmt(m.travel_time_s)], ['total time (s)', fmt(m.total_time_s)],
        ['extruded vol (mm³)', fmt(m.extruded_volume)], ['filament (mm)', fmt(m.filament_length)],
        ['extrude dist (mm)', fmt(m.extruding_distance)], ['max flow (mm³/s)', fmt(m.max_flow_rate)],
      ].map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join('');
    }

    setModel(ir);

    if (optimizeEl) {
      const raw = ir.segments.length, opt = optimizedIr.segments.length, saved = raw - opt;
      optimizeEl.innerHTML = saved > 0
        ? `segments: ${raw} → ${opt} <span class="delta">(−${saved})</span>`
        : `segments: ${raw} → ${opt} <span class="none">(nothing to merge)</span>`;
    }

    let report = null;
    if (verifyEl) {
      const maxFlow = getMaxFlow() || 0, minTemp = getMinTemp() || 0;
      report = JSON.parse(wasm.resolve_verify(opsJson, paramsJson, maxFlow, minTemp));
      const findings = report.findings || [];
      verifyEl.innerHTML = findings.length
        ? findings.map((f) => `<div class="finding ${f.severity}"><span class="rule">${f.rule}</span>` +
            `${f.segment != null ? ` · seg ${f.segment}` : ''}<span class="msg">${f.message}</span></div>`).join('')
        : '<div class="ok">✓ no findings</div>';
    }

    window.__dry = { gcode, metrics: m, ir, optimizedIr,
                     rawSegments: ir.segments.length, optimizedSegments: optimizedIr.segments.length, report };
    return window.__dry;
  }

  // ---- wire up interaction ----
  initScene();
  buildSpeedButtons();
  if (gcodeEl) {
    gcodeEl.addEventListener('click', (e) => { const r = e.target.closest('.gline'); if (r) seekToLine(+r.dataset.i); });
    gcodeEl.addEventListener('mouseover', (e) => { const r = e.target.closest('.gline'); if (r) showExplain(GLINES[+r.dataset.i]); });
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

  return { show, seekToLine, _P: P, _V: V };
}
