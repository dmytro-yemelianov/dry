// Clean-room demo designs, authored as Dry L1 ops (the design layer the engine resolves).
// Each is an array of ops: geometry / extruder / speed / move / arc. The same op vocabulary the
// Python SDK and the conformance oracle use. No FullControl code — just the public op shape.
import { starPolygonLatticeOps } from './lattice-research.js';
import { tpmsOps } from './tpms.js';

const TAU = Math.PI * 2;
const G = (w, h) => ({ op: 'geometry', width: w, height: h });
const ON = { op: 'extruder', on: true };
const OFF = { op: 'extruder', on: false };
const SPEED = (v) => ({ op: 'speed', print: v });
const M = (x, y, z) => ({ op: 'move', x, y, z });
const ARC = (cx, cy, x, y, z, clockwise) => ({ op: 'arc', cx, cy, x, y, z, clockwise });
const TEMP = (c) => ({ op: 'temperature', nozzle: c });
const FAN = (v) => ({ op: 'fan', speed: v });
const gcd = (a, b) => { a = Math.abs(a); b = Math.abs(b); while (b) { [a, b] = [b, a % b]; } return a; };

function square() {
  return [G(0.6, 0.2), ON, M(0, 0, 0.2), M(10, 0, 0.2), M(10, 10, 0.2), M(0, 10, 0.2), M(0, 0, 0.2)];
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
function arcsMix() {
  return [
    G(0.6, 0.2), ON, M(20, 5, 0.4), SPEED(1800),
    ARC(10, 5, 0, 5, null, true),
    M(0, 15, 0.4),
    ARC(10, 15, 20, 15, null, true),
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

// Each entry carries a `group` (for grouped pickers) and `tags` (chips / filtering). The `ops`
// are unchanged — the gallery designs are byte-identical to before; only metadata was added.
const DESIGNS = {
  square: { label: 'Square (line moves)', group: 'Basics', tags: ['line', 'perimeter'], ops: square() },
  star: { label: 'Star (continuous stroke)', group: 'Basics', tags: ['line', 'parametric'], ops: star() },
  arcs_mix: { label: 'Arcs (native G2/G3)', group: 'Curves', tags: ['arc'], ops: arcsMix() },
  rounded_rect: { label: 'Rounded rect (lines + 4 arcs)', group: 'Curves', tags: ['arc', 'line'], ops: roundedRect() },
  infill_panel: { label: 'Infill panel (perimeter + zig-zag)', group: 'Infill & multi-layer', tags: ['infill', 'travel'], ops: infillPanel() },
  layered_tower: { label: 'Layered tower (10 layers + travels)', group: 'Infill & multi-layer', tags: ['multi-layer', 'travel'], ops: layeredTower() },
  spiral_vase: { label: 'Spiral vase (~120-seg helix)', group: 'Vases & non-planar', tags: ['non-planar', '3D'], ops: spiralVase() },
  cone_vase: { label: 'Cone vase (non-planar helix)', group: 'Vases & non-planar', tags: ['non-planar', '3D'], ops: coneVase() },
  collinear_comb: { label: 'Comb (collinear runs → optimize)', group: 'Infill & multi-layer', tags: ['travel', 'optimize'], ops: collinearComb() },
  // complex parametric samples
  hilbert: { label: 'Hilbert curve (space-filling fractal)', group: 'Curves', tags: ['fractal', 'parametric'], ops: hilbert() },
  rose: { label: 'Rose curve (rhodonea)', group: 'Curves', tags: ['parametric'], ops: rose() },
  spirograph: { label: 'Spirograph (hypotrochoid)', group: 'Curves', tags: ['parametric'], ops: spirograph() },
  honeycomb: { label: 'Honeycomb (hex tiling + travels)', group: 'Infill & multi-layer', tags: ['infill', 'travel'], ops: honeycomb() },
  corrugated_wall: { label: 'Corrugated wall (10-layer sine)', group: 'Infill & multi-layer', tags: ['multi-layer', 'parametric'], ops: corrugatedWall() },
  twisted_vase: { label: 'Twisted vase (fluted, non-planar)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'], ops: twistedVase() },
  star_tower: { label: 'Star tower (stacked + twist)', group: 'Vases & non-planar', tags: ['multi-layer', 'travel', '3D'], ops: starTower() },
  torus_knot: { label: 'Torus knot (3D, non-planar)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'], ops: torusKnot() },
  lissajous: { label: 'Lissajous ribbon (3D)', group: 'Vases & non-planar', tags: ['non-planar', '3D', 'parametric'], ops: lissajous() },
  lattice: { label: 'Lattice cube (cross-hatch layers)', group: 'Infill & multi-layer', tags: ['infill', 'multi-layer', '3D'], ops: lattice() },
  star_lattice_m1: { label: 'M1 star-polygon lattice (alpha 30 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M1', 'parametric'], ops: starPolygonLatticeOps({ family: 'M1', alphaDeg: 30, cols: 5, rows: 3, unit: 13 }) },
  star_lattice_m2: { label: 'M2 star-polygon lattice (alpha 60 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M2', 'parametric'], ops: starPolygonLatticeOps({ family: 'M2', alphaDeg: 60, cols: 5, rows: 3, unit: 13 }) },
  star_lattice_m3: { label: 'M3 star-polygon lattice (alpha 30 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M3', 'parametric'], ops: starPolygonLatticeOps({ family: 'M3', alphaDeg: 30, cols: 5, rows: 3, unit: 13 }) },
  star_lattice_m4: { label: 'M4 star-polygon lattice (alpha 45 deg)', group: 'Research lattices', tags: ['research', 'lattice', 'M4', 'parametric'], ops: starPolygonLatticeOps({ family: 'M4', alphaDeg: 45, cols: 5, rows: 3, unit: 13 }) },
  tpms_gyroid: { label: 'TPMS gyroid contours', group: 'TPMS', tags: ['TPMS', 'gyroid', 'implicit'], ops: tpmsOps({ surface: 'gyroid', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
  tpms_schwarz_p: { label: 'TPMS Schwarz P contours', group: 'TPMS', tags: ['TPMS', 'Schwarz P', 'implicit'], ops: tpmsOps({ surface: 'schwarz-p', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
  tpms_schwarz_d: { label: 'TPMS Schwarz D contours', group: 'TPMS', tags: ['TPMS', 'Schwarz D', 'implicit'], ops: tpmsOps({ surface: 'schwarz-d', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
  tpms_iwp: { label: 'TPMS I-WP contours', group: 'TPMS', tags: ['TPMS', 'I-WP', 'implicit'], ops: tpmsOps({ surface: 'iwp', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
  tpms_neovius: { label: 'TPMS Neovius contours', group: 'TPMS', tags: ['TPMS', 'Neovius', 'implicit'], ops: tpmsOps({ surface: 'neovius', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
  tpms_fks: { label: 'TPMS Fischer-Koch S contours', group: 'TPMS', tags: ['TPMS', 'FKS', 'implicit'], ops: tpmsOps({ surface: 'fischer-koch-s', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
  tpms_frd: { label: 'TPMS F-RD contours', group: 'TPMS', tags: ['TPMS', 'F-RD', 'implicit'], ops: tpmsOps({ surface: 'frd', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 22, samplesPerCell: 16, layerHeight: 1.4 }) },
};

const RESOLVE_PARAMS = { print_speed: 1000, travel_speed: 8000, dia: 1.75 };

export { DESIGNS, RESOLVE_PARAMS, TAU };
