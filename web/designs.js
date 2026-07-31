// Demo designs authored as Dry L1 ops, plus generated research examples.
// Each is an array of ops: geometry / extruder / speed / move / arc. The same op vocabulary the
// Python SDK and the conformance oracle use.
import { starPolygonLatticeOps } from './lattice-research.js';
import { tpmsOps } from './tpms-engine.js';
import { FULLCONTROL_DESIGNS } from './fullcontrol-gallery.generated.js';

const TAU = Math.PI * 2;
const G = (w, h) => ({ op: 'geometry', width: w, height: h });
const ON = { op: 'extruder', on: true };
const OFF = { op: 'extruder', on: false };
const SPEED = (v) => ({ op: 'speed', print: v });
const M = (x, y, z) => ({ op: 'move', x, y, z });
const ARC = (cx, cy, x, y, z, clockwise) => ({ op: 'arc', cx, cy, x, y, z, clockwise });
const TEMP = (c) => ({ op: 'temperature', nozzle: c });
const FAN = (v) => ({ op: 'fan', speed: v });
const RETRACT = (distance, speed) => ({ op: 'retract', distance, speed });
const UNRETRACT = (distance, speed) => ({ op: 'unretract', distance, speed });
const SPLINE = (points) => ({ op: 'spline', points });
const gcd = (a, b) => { a = Math.abs(a); b = Math.abs(b); while (b) { [a, b] = [b, a % b]; } return a; };

function square(side = 10, z = 0.2) {
  return [G(0.6, 0.2), ON, M(0, 0, z), M(side, 0, z), M(side, side, z), M(0, side, z), M(0, 0, z)];
}

// vase-mode helix — points computed here and passed as explicit coords (line length is
// sqrt/division, IEEE-deterministic across native/wasm, so the bytes match the oracle).
function spiralVase(radius = 15, height = 1.5, layerH = 0.3, perLayer = 24, cx = 50, cy = 50) {
  const ops = [G(0.6, 0.2), ON];
  const n = Math.round((height / layerH) * perLayer);
  for (let i = 0; i <= n; i++) {
    const frac = i / perLayer;
    const a = frac * TAU;
    ops.push(M(cx + radius * Math.cos(a), cy + radius * Math.sin(a), 0.2 + frac * layerH));
  }
  return ops;
}

// native G2/G3 arcs at varying speed
function arcsMix(radius = 10, gap = 10, speed = 1800, z = 0.4) {
  return [
    G(0.6, 0.2), ON, M(radius * 2, 5, z), SPEED(speed),
    ARC(radius, 5, 0, 5, null, true),
    M(0, 5 + gap, z),
    ARC(radius, 5 + gap, radius * 2, 5 + gap, null, true),
  ];
}

// a five-point star drawn as one continuous extruding stroke (my own parametric design)
function star(points = 5, outer = 20, inner = 8, cx = 50, cy = 50, z = 0.2) {
  const ops = [G(0.6, 0.2), ON];
  const verts = [];
  for (let i = 0; i < points * 2; i++) {
    const r = i % 2 === 0 ? outer : inner;
    const a = (i / (points * 2)) * TAU - Math.PI / 2;
    verts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
  }
  ops.push(M(verts[0][0], verts[0][1], z));
  for (let i = 1; i < verts.length; i++) ops.push(M(verts[i][0], verts[i][1], z));
  ops.push(M(verts[0][0], verts[0][1], z));
  return ops;
}

// A back-and-forth comb whose long straight runs are authored as several short collinear hops.
// Each rung is one straight line broken into `subdiv` equal moves that share all process state, so
// the optimizer (merge_collinear) collapses each run back into a single move — a visible reduction.
function collinearComb(rungs = 6, len = 30, pitch = 4, subdiv = 5, x0 = 10, y0 = 10, z = 0.2) {
  const ops = [G(0.6, 0.2), ON];
  let x = x0;
  ops.push(M(x, y0, z));
  for (let r = 0; r < rungs; r++) {
    const y = y0 + r * pitch;
    const dir = r % 2 === 0 ? 1 : -1; // serpentine: alternate sweep direction
    const xEnd = x + dir * len;
    // walk the straight rung in `subdiv` collinear hops (the redundant intermediate points)
    for (let k = 1; k <= subdiv; k++) {
      ops.push(M(x + dir * len * (k / subdiv), y, z));
    }
    x = xEnd;
    if (r < rungs - 1) ops.push(M(x, y0 + (r + 1) * pitch, z)); // connector to the next rung
  }
  return ops;
}

