// Node smoke test: the wasm engine must reproduce the FullControl oracle byte-for-byte.
// Usage: node web/smoke.cjs <pkg-node-dir> <gcode-dir> <simulate-dir>
const fs = require('fs');
const path = require('path');

const pkgDir = process.argv[2];
const gcodeDir = process.argv[3];
const simDir = process.argv[4];
const dry = require(path.resolve(pkgDir, 'dry_wasm.js'));

let checked = 0;
let verifyFixture;
for (const name of fs.readdirSync(gcodeDir)) {
  if (!name.endsWith('.json')) continue;
  const fx = JSON.parse(fs.readFileSync(path.join(gcodeDir, name), 'utf8'));
  const opsJson = JSON.stringify(fx.l1.ops);
  const paramsJson = JSON.stringify(fx.resolve_params);
  verifyFixture ||= { opsJson, paramsJson };

  const got = dry.resolve_gcode(opsJson, paramsJson, fx.params.relative_e, false, false, 'ab');
  if (JSON.stringify(got) !== JSON.stringify(fx.expected)) {
    console.error(`[${fx.design}] GCODE MISMATCH`);
    console.error('  expected:', JSON.stringify(fx.expected));
    console.error('  got:     ', JSON.stringify(got));
    process.exit(1);
  }

  // metrics parity against the simulate fixture
  const sim = JSON.parse(fs.readFileSync(path.join(simDir, name), 'utf8')).expected;
  const m = JSON.parse(dry.resolve_metrics(opsJson, paramsJson));
  if (m.segment_count !== sim.segment_count) {
    console.error(`[${fx.design}] segment_count ${m.segment_count} != ${sim.segment_count}`);
    process.exit(1);
  }
  if (Math.abs(m.total_time_s - sim.total_time_s) > 1e-9) {
    console.error(`[${fx.design}] total_time_s ${m.total_time_s} != ${sim.total_time_s}`);
    process.exit(1);
  }
  if (Math.abs(m.extruded_volume - sim.extruded_volume) > 1e-9) {
    console.error(`[${fx.design}] extruded_volume ${m.extruded_volume} != ${sim.extruded_volume}`);
    process.exit(1);
  }
  checked++;
}
if (checked < 1) { console.error('no fixtures'); process.exit(1); }

const invertedContracts = [
  ['bounds', new Float64Array([100, 0, 0, 100, 0, 50]), undefined, undefined, undefined],
  ['speed_range', undefined, new Float64Array([9000, 300]), undefined, undefined],
  ['first_layer_height_range', undefined, undefined, new Float64Array([0.5, 0.1]), undefined],
  ['first_layer_speed_range', undefined, undefined, undefined, new Float64Array([3000, 1000])],
];
const resolveVerify = (bounds, speedRange, firstLayerHeightRange, firstLayerSpeedRange) =>
  dry.resolve_verify(
    verifyFixture.opsJson, verifyFixture.paramsJson, 0, 0, bounds, false, speedRange,
    0, 0, 0, firstLayerHeightRange, firstLayerSpeedRange, ''
  );
for (const [name, bounds, speedRange, firstLayerHeightRange, firstLayerSpeedRange] of invertedContracts) {
  try {
    resolveVerify(bounds, speedRange, firstLayerHeightRange, firstLayerSpeedRange);
    console.error(`${name} accepted an inverted range`);
    process.exit(1);
  } catch (error) {
    if (!String(error).includes('lower bound must be <= upper bound')) {
      console.error(`${name} returned an unexpected error:`, error);
      process.exit(1);
    }
  }
}

const equalEndpointContracts = [
  ['bounds', new Float64Array([0, 0, 0, 0, 0.2, 0.2]), undefined, undefined, undefined],
  ['speed_range', undefined, new Float64Array([600, 600]), undefined, undefined],
  ['first_layer_height_range', undefined, undefined, new Float64Array([0.2, 0.2]), undefined],
  ['first_layer_speed_range', undefined, undefined, undefined, new Float64Array([600, 600])],
];
for (const [name, bounds, speedRange, firstLayerHeightRange, firstLayerSpeedRange] of equalEndpointContracts) {
  try {
    JSON.parse(resolveVerify(bounds, speedRange, firstLayerHeightRange, firstLayerSpeedRange));
  } catch (error) {
    console.error(`${name} rejected equal endpoints:`, error);
    process.exit(1);
  }
}
// TPMS delegation (web/tpms-engine.js -> tpms_ops_json): the browser gallery's TPMS Op generation
// has no other behavioural coverage (blocks-regression.mjs only checks static delegation wiring,
// and conformance/ carries no TPMS vectors), so exercise a few surfaces here directly.
const tpmsSurfaces = ['gyroid', 'schwarz-p', 'iwp'];
for (const surface of tpmsSurfaces) {
  const optsJson = JSON.stringify({
    surface, cellSize: 20, cellsX: 1, cellsY: 1, cellsZ: 1,
    samplesPerCell: 12, layerHeight: 0.28, beadWidth: 0.45, beadHeight: 0.28,
    printSpeed: 1200, perimeter: true, maxFieldSamples: 1_000_000,
  });
  const ops = JSON.parse(dry.tpms_ops_json(optsJson));
  if (!Array.isArray(ops) || ops.length === 0) {
    console.error(`[tpms:${surface}] expected a non-empty op list, got`, ops);
    process.exit(1);
  }
  if (ops[0].op !== 'geometry') {
    console.error(`[tpms:${surface}] expected the first op to be 'geometry', got`, ops[0]);
    process.exit(1);
  }
  if (!ops.some((op) => op.op === 'move')) {
    console.error(`[tpms:${surface}] expected at least one 'move' op`);
    process.exit(1);
  }
  if (ops.some((op) => op.op === 'arc')) {
    console.error(`[tpms:${surface}] TPMS generation should never emit 'arc' ops`);
    process.exit(1);
  }
}
try {
  dry.tpms_ops_json(JSON.stringify({ surface: 'not-a-real-surface', cellSize: 20 }));
  console.error('[tpms] expected an unknown surface to be rejected');
  process.exit(1);
} catch (error) {
  // expected: the engine rejects unknown surfaces with a structured error
}
console.log(`wasm engine reproduced the oracle for ${checked} designs (incl spiral_vase), plus ${tpmsSurfaces.length} TPMS surfaces`);
