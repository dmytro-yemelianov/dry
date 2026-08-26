// Parity additions over the wasm engine: Design.binary(), Design.balancedIr(), Design.verify(kinematics),
// the free resolveBinary/resolveMetricsIr, and resolveMetricsIr over a resolved IR. These mirror the
// Python SDK surface and exercise the freshly wired wasm exports.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  Design,
  resolveBinary,
  resolveMetricsIr,
  type MachineKinematics,
  type Op,
  RESOLVE_PARAMS,
} from '../src/index';

// A small extruding square — enough to produce a non-trivial toolpath / binary archive.
function square(): Design {
  return new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .point(10, 10, 0.2)
    .point(0, 10, 0.2)
    .point(0, 0, 0.2);
}

const SQUARE_OPS: Op[] = square().ops;

// The columnar DRY0 archive starts with the 4-byte ASCII magic "DRY0".
const DRY0_MAGIC = Uint8Array.from([0x44, 0x52, 0x59, 0x30]);

test('Design.binary() returns a non-empty DRY0 binary archive', () => {
  const bytes = square().binary();
  assert.ok(bytes instanceof Uint8Array, 'binary() must return a Uint8Array');
  assert.ok(bytes.length > 4, 'binary archive should be non-trivial');
  assert.deepEqual(bytes.slice(0, 4), DRY0_MAGIC, 'binary must start with the DRY0 magic');
});

test('resolveBinary agrees with Design.binary()', () => {
  const free = resolveBinary(SQUARE_OPS, RESOLVE_PARAMS);
  const method = square().binary();
  assert.deepEqual(free, method);
});

test('Design.balancedIr() returns segments and matches the free resolveBalancedIr fallback', () => {
  const tp = square().balancedIr();
  assert.ok(tp.segments.length > 0, 'balancedIr must produce segments');
});

test('Design.balancedIr(kinematics) lowers corner feedrate vs the no-kinematics fallback', () => {
  // A sharp right-angle corner (open L): junction-velocity capping bites at the (20,0) corner.
  const corner = () =>
    new Design()
      .geometry(0.6, 0.2)
      .extruder(true)
      .point(0, 0, 0.2)
      .point(20, 0, 0.2)
      .point(20, 20, 0.2);
  const kinematics: MachineKinematics = {
    max_acceleration_mm_s2: 500,
    max_junction_velocity_mm_s: 5, // 5 mm/s ≈ 300 mm/min — well below the default 1000 mm/min
  };
  const withKin = corner().balancedIr('generic', kinematics);
  const withoutKin = corner().balancedIr('generic');

  const minWith = Math.min(...withKin.segments.map((s) => s.speed));
  const minWithout = Math.min(...withoutKin.segments.map((s) => s.speed));
  assert.ok(
    minWith < minWithout,
    `kinematics must lower the minimum corner speed (with=${minWith.toFixed(1)}, without=${minWithout.toFixed(1)})`
  );
});

test('Design.verify(kinematics) surfaces a peak-acceleration finding on a fast arc', () => {
  // CCW quarter-circle r=5 at 1000 mm/min ⇒ centripetal a ≈ 55.7 mm/s², above a 50 mm/s² ceiling.
  const arc = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(5, 0, 0.2)
    .arc({ cx: 0, cy: 0, x: 0, y: 5, z: 0.2, clockwise: false });

  const report = arc.verify({ kinematics: { max_acceleration_mm_s2: 50 } });
  assert.ok(
    report.findings.some((f) => f.rule === 'peak-acceleration'),
    'expected a peak-acceleration finding under tight kinematics'
  );
});

test('Design.verify() without kinematics does not surface kinematic findings', () => {
  const arc = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(5, 0, 0.2)
    .arc({ cx: 0, cy: 0, x: 0, y: 5, z: 0.2, clockwise: false });
  const report = arc.verify();
  for (const rule of ['peak-acceleration', 'junction-velocity']) {
    assert.ok(!report.findings.some((f) => f.rule === rule), `unexpected ${rule} finding`);
  }
});

test('resolveMetricsIr over a resolved IR matches the design simulate metrics', () => {
  const d = square();
  const fromDesign = d.simulate();
  const metricsFromIr = resolveMetricsIr(JSON.stringify(d.ir()));
  // Simulating the resolved IR must reproduce the design's own simulation metrics exactly.
  assert.deepEqual(metricsFromIr, fromDesign);
});

test('the power channel authors onto segments and survives optimisation', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .power(600)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .power(0)
    .point(20, 0, 0.2);
  assert.deepEqual(d.ir().segments.map((s) => s.power), [600, 600, 0]);
  // the optimiser coalesces the two lit moves but must not swallow the commanded beam-off.
  assert.ok(
    d.optimizedIr().segments.some((s) => s.power === 0),
    'optimisation deleted the commanded beam-off'
  );
});

test('the default flavor refuses the power channel instead of dropping it', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .power(600)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2);
  // `gcode()` emits with the default (Marlin) flavor, which has no rendering for the channel.
  assert.throws(() => d.gcode(), /cannot render the spindle\/laser power channel/);
});

test('a clothoid corner blend resolves and emits gcode via the wasm engine', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .clothoid({ corner_x: 10, corner_y: 0, blend: 3, x: 10, y: 10, z: 0.2 });
  const gcode = d.gcode();
  assert.ok(gcode.length > 5, 'clothoid should resolve to multiple line moves');
  const ir = d.ir();
  assert.ok(ir.segments.length > 1, 'clothoid should produce multiple segments');
});