// A multi-layer square tower: each layer is a perimeter, with the extruder OFF for the lift/travel to
// the next layer's start — so the g-code has real G0 travels between G1 perimeters, like a real print.
function layeredTower(side = 20, layers = 10, layerH = 0.3, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), SPEED(1200)];
  const h = side / 2;
  const corner = [[cx - h, cy - h], [cx + h, cy - h], [cx + h, cy + h], [cx - h, cy + h]];
  for (let L = 0; L < layers; L++) {
    const z = z0 + L * layerH;
    ops.push(OFF, M(corner[0][0], corner[0][1], z), ON); // travel to the layer start, then extrude
    for (let i = 1; i <= 4; i++) ops.push(M(corner[i % 4][0], corner[i % 4][1], z)); // square perimeter
  }
  return ops;
}

// A square tower printed with filament retraction: after each perimeter the filament is retracted,
// the extruder lifts and travels (OFF) to the next layer start, then unretracts and resumes — the
// canonical retract / travel / unretract cycle that suppresses oozing/stringing between layers.
function retractionTower(side = 16, layers = 6, layerH = 0.4, retractDist = 1.2, retractSpeed = 2400, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), SPEED(1200)];
  const h = side / 2;
  const corner = [[cx - h, cy - h], [cx + h, cy - h], [cx + h, cy + h], [cx - h, cy + h]];
  for (let L = 0; L < layers; L++) {
    const z = z0 + L * layerH;
    ops.push(OFF, M(corner[0][0], corner[0][1], z));            // travel to the layer start
    if (L > 0) ops.push(UNRETRACT(retractDist, retractSpeed));  // prime the filament before extruding
    ops.push(ON);
    for (let i = 1; i <= 4; i++) ops.push(M(corner[i % 4][0], corner[i % 4][1], z)); // square perimeter
    if (L < layers - 1) ops.push(RETRACT(retractDist, retractSpeed)); // retract before the lift/travel
  }
  return ops;
}

// A smooth S-curve drawn as a single native Catmull-Rom spline op: travel to the curve start, then one
// spline through control points sampled along a full sine wave (one up lobe + one down lobe).
function splineSCurve(length = 64, amp = 16, points = 6, cx = 50, cy = 50, z = 0.2) {
  const x0 = cx - length / 2;
  const ctrl = [];
  for (let i = 0; i <= points; i++) {
    const f = i / points;
    ctrl.push([x0 + f * length, cy + amp * Math.sin(f * TAU), z]);
  }
  return [G(0.6, 0.2), OFF, M(ctrl[0][0], ctrl[0][1], z), ON, SPLINE(ctrl.slice(1))];
}

// A rectangular panel: one perimeter, a travel, then a serpentine (zig-zag) infill — a complete layer.
function infillPanel(w = 26, h = 18, gap = 2, cx = 50, cy = 50, z = 0.2) {
  const ops = [G(0.6, 0.2), ON];
  const x0 = cx - w / 2, x1 = cx + w / 2, y0 = cy - h / 2, y1 = cy + h / 2;
  ops.push(M(x0, y0, z), M(x1, y0, z), M(x1, y1, z), M(x0, y1, z), M(x0, y0, z)); // perimeter
  const bot = y0 + gap, top = y1 - gap;
  ops.push(OFF, M(x0 + gap, bot, z), ON); // travel to the infill start
  const xs = [];
  for (let x = x0 + gap; x <= x1 - gap + 1e-9; x += gap) xs.push(x);
  let y = bot;
  for (let i = 0; i < xs.length; i++) {
    const ny = y === bot ? top : bot;
    ops.push(M(xs[i], ny, z)); // sweep across the panel
    y = ny;
    if (i < xs.length - 1) ops.push(M(xs[i + 1], y, z)); // step to the next rail
  }
  return ops;
}

// A conical vase: a helix whose radius shrinks as it rises — a genuinely 3D, non-planar surface.
function coneVase(r0 = 18, r1 = 4, height = 12, layerH = 0.4, perLayer = 32, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), ON];
  const turns = height / layerH;
  const n = Math.round(turns * perLayer);
  for (let i = 0; i <= n; i++) {
    const f = i / n, a = f * turns * TAU, r = r0 + (r1 - r0) * f;
    ops.push(M(cx + r * Math.cos(a), cy + r * Math.sin(a), z0 + f * height));
  }
  return ops;
}

