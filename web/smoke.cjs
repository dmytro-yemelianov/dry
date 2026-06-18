// Node smoke test: the wasm engine must reproduce the FullControl oracle byte-for-byte.
// Usage: node web/smoke.cjs <pkg-node-dir> <gcode-dir> <simulate-dir>
const fs = require('fs');
const path = require('path');

const pkgDir = process.argv[2];
const gcodeDir = process.argv[3];
const simDir = process.argv[4];
const dry = require(path.resolve(pkgDir, 'dry_wasm.js'));

let checked = 0;
for (const name of fs.readdirSync(gcodeDir)) {
  if (!name.endsWith('.json')) continue;
  const fx = JSON.parse(fs.readFileSync(path.join(gcodeDir, name), 'utf8'));
  const opsJson = JSON.stringify(fx.l1.ops);
  const paramsJson = JSON.stringify(fx.resolve_params);

  const got = dry.resolve_gcode(opsJson, paramsJson, fx.params.relative_e);
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
console.log(`wasm engine reproduced the oracle for ${checked} designs (incl spiral_vase)`);
