// Drift / delegation tests for MachineKinematics (Task 7): verifies that the TS wrappers
// correctly pass kinematics_json to the wasm engine and that the engine behaves as expected.
//
// Two checks:
//   1. resolveBalancedIr with max_junction_velocity_mm_s shapes cornering speeds (drift check):
//      the toolpath produced WITH kinematics has lower corner feedrates than without.
//   2. resolveVerify with a tight max_acceleration_mm_s2 surfaces a peak-acceleration finding
//      for an arc move whose centripetal acceleration exceeds the limit.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  resolveBalancedIr,
  resolveVerify,
  type MachineKinematics,
  type Op,
  RESOLVE_PARAMS,
} from '../src/index';

// A simple two-leg right-angle corner (X then Y) — the sharp 90° direction change is where
// junction-velocity shaping bites. Total design: two extruding line moves meeting at (20, 0).
const CORNER_OPS: Op[] = [
  { op: 'geometry', width: 0.6, height: 0.2 },
  { op: 'extruder', on: true },
  { op: 'move', x: 0, y: 0, z: 0.2 },
  { op: 'move', x: 20, y: 0, z: 0.2 },
  { op: 'move', x: 20, y: 20, z: 0.2 },
];

// An arc op after setup at (5, 0): a CCW quarter-circle, radius 5 mm, centre at origin.
// At 1000 mm/min (≈16.7 mm/s) centripetal a = v²/r = 16.7²/5 ≈ 55.7 mm/s² — above 50.
const ARC_OPS: Op[] = [
  { op: 'geometry', width: 0.6, height: 0.2 },
  { op: 'extruder', on: true },
  { op: 'move', x: 5, y: 0, z: 0.2 },           // move to arc start
  { op: 'arc', cx: 0, cy: 0, x: 0, y: 5, z: 0.2, clockwise: false }, // CCW quarter-circle
];

// ─── Test 1: resolveBalancedIr kinematics shapes cornering speeds ─────────────────────────────

test('resolveBalancedIr with max_junction_velocity_mm_s lowers corner feedrate vs no kinematics', () => {
  const kinematics: MachineKinematics = {
    max_acceleration_mm_s2: 500,
    max_junction_velocity_mm_s: 5,  // 5 mm/s ≈ 300 mm/min — well below default 1000 mm/min
  };

  const withKin = resolveBalancedIr(CORNER_OPS, RESOLVE_PARAMS, kinematics);
  const withoutKin = resolveBalancedIr(CORNER_OPS, RESOLVE_PARAMS);

  assert.ok(withKin.segments.length > 0, 'expected non-empty toolpath with kinematics');
  assert.ok(withoutKin.segments.length > 0, 'expected non-empty toolpath without kinematics');

  // The pipeline applies junction-velocity capping at corners: the minimum segment speed in the
  // kinematics-shaped toolpath must be strictly lower than in the uncapped one.
  const minSpeedWith = Math.min(...withKin.segments.map((s) => s.speed));
  const minSpeedWithout = Math.min(...withoutKin.segments.map((s) => s.speed));

  assert.ok(
    minSpeedWith < minSpeedWithout,
    `kinematics must lower the minimum segment speed at the corner ` +
      `(with=${minSpeedWith.toFixed(1)} mm/min, without=${minSpeedWithout.toFixed(1)} mm/min)`
  );
});

test('resolveBalancedIr without kinematics falls back to safe pipeline (no crash, has segments)', () => {
  const tp = resolveBalancedIr(CORNER_OPS, RESOLVE_PARAMS);
  assert.ok(tp.segments.length > 0, 'safe-pipeline fallback must produce segments');
});

// ─── Test 2: resolveVerify surfaces peak-acceleration for an arc under tight limits ──────────

test('resolveVerify surfaces peak-acceleration finding for a fast arc under tight kinematics', () => {
  // Arc centripetal a ≈ 55.7 mm/s² → exceeds max_acceleration_mm_s2: 50 → peak-acceleration fires.
  const kinematics: MachineKinematics = { max_acceleration_mm_s2: 50 };

  const report = resolveVerify(
    ARC_OPS,
    RESOLVE_PARAMS,
    0, 0, undefined, false, undefined, 0, 0, 0, undefined, undefined,
    kinematics
  );

  const rules = new Set(report.findings.map((f) => f.rule));
  assert.ok(
    rules.has('peak-acceleration'),
    `expected peak-acceleration finding (got: [${[...rules].join(', ')}])`
  );
});

test('resolveVerify without kinematics does not surface kinematic findings', () => {
  // The same arc with no kinematics arg must not fire peak-acceleration or junction-velocity.
  const report = resolveVerify(ARC_OPS, RESOLVE_PARAMS);
  const kinematicRules = ['peak-acceleration', 'junction-velocity'];
  for (const rule of kinematicRules) {
    assert.ok(
      !report.findings.some((f) => f.rule === rule),
      `unexpected ${rule} finding without kinematics`
    );
  }
});
