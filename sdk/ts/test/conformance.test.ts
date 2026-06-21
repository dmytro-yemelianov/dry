// The TS SDK authors a design and the engine reproduces the FullControl oracle (clean-room proof):
// the same Rust engine, via wasm, emits g-code byte-identical to `conformance/gcode/*.json`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import {
  Design,
  normalizeStarPolygonAlpha,
  type Op,
  resolveGcode,
  RESOLVE_PARAMS,
  starPolygonDentRadiusRatio,
  STAR_POLYGON_FAMILIES,
  starPolygonLattice,
  starPolygonLatticeOps,
  tpms,
  tpmsField,
  tpmsOps,
  TPMS_SURFACES,
} from '../src/index';

// dist/test/conformance.test.js -> repo root is four levels up (dist/test -> ts -> sdk -> repo).
const CONF = path.resolve(__dirname, '../../../../conformance');

function fixture(kind: string, name: string): any {
  return JSON.parse(fs.readFileSync(path.join(CONF, kind, `${name}.json`), 'utf8'));
}

// Every oracle design, resolved through the SDK's low-level engine call, must match byte-for-byte.
test('every oracle design reproduces byte-for-byte (incl spiral_vase)', () => {
  let checked = 0;
  for (const file of fs.readdirSync(path.join(CONF, 'gcode'))) {
    if (!file.endsWith('.json')) continue;
    const fx = fixture('gcode', file.replace(/\.json$/, ''));
    const got = resolveGcode(fx.l1.ops, fx.resolve_params, fx.params.relative_e);
    assert.deepEqual(got, fx.expected, `[${fx.design}] g-code mismatch`);
    checked++;
  }
  assert.ok(checked >= 1, 'no fixtures');
});

// The fluent builder authors the square design and emits the oracle's g-code.
test('the fluent builder reproduces the square oracle', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2);
  assert.deepEqual(d.gcode(), fixture('gcode', 'square').expected);
});

// A native G3 arc authored fluently.
test('the fluent builder reproduces a native arc (G3)', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(10, 0, 0.2)
    .arc({ cx: 0, cy: 0, x: 0, y: 10, clockwise: false })
    .point(0, 20, 0.2);
  assert.deepEqual(d.gcode(), fixture('gcode', 'arc_ccw').expected);
});

// simulate() metrics parity with the oracle.
test('simulate() matches the oracle metrics', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2);
  const m = d.simulate();
  const want = fixture('simulate', 'square').expected;
  assert.equal(m.segment_count, want.segment_count);
  assert.ok(Math.abs(m.total_time_s - want.total_time_s) < 1e-9);
  assert.ok(Math.abs(m.extruded_volume - want.extruded_volume) < 1e-9);
});

// ir() returns the L2 Dry IR.
test('ir() returns resolved segments', () => {
  const d = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(10, 0, 0.2);
  const ir = d.ir();
  assert.equal(ir.version, 0);
  assert.equal(ir.segments.length, 2);
  assert.deepEqual(ir.segments[1].end, [10, 0, 0.2]);
});

test('optimizedIr() uses the shared optimizer pipeline', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(5, 0, 0.2)
    .point(10, 0, 0.2)
    .point(15, 0, 0.2);
  const raw = d.ir();
  const opt = d.optimizedIr();
  assert.ok(opt.segments.length < raw.segments.length);
  assert.equal(opt.segments[0].kind, 'line');
});

// RESOLVE_PARAMS is the documented generic-printer default.
test('generic params are exposed', () => {
  assert.deepEqual(RESOLVE_PARAMS, { print_speed: 1000, travel_speed: 8000, dia: 1.75 });
});

// Process channels author onto the resolved IR, and dwell emits a G4.
test('channels and dwell author through the builder', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .temperature(210)
    .fan(0.5)
    .tool(1)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .dwell(2.5);
  const ir = d.ir();
  assert.equal(ir.segments[1].temperature, 210);
  assert.equal(ir.segments[1].fan, 0.5);
  assert.equal(ir.segments[1].tool, 1);
  assert.ok(d.gcode().some((l) => l === 'G4 S2.5'), 'expected a G4 dwell line');
});

// Toolframe orientation rides each segment (5-axis / non-planar as a first-class IR property).
test('toolframe orientation authors onto segments', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .orient(0.6, 0, 0.8)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2);
  assert.deepEqual(d.ir().segments[1].orientation, [0.6, 0, 0.8]);
});