// A rounded rectangle: four straight edges joined by four native G3 corner arcs (a complete closed loop
// mixing lines and arcs). Traversed counter-clockwise, so every fillet is a +90° CCW arc.
function roundedRect(w = 26, h = 18, r = 5, cx = 50, cy = 50, z = 0.4) {
  const ops = [G(0.6, 0.2), ON];
  const x0 = cx - w / 2, x1 = cx + w / 2, y0 = cy - h / 2, y1 = cy + h / 2;
  ops.push(M(x0 + r, y0, z), M(x1 - r, y0, z));        // bottom edge
  ops.push(ARC(x1 - r, y0 + r, x1, y0 + r, null, false)); // BR fillet
  ops.push(M(x1, y1 - r, z));                          // right edge
  ops.push(ARC(x1 - r, y1 - r, x1 - r, y1, null, false)); // TR fillet
  ops.push(M(x0 + r, y1, z));                          // top edge
  ops.push(ARC(x0 + r, y1 - r, x0, y1 - r, null, false)); // TL fillet
  ops.push(M(x0, y0 + r, z));                          // left edge
  ops.push(ARC(x0 + r, y0 + r, x0 + r, y0, null, false)); // BL fillet (closes the loop)
  return ops;
}

// ---- complex parametric samples ----

// A Hilbert space-filling curve (a recursive fractal) drawn as one continuous extruding stroke.
function hilbert(order = 4, size = 40, cx = 50, cy = 50, z = 0.2) {
  const n = 1 << order;
  const d2xy = (d) => {
    let t = d, x = 0, y = 0;
    for (let s = 1; s < n; s *= 2) {
      const rx = 1 & ((t / 2) | 0), ry = 1 & (t ^ rx);
      if (ry === 0) { if (rx === 1) { x = s - 1 - x; y = s - 1 - y; } const tmp = x; x = y; y = tmp; }
      x += s * rx; y += s * ry; t = (t / 4) | 0;
    }
    return [x, y];
  };
  const ops = [G(0.5, 0.2), TEMP(205), ON];
  for (let d = 0; d < n * n; d++) {
    const [gx, gy] = d2xy(d);
    ops.push(M(cx - size / 2 + (gx / (n - 1)) * size, cy - size / 2 + (gy / (n - 1)) * size, z));
  }
  return ops;
}

// A rhodonea (rose) curve r = a·cos(kθ): k or 2k petals.
function rose(k = 5, a = 18, cx = 50, cy = 50, z = 0.2, samples = 360) {
  const ops = [G(0.5, 0.2), TEMP(205), ON];
  const maxTh = k % 2 === 0 ? TAU : Math.PI;
  for (let i = 0; i <= samples; i++) {
    const th = (i / samples) * maxTh, r = a * Math.cos(k * th);
    ops.push(M(cx + r * Math.cos(th), cy + r * Math.sin(th), z));
  }
  return ops;
}

// A spirograph (hypotrochoid): a fixed circle R, a rolling circle r, a pen offset d.
function spirograph(R = 22, r = 7, d = 11, cx = 50, cy = 50, z = 0.2, samples = 720) {
  const ops = [G(0.5, 0.2), TEMP(205), ON];
  const turns = r / gcd(R, r);
  for (let i = 0; i <= samples; i++) {
    const th = (i / samples) * TAU * turns;
    const x = (R - r) * Math.cos(th) + d * Math.cos(((R - r) / r) * th);
    const y = (R - r) * Math.sin(th) - d * Math.sin(((R - r) / r) * th);
    ops.push(M(cx + x, cy + y, z));
  }
  return ops;
}

// A honeycomb: a grid of hexagons, each a closed loop, with travels between cells.
function honeycomb(cols = 5, rows = 4, s = 4.5, cx = 50, cy = 50, z = 0.2) {
  const ops = [G(0.5, 0.2), TEMP(205), ON];
  const hex = [];
  for (let i = 0; i < 6; i++) { const a = Math.PI / 6 + (i * TAU) / 6; hex.push([s * Math.cos(a), s * Math.sin(a)]); }
  const dx = s * Math.sqrt(3), dy = s * 1.5;
  const ox = cx - ((cols - 1) * dx) / 2, oy = cy - ((rows - 1) * dy) / 2;
  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const hxc = ox + col * dx + (row % 2 ? dx / 2 : 0), hyc = oy + row * dy;
      ops.push(OFF, M(hxc + hex[0][0], hyc + hex[0][1], z), ON);
      for (let i = 1; i <= 6; i++) ops.push(M(hxc + hex[i % 6][0], hyc + hex[i % 6][1], z));
    }
  }
  return ops;
}

