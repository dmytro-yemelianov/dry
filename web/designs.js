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

const DESIGNS = {
  square: { label: 'Square (line moves)', ops: square() },
  star: { label: 'Star (continuous stroke)', ops: star() },
  arcs_mix: { label: 'Arcs (native G2/G3)', ops: arcsMix() },
  spiral_vase: { label: 'Spiral vase (~120-seg helix)', ops: spiralVase() },
  collinear_comb: { label: 'Comb (collinear runs → optimize)', ops: collinearComb() },
};

const RESOLVE_PARAMS = { print_speed: 1000, travel_speed: 8000, dia: 1.75 };

export { DESIGNS, RESOLVE_PARAMS, TAU };