// A Catmull-Rom spline keeps curves intact in the L2 toolpath and lowers in emit.
test("spline authors through the builder", () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .spline([
      [10, 0, 0.2],
      [10, 10, 0.2],
      [0, 10, 0.2],
    ]);
  const ir = d.ir();
  // Expect 2 segments: 1 positioning line + 1 first-class spline segment.
  assert.equal(ir.segments.length, 2);
  assert.equal(ir.segments[0].kind, "line");
  assert.equal(ir.segments[1].kind, "spline");
  assert.deepEqual(ir.segments[1].end, [0, 10, 0.2]);
  assert.deepEqual(ir.segments[1].control_points, [
    [10, 0, 0.2],
    [10, 10, 0.2],
    [0, 10, 0.2],
  ]);

  // Verify that emitting g-code resolves the spline into 48 sub-moves.
  const gcode = d.gcode();
  assert.equal(gcode.length, 49);
});

// A flow multiplier scales the deposited volume.
test('flow multiplier scales deposited volume', () => {
  const base = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(10, 0, 0.2);
  const scaled = new Design()
    .geometry(0.6, 0.2)
    .flow(0.8)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2);
  const b = base.ir().segments[1].volume;
  const s = scaled.ir().segments[1].volume;
  assert.ok(Math.abs(s - b * 0.8) < 1e-12, `${s} vs ${b * 0.8}`);
});

// verify() safety contracts.
test('verify() finds contract violations', () => {
  const d = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(10, 0, 0.2);
  const report = d.verify('generic', 15.0, 0, '0,100,0,100,0,50');
  assert.deepEqual(report.findings, []);

  const dBadBounds = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(150, 0, 0.2);
  const reportBounds = dBadBounds.verify('generic', 0, 0, '0,100,0,100,0,50');
  assert.ok(reportBounds.findings.length > 0);
  assert.equal(reportBounds.findings[0].rule, 'bounds');

  const dBadZ = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.5).point(10, 0, 0.2);
  const reportZ = dBadZ.verify('generic', 0, 0, '', true);
  assert.ok(reportZ.findings.length > 0);
  assert.equal(reportZ.findings[0].rule, 'monotonic-z');
});

test('star-polygon family specs expose the paper alpha limits', () => {
  assert.equal(STAR_POLYGON_FAMILIES.M1.topology, '4 . 4*alpha . 4**alpha');
  assert.equal(STAR_POLYGON_FAMILIES.M1.alphaSplDeg, 90);
  assert.equal(STAR_POLYGON_FAMILIES.M1.alphaUlDeg, 135);
  assert.equal(STAR_POLYGON_FAMILIES.M2.alphaSplDeg, 120);
  assert.equal(STAR_POLYGON_FAMILIES.M2.alphaUlDeg, 150);
  assert.equal(STAR_POLYGON_FAMILIES.M3.alphaSplDeg, 60);
  assert.equal(STAR_POLYGON_FAMILIES.M3.alphaUlDeg, 120);
  assert.equal(STAR_POLYGON_FAMILIES.M4.basis, 'square');
  assert.equal(STAR_POLYGON_FAMILIES.M4.isotropicInPlane, false);
});

test('star-polygon alpha normalization mirrors around the uniqueness limit', () => {
  assert.deepEqual(normalizeStarPolygonAlpha('M1', 30), {
    inputDeg: 30,
    effectiveDeg: 30,
    mirrored: false,
    regime: 'star',
  });
  assert.deepEqual(normalizeStarPolygonAlpha('M1', 150), {
    inputDeg: 150,
    effectiveDeg: 120,
    mirrored: true,
    regime: 'convex',
  });
  assert.equal(normalizeStarPolygonAlpha('M2', 120).regime, 'star-limit');
  assert.equal(normalizeStarPolygonAlpha('M4', 120).regime, 'uniqueness-limit');
  assert.throws(() => normalizeStarPolygonAlpha('M4', 241), /0..240/);
});

test('star-polygon dent radius ratio reaches the expected geometric limits', () => {
  assert.ok(Math.abs(starPolygonDentRadiusRatio(4, 90) - Math.SQRT1_2) < 1e-12);
  assert.ok(Math.abs(starPolygonDentRadiusRatio(4, 135) - 1) < 1e-12);
  assert.ok(Math.abs(starPolygonDentRadiusRatio(3, 60) - 0.5) < 1e-12);
});

test('star-polygon lattice generator authors resolvable Dry L1 ops', () => {
  for (const family of ['M1', 'M2', 'M3', 'M4'] as const) {
    const ops = starPolygonLatticeOps({ family, alphaDeg: 30, cols: 2, rows: 2, layers: 1, unit: 12 });
    assert.equal(ops[0].op, 'geometry');
    assert.ok(ops.some((op) => op.op === 'temperature' && op.nozzle === 210));
    assert.ok(ops.some((op) => op.op === 'speed' && op.print === 1000));

    const design = starPolygonLattice({ family, alphaDeg: 30, cols: 2, rows: 2, layers: 1, unit: 12 });
    const ir = design.ir();
    assert.ok(ir.segments.length > 12, `${family} should emit a non-trivial toolpath`);
    assert.ok(ir.segments.some((segment) => segment.travel), `${family} should include ordered repositioning travels`);
    assert.ok(ir.segments.some((segment) => !segment.travel), `${family} should include extruding paths`);
  }
});