// A corrugated wall: a sine-wave wall printed continuously over many layers (boustrophedon).
function corrugatedWall(length = 44, amp = 4, waves = 5, layers = 10, layerH = 0.3, samples = 72, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), TEMP(210), FAN(0.4), ON];
  const x0 = cx - length / 2;
  for (let L = 0; L < layers; L++) {
    const z = z0 + L * layerH;
    for (let i = 0; i <= samples; i++) {
      const f = L % 2 === 0 ? i / samples : 1 - i / samples;
      ops.push(M(x0 + f * length, cy + amp * Math.sin(f * TAU * waves), z));
    }
  }
  return ops;
}

// A twisted, fluted vase: an N-gon cross-section that twists and flutes as it rises.
function twistedVase(sides = 5, radius = 14, height = 16, layerH = 0.4, twist = TAU, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), TEMP(210), ON];
  const layers = Math.round(height / layerH), n = layers * sides;
  for (let i = 0; i <= n; i++) {
    const f = i / n, ang = (i / sides) * TAU + twist * f, r = radius * (0.88 + 0.12 * Math.cos(sides * ang));
    ops.push(M(cx + r * Math.cos(ang), cy + r * Math.sin(ang), z0 + f * height));
  }
  return ops;
}

// A star tower: a star perimeter stacked over layers (with a slight per-layer twist), travels between.
function starTower(points = 5, outer = 16, inner = 7, layers = 9, layerH = 0.4, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), TEMP(210), ON];
  const m = points * 2;
  for (let L = 0; L < layers; L++) {
    const z = z0 + L * layerH, rot = L * 0.12, v = [];
    for (let i = 0; i < m; i++) {
      const r = i % 2 === 0 ? outer : inner, a = (i / m) * TAU - Math.PI / 2 + rot;
      v.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
    }
    ops.push(OFF, M(v[0][0], v[0][1], z), ON);
    for (let i = 1; i <= m; i++) ops.push(M(v[i % m][0], v[i % m][1], z));
  }
  return ops;
}

// A (p,q) torus knot — a genuinely 3D, non-planar closed curve.
function torusKnot(p = 3, q = 2, R = 15, r = 5, samples = 480, cx = 50, cy = 50, zc = 10) {
  const ops = [G(0.6, 0.2), TEMP(210), ON];
  for (let i = 0; i <= samples; i++) {
    const t = (i / samples) * TAU, rad = R + r * Math.cos(q * t);
    ops.push(M(cx + rad * Math.cos(p * t), cy + rad * Math.sin(p * t), zc + r * Math.sin(q * t)));
  }
  return ops;
}

// A 3D Lissajous ribbon: x and y are out-of-phase sinusoids while z rises.
function lissajous(a = 3, b = 2, delta = Math.PI / 2, A = 18, B = 18, samples = 500, cx = 50, cy = 50, z0 = 0.2, zRange = 9) {
  const ops = [G(0.5, 0.2), TEMP(205), ON];
  for (let i = 0; i <= samples; i++) {
    const t = (i / samples) * TAU;
    ops.push(M(cx + A * Math.sin(a * t + delta), cy + B * Math.sin(b * t), z0 + (i / samples) * zRange));
  }
  return ops;
}

// A lattice cube: cross-hatched layers whose line direction alternates each layer (a 3D grid).
function lattice(size = 28, gap = 4, layers = 8, layerH = 0.3, cx = 50, cy = 50, z0 = 0.2) {
  const ops = [G(0.6, 0.2), TEMP(210), ON];
  const x0 = cx - size / 2, y0 = cy - size / 2, x1 = cx + size / 2, y1 = cy + size / 2;
  for (let L = 0; L < layers; L++) {
    const z = z0 + L * layerH, lines = [];
    if (L % 2 === 0) for (let y = y0; y <= y1 + 1e-9; y += gap) lines.push([[x0, y], [x1, y]]);
    else for (let x = x0; x <= x1 + 1e-9; x += gap) lines.push([[x, y0], [x, y1]]);
    let flip = false;
    for (const [p, q] of lines) {
      const a = flip ? q : p, b = flip ? p : q;
      ops.push(OFF, M(a[0], a[1], z), ON, M(b[0], b[1], z));
      flip = !flip;
    }
  }
  return ops;
}

