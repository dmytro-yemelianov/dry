// Authoring pilot: generate -> verify -> emit with the Dry TypeScript SDK.
// Mirrors examples/authoring.py.
//
// Run from the repository root:
//   cd sdk/ts && npm ci && npm run example:authoring
import { Design } from '@dry/sdk';

// A small first-layer path: a line, a quarter arc (G3) about the origin, then a line.
const design = new Design()
  .geometry(0.6, 0.2) // bead width/height
  .extruder(true)
  .point(10, 0, 0.2) // start
  .arc({ cx: 0, cy: 0, x: 0, y: 10 }) // quarter arc about (0,0) -> ends at (0,10)
  .point(0, 20, 0.2); // finish with a straight line

// 1) verify against a machine envelope BEFORE emitting.
//    bounds is structured [[x0,x1],[y0,y1],[z0,z1]] in mm.
const report = design.verify({ bounds: [[0, 200], [0, 200], [0, 200]] });
const errors = report.findings.filter((f) => f.severity === 'error');
console.log(`verify: ${report.findings.length} finding(s), ${errors.length} error(s)`);
if (errors.length) {
  for (const f of errors) console.log(`  [${f.rule}] seg ${f.segment}: ${f.message}`);
  process.exit(1);
}

// 2) metrics
const m = design.simulate();
console.log(`simulate: ${m.segment_count} segments`);

// 3) emit motion g-code
console.log('g-code:');
console.log(design.gcode().join('\n'));