test('star-polygon M1 path matches the original Colab repeating-unit walk', () => {
  const ops = starPolygonLatticeOps({
    family: 'M1',
    alphaDeg: 30,
    segLength: 4.33,
    cols: 2,
    rows: 1,
    layers: 1,
    sacrificialReturn: false,
  });
  const moves = ops.filter((op): op is Extract<Op, { op: 'move' }> => op.op === 'move');
  assert.deepEqual(
    moves.slice(0, 4).map((op) => [op.x, op.y, op.z]),
    [
      [30, 30, 0.16],
      [34.182459, 28.879314, 0.16],
      [37.244231, 25.817541, 0.16],
      [36.123545, 30, 0.16],
    ]
  );
});

test('TPMS field specs expose the requested surface families', () => {
  for (const surface of ['gyroid', 'schwarz-p', 'schwarz-d', 'iwp', 'neovius', 'fischer-koch-s', 'frd'] as const) {
    assert.equal(TPMS_SURFACES[surface].surface, surface);
    assert.ok(TPMS_SURFACES[surface].equation.length > 10);
  }
  assert.equal(tpmsField('schwarz-p', 0, 0, 0), 3);
  assert.equal(tpmsField('gyroid', 0, 0, 0), 0);
  assert.equal(tpmsField('neovius', 0, 0, 0), 13);
});

test('TPMS generator slices implicit fields into resolvable Dry contours', () => {
  for (const surface of ['gyroid', 'schwarz-p', 'schwarz-d', 'iwp', 'neovius', 'fischer-koch-s', 'frd'] as const) {
    const ops = tpmsOps({
      surface,
      cellsX: 1,
      cellsY: 1,
      cellsZ: 1,
      cellSize: 10,
      samplesPerCell: 10,
      layerHeight: 2,
      minPathLength: 0,
    });
    assert.equal(ops[0].op, 'geometry');
    assert.ok(ops.length > 20, `${surface} should generate contour ops`);

    const ir = tpms({
      surface,
      cellsX: 1,
      cellsY: 1,
      cellsZ: 1,
      cellSize: 10,
      samplesPerCell: 10,
      layerHeight: 2,
      minPathLength: 0,
    }).ir();
    assert.ok(ir.segments.some((segment) => !segment.travel), `${surface} should include extrusion segments`);
  }

  const loose = tpmsOps({ surface: 'gyroid', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 10, samplesPerCell: 10, layerHeight: 2 });
  const framed = tpmsOps({
    surface: 'gyroid',
    cellsX: 1,
    cellsY: 1,
    cellsZ: 1,
    cellSize: 10,
    samplesPerCell: 10,
    layerHeight: 2,
    perimeter: true,
  });
  assert.ok(framed.length > loose.length, 'TPMS perimeter mode should add bounded layer loops');

  const printableDefault = tpmsOps({
    surface: 'gyroid',
    cellsX: 1,
    cellsY: 1,
    cellsZ: 1,
    cellSize: 6,
    samplesPerCell: 4,
    minPathLength: 0,
  });
  assert.deepEqual(printableDefault[0], { op: 'geometry', width: 0.45, height: 0.28 });

  const coarse = tpmsOps({
    surface: 'gyroid',
    cellsX: 1,
    cellsY: 1,
    cellsZ: 1,
    cellSize: 10,
    samplesPerCell: 8,
    layerHeight: 1.2,
    minPathLength: 0,
  });
  const adaptive = tpmsOps({
    surface: 'gyroid',
    cellsX: 1,
    cellsY: 1,
    cellsZ: 1,
    cellSize: 10,
    samplesPerCell: 8,
    layerHeight: 1.2,
    minPathLength: 0,
    adaptive: true,
    adaptiveMinLayerHeight: 0.15,
    adaptiveMaxLayerHeight: 0.3,
  });
  assert.ok(adaptive.length > coarse.length, 'TPMS adaptive mode should insert extra slices in coarse/bad intervals');
  assert.throws(
    () => tpmsOps({
      cellsX: 8,
      cellsY: 8,
      cellsZ: 8,
      cellSize: 80,
      samplesPerCell: 64,
      layerHeight: 0.08,
      maxFieldSamples: 100_000,
    }),
    /TPMS resolution budget exceeded/,
    'TPMS should reject runaway marching-squares resolutions'
  );
});