const range = (id, label, defaultValue, min, max, step, unit = '1', title = '') => ({
  type: 'range',
  id,
  label,
  defaultValue,
  min,
  max,
  step,
  unit,
  title,
  integer: Number(step) >= 1 && Number.isInteger(defaultValue) && Number.isInteger(min) && Number.isInteger(max),
});
const centerParams = () => [
  range('cx', 'cx', 50, 0, 100, 0.5, 'mm', 'Center X.'),
  range('cy', 'cy', 50, 0, 100, 0.5, 'mm', 'Center Y.'),
];
const zParam = (id = 'z', value = 0.2) => range(id, id, value, 0.05, 20, 0.01, 'mm', 'Z height.');
const sampleParam = (value = 360, max = 1200) => range('samples', 'samples', value, 12, max, 1, '1', 'Generated curve resolution.');
const starLatticeParams = (alphaDeg) => [
  range('alphaDeg', 'alpha', alphaDeg, 0, 150, 1, 'deg'),
  range('segLength', 'strut', 4.33, 1, 50, 0.01, 'mm'),
  range('cols', 'columns', 10, 1, 15, 1, '1'),
  range('rows', 'rows', 3, 1, 12, 1, '1'),
  range('layers', 'layers', 2, 1, 24, 1, '1'),
  range('layerHeight', 'layer', 0.2, 0.05, 2, 0.001, 'mm'),
];
const tpmsParams = () => [
  range('cellSize', 'cell', 22, 4, 80, 0.5, 'mm'),
  range('samplesPerCell', 'samples', 16, 4, 64, 1, '1/cell'),
  range('cellsX', 'cells X', 1, 1, 8, 1, '1'),
  range('cellsY', 'cells Y', 1, 1, 8, 1, '1'),
  range('cellsZ', 'cells Z', 1, 1, 8, 1, '1'),
  range('layerHeight', 'layer', 0.28, 0.08, 1.4, 0.01, 'mm'),
  range('isoLevel', 'iso', 0, -4, 4, 0.05, '1'),
];

function defaultParams(params = []) {
  return Object.fromEntries(params.map((param) => [param.id, param.defaultValue]));
}

function materializeDesign(def) {
  const params = def.params || [];
  const defaults = defaultParams(params);
  // `ops` is a lazy thunk, not an eagerly-computed array: engine-backed defs (e.g. TPMS) call into
  // the wasm module via `build`, which isn't initialized yet at module-evaluation time. Consumers
  // already know how to call a function-valued `ops` (see fillCardThumbnail in index.html and the
  // SOURCE_DEFS entries it was modeled on), so this stays a drop-in replacement.
  const ops = () => (typeof def.build === 'function' ? def.build(defaults) : (def.ops || []));
  return { ...def, params, defaults, ops };
}

function researchLattice(family, alphaDeg) {
  return ({ cols, rows, segLength, layers, layerHeight, alphaDeg: alpha }) => starPolygonLatticeOps({
    family,
    alphaDeg: alpha,
    cols,
    rows,
    segLength,
    layers,
    layerHeight,
  });
}

function tpmsGallery(surface) {
  return (params) => tpmsOps({ surface, ...params });
}

