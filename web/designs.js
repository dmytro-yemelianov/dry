// Clean-room demo designs, authored as Dry L1 ops (the design layer the engine resolves).
// Each is an array of ops: geometry / extruder / speed / move / arc. The same op vocabulary the
// Python SDK and the conformance oracle use. No FullControl code — just the public op shape.
const TAU = Math.PI * 2;
const G = (w, h) => ({ op: 'geometry', width: w, height: h });
const ON = { op: 'extruder', on: true };
const OFF = { op: 'extruder', on: false };
const SPEED = (v) => ({ op: 'speed', print: v });
const M = (x, y, z) => ({ op: 'move', x, y, z });
const ARC = (cx, cy, x, y, z, clockwise) => ({ op: 'arc', cx, cy, x, y, z, clockwise });

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

const DESIGNS = {
  square: { label: 'Square (line moves)', ops: square() },
  star: { label: 'Star (continuous stroke)', ops: star() },
  arcs_mix: { label: 'Arcs (native G2/G3)', ops: arcsMix() },
  rounded_rect: { label: 'Rounded rect (lines + 4 arcs)', ops: roundedRect() },
  infill_panel: { label: 'Infill panel (perimeter + zig-zag)', ops: infillPanel() },
  layered_tower: { label: 'Layered tower (10 layers + travels)', ops: layeredTower() },
  spiral_vase: { label: 'Spiral vase (~120-seg helix)', ops: spiralVase() },
  cone_vase: { label: 'Cone vase (non-planar helix)', ops: coneVase() },
  collinear_comb: { label: 'Comb (collinear runs → optimize)', ops: collinearComb() },
};

const RESOLVE_PARAMS = { print_speed: 1000, travel_speed: 8000, dia: 1.75 };

export { DESIGNS, RESOLVE_PARAMS, TAU };