// Each entry carries build metadata so the page can generate controls directly from this table.
const DESIGN_DEFS = {
  square: {
    label: 'Square (line moves)', group: 'Basics', tags: ['line', 'perimeter'],
    params: [range('side', 'side', 10, 1, 80, 0.5, 'mm'), zParam()],
    build: ({ side, z }) => square(side, z),
  },
  star: {
    label: 'Star (continuous stroke)', group: 'Basics', tags: ['line', 'parametric'],
    params: [range('points', 'points', 5, 3, 16, 1, '1'), range('outer', 'outer', 20, 2, 45, 0.5, 'mm'), range('inner', 'inner', 8, 1, 35, 0.5, 'mm'), ...centerParams(), zParam()],
    build: ({ points, outer, inner, cx, cy, z }) => star(points, outer, inner, cx, cy, z),
  },
  arcs_mix: {
    label: 'Arcs (native G2/G3)', group: 'Curves', tags: ['arc'],
    params: [range('radius', 'radius', 10, 2, 40, 0.5, 'mm'), range('gap', 'gap', 10, 2, 50, 0.5, 'mm'), range('speed', 'speed', 1800, 60, 12000, 10, 'mm/min'), zParam('z', 0.4)],
    build: ({ radius, gap, speed, z }) => arcsMix(radius, gap, speed, z),
  },
  rounded_rect: {
    label: 'Rounded rect (lines + 4 arcs)', group: 'Curves', tags: ['arc', 'line'],
    params: [range('w', 'width', 26, 4, 80, 0.5, 'mm'), range('h', 'height', 18, 4, 80, 0.5, 'mm'), range('r', 'radius', 5, 0.5, 30, 0.5, 'mm'), ...centerParams(), zParam('z', 0.4)],
    build: ({ w, h, r, cx, cy, z }) => roundedRect(w, h, Math.min(r, w / 2, h / 2), cx, cy, z),
  },
  spline_s_curve: {
    label: 'S-curve (native spline)', group: 'Curves', tags: ['spline', 'curve'],
    params: [range('length', 'length', 64, 8, 100, 1, 'mm'), range('amp', 'amp', 16, 1, 40, 0.5, 'mm'), range('points', 'points', 6, 3, 24, 1, '1'), ...centerParams(), zParam()],
    build: ({ length, amp, points, cx, cy, z }) => splineSCurve(length, amp, points, cx, cy, z),
  },
  infill_panel: {
    label: 'Infill panel (perimeter + zig-zag)', group: 'Infill & multi-layer', tags: ['infill', 'travel'],
    params: [range('w', 'width', 26, 4, 80, 0.5, 'mm'), range('h', 'height', 18, 4, 80, 0.5, 'mm'), range('gap', 'gap', 2, 0.5, 12, 0.1, 'mm'), ...centerParams(), zParam()],
    build: ({ w, h, gap, cx, cy, z }) => infillPanel(w, h, gap, cx, cy, z),
  },
  layered_tower: {
    label: 'Layered tower (10 layers + travels)', group: 'Infill & multi-layer', tags: ['multi-layer', 'travel'],
    params: [range('side', 'side', 20, 2, 80, 0.5, 'mm'), range('layers', 'layers', 10, 1, 80, 1, '1'), range('layerH', 'layer', 0.3, 0.05, 2, 0.01, 'mm'), ...centerParams(), zParam('z0', 0.2)],
    build: ({ side, layers, layerH, cx, cy, z0 }) => layeredTower(side, layers, layerH, cx, cy, z0),
  },
  retraction_tower: {
    label: 'Retraction tower (retract / travel / unretract)', group: 'Infill & multi-layer', tags: ['multi-layer', 'travel', 'retract'],
    params: [range('side', 'side', 16, 2, 80, 0.5, 'mm'), range('layers', 'layers', 6, 1, 60, 1, '1'), range('layerH', 'layer', 0.4, 0.05, 2, 0.01, 'mm'), range('retractDist', 'retract', 1.2, 0, 8, 0.1, 'mm'), range('retractSpeed', 'retract v', 2400, 60, 9000, 60, 'mm/min'), ...centerParams(), zParam('z0', 0.2)],
    build: ({ side, layers, layerH, retractDist, retractSpeed, cx, cy, z0 }) => retractionTower(side, layers, layerH, retractDist, retractSpeed, cx, cy, z0),
  },
  spiral_vase: {
    label: 'Spiral vase (~120-seg helix)', group: 'Vases & non-planar', tags: ['non-planar', '3D'],
    params: [range('radius', 'radius', 15, 2, 45, 0.5, 'mm'), range('height', 'height', 1.5, 0.2, 80, 0.1, 'mm'), range('layerH', 'layer', 0.3, 0.05, 2, 0.01, 'mm'), range('perLayer', 'samples/layer', 24, 4, 96, 1, '1'), ...centerParams()],
    build: ({ radius, height, layerH, perLayer, cx, cy }) => spiralVase(radius, height, layerH, perLayer, cx, cy),
  },
  cone_vase: {
    label: 'Cone vase (non-planar helix)', group: 'Vases & non-planar', tags: ['non-planar', '3D'],
    params: [range('r0', 'base r', 18, 2, 45, 0.5, 'mm'), range('r1', 'top r', 4, 1, 45, 0.5, 'mm'), range('height', 'height', 12, 0.5, 100, 0.5, 'mm'), range('layerH', 'layer', 0.4, 0.05, 2, 0.01, 'mm'), range('perLayer', 'samples/layer', 32, 4, 120, 1, '1'), ...centerParams(), zParam('z0', 0.2)],
    build: ({ r0, r1, height, layerH, perLayer, cx, cy, z0 }) => coneVase(r0, r1, height, layerH, perLayer, cx, cy, z0),
  },
  collinear_comb: {
    label: 'Comb (collinear runs -> optimize)', group: 'Infill & multi-layer', tags: ['travel', 'optimize'],
    params: [range('rungs', 'rungs', 6, 1, 24, 1, '1'), range('len', 'length', 30, 2, 90, 0.5, 'mm'), range('pitch', 'pitch', 4, 0.5, 20, 0.5, 'mm'), range('subdiv', 'subdiv', 5, 1, 24, 1, '1'), range('x0', 'x0', 10, 0, 100, 0.5, 'mm'), range('y0', 'y0', 10, 0, 100, 0.5, 'mm'), zParam()],
    build: ({ rungs, len, pitch, subdiv, x0, y0, z }) => collinearComb(rungs, len, pitch, subdiv, x0, y0, z),
  },
  hilbert: {
    label: 'Hilbert curve (space-filling fractal)', group: 'Curves', tags: ['fractal', 'parametric'],
    params: [range('order', 'order', 4, 1, 7, 1, '1'), range('size', 'size', 40, 4, 90, 1, 'mm'), ...centerParams(), zParam()],
    build: ({ order, size, cx, cy, z }) => hilbert(order, size, cx, cy, z),
  },
  rose: {
    label: 'Rose curve (rhodonea)', group: 'Curves', tags: ['parametric'],
    params: [range('k', 'k', 5, 1, 16, 1, '1'), range('a', 'radius', 18, 2, 45, 0.5, 'mm'), ...centerParams(), zParam(), sampleParam(360, 1600)],
    build: ({ k, a, cx, cy, z, samples }) => rose(k, a, cx, cy, z, samples),
  },
  spirograph: {
    label: 'Spirograph (hypotrochoid)', group: 'Curves', tags: ['parametric'],
    params: [range('R', 'outer R', 22, 3, 50, 1, 'mm'), range('r', 'inner r', 7, 1, 30, 1, 'mm'), range('d', 'pen d', 11, 1, 40, 0.5, 'mm'), ...centerParams(), zParam(), sampleParam(720, 2400)],
    build: ({ R, r, d, cx, cy, z, samples }) => spirograph(R, r, d, cx, cy, z, samples),
  },
  honeycomb: {
    label: 'Honeycomb (hex tiling + travels)', group: 'Infill & multi-layer', tags: ['infill', 'travel'],
    params: [range('cols', 'columns', 5, 1, 16, 1, '1'), range('rows', 'rows', 4, 1, 16, 1, '1'), range('s', 'cell', 4.5, 1, 15, 0.25, 'mm'), ...centerParams(), zParam()],
    build: ({ cols, rows, s, cx, cy, z }) => honeycomb(cols, rows, s, cx, cy, z),
  },
  corrugated_wall: {
    label: 'Corrugated wall (10-layer sine)', group: 'Infill & multi-layer', tags: ['multi-layer', 'parametric'],
    params: [range('length', 'length', 44, 4, 100, 1, 'mm'), range('amp', 'amp', 4, 0.5, 20, 0.5, 'mm'), range('waves', 'waves', 5, 1, 20, 1, '1'), range('layers', 'layers', 10, 1, 80, 1, '1'), range('layerH', 'layer', 0.3, 0.05, 2, 0.01, 'mm'), sampleParam(72, 500), ...centerParams(), zParam('z0', 0.2)],
    build: ({ length, amp, waves, layers, layerH, samples, cx, cy, z0 }) => corrugatedWall(length, amp, waves, layers, layerH, samples, cx, cy, z0),
  },
  twisted_vase: {
    label: 'Twisted vase (fluted, non-planar)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'],
    params: [range('sides', 'sides', 5, 3, 16, 1, '1'), range('radius', 'radius', 14, 2, 45, 0.5, 'mm'), range('height', 'height', 16, 0.5, 100, 0.5, 'mm'), range('layerH', 'layer', 0.4, 0.05, 2, 0.01, 'mm'), range('twistDeg', 'twist', 360, -1080, 1080, 15, 'deg'), ...centerParams(), zParam('z0', 0.2)],
    build: ({ sides, radius, height, layerH, twistDeg, cx, cy, z0 }) => twistedVase(sides, radius, height, layerH, (twistDeg / 360) * TAU, cx, cy, z0),
  },
  star_tower: {
    label: 'Star tower (stacked + twist)', group: 'Vases & non-planar', tags: ['multi-layer', 'travel', '3D'],
    params: [range('points', 'points', 5, 3, 16, 1, '1'), range('outer', 'outer', 16, 2, 45, 0.5, 'mm'), range('inner', 'inner', 7, 1, 35, 0.5, 'mm'), range('layers', 'layers', 9, 1, 80, 1, '1'), range('layerH', 'layer', 0.4, 0.05, 2, 0.01, 'mm'), ...centerParams(), zParam('z0', 0.2)],
    build: ({ points, outer, inner, layers, layerH, cx, cy, z0 }) => starTower(points, outer, inner, layers, layerH, cx, cy, z0),
  },
  torus_knot: {
    label: 'Torus knot (3D, non-planar)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'],
    params: [range('p', 'p', 3, 1, 12, 1, '1'), range('q', 'q', 2, 1, 12, 1, '1'), range('R', 'major R', 15, 2, 45, 0.5, 'mm'), range('r', 'minor r', 5, 0.5, 20, 0.5, 'mm'), sampleParam(480, 2400), ...centerParams(), range('zc', 'z center', 10, 0, 80, 0.5, 'mm')],
    build: ({ p, q, R, r, samples, cx, cy, zc }) => torusKnot(p, q, R, r, samples, cx, cy, zc),
  },
  lissajous: {
    label: 'Lissajous ribbon (3D)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'],
    params: [range('a', 'a', 3, 1, 12, 1, '1'), range('b', 'b', 2, 1, 12, 1, '1'), range('deltaDeg', 'phase', 90, -360, 360, 5, 'deg'), range('A', 'amp X', 18, 1, 45, 0.5, 'mm'), range('B', 'amp Y', 18, 1, 45, 0.5, 'mm'), sampleParam(500, 2400), ...centerParams(), zParam('z0', 0.2), range('zRange', 'z span', 9, 0, 80, 0.5, 'mm')],
    build: ({ a, b, deltaDeg, A, B, samples, cx, cy, z0, zRange }) => lissajous(a, b, (deltaDeg / 360) * TAU, A, B, samples, cx, cy, z0, zRange),
  },
  lattice: {
    label: 'Lattice cube (cross-hatch layers)', group: 'Infill & multi-layer', tags: ['infill', 'multi-layer', '3D'],
    params: [range('size', 'size', 28, 4, 90, 0.5, 'mm'), range('gap', 'gap', 4, 0.5, 20, 0.5, 'mm'), range('layers', 'layers', 8, 1, 80, 1, '1'), range('layerH', 'layer', 0.3, 0.05, 2, 0.01, 'mm'), ...centerParams(), zParam('z0', 0.2)],
    build: ({ size, gap, layers, layerH, cx, cy, z0 }) => lattice(size, gap, layers, layerH, cx, cy, z0),
  },
  star_lattice_m1: { label: 'M1 star-polygon lattice (alpha 30 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M1', 'parametric'], params: starLatticeParams(30), build: researchLattice('M1', 30) },
  star_lattice_m2: { label: 'M2 star-polygon lattice (alpha 60 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M2', 'parametric'], params: starLatticeParams(60), build: researchLattice('M2', 60) },
  star_lattice_m3: { label: 'M3 star-polygon lattice (alpha 30 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M3', 'parametric'], params: starLatticeParams(30), build: researchLattice('M3', 30) },
  star_lattice_m4: { label: 'M4 star-polygon lattice (alpha 45 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M4', 'parametric'], params: starLatticeParams(45), build: researchLattice('M4', 45) },
  tpms_gyroid: { label: 'TPMS gyroid contours', group: 'TPMS', tags: ['TPMS', 'gyroid', 'implicit'], params: tpmsParams(), build: tpmsGallery('gyroid') },
  tpms_schwarz_p: { label: 'TPMS Schwarz P contours', group: 'TPMS', tags: ['TPMS', 'Schwarz P', 'implicit'], params: tpmsParams(), build: tpmsGallery('schwarz-p') },
  tpms_schwarz_d: { label: 'TPMS Schwarz D contours', group: 'TPMS', tags: ['TPMS', 'Schwarz D', 'implicit'], params: tpmsParams(), build: tpmsGallery('schwarz-d') },
  tpms_iwp: { label: 'TPMS I-WP contours', group: 'TPMS', tags: ['TPMS', 'I-WP', 'implicit'], params: tpmsParams(), build: tpmsGallery('iwp') },
  tpms_neovius: { label: 'TPMS Neovius contours', group: 'TPMS', tags: ['TPMS', 'Neovius', 'implicit'], params: tpmsParams(), build: tpmsGallery('neovius') },
  tpms_fks: { label: 'TPMS Fischer-Koch S contours', group: 'TPMS', tags: ['TPMS', 'FKS', 'implicit'], params: tpmsParams(), build: tpmsGallery('fischer-koch-s') },
  tpms_frd: { label: 'TPMS F-RD contours', group: 'TPMS', tags: ['TPMS', 'F-RD', 'implicit'], params: tpmsParams(), build: tpmsGallery('frd') },
};

const DESIGNS = Object.fromEntries(
  Object.entries(DESIGN_DEFS).map(([key, def]) => [key, materializeDesign(def)])
);

const RESOLVE_PARAMS = { print_speed: 1000, travel_speed: 8000, dia: 1.75 };

export { DESIGNS, FULLCONTROL_DESIGNS, RESOLVE_PARAMS, TAU };
