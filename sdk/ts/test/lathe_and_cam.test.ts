import assert from 'node:assert/strict';
import test, { describe } from 'node:test';
import {
  latheFacingOps,
  latheTurningOps,
  checkToolHolderCollision,
  reverseToolpath,
  resolveIr,
  RESOLVE_PARAMS,
  Op,
  Toolpath,
  ToolHolder,
} from '../src/index';

describe('CNC Lathe & Advanced CAM Suite', () => {
  test('generates lathe facing operations', () => {
    const ops = latheFacingOps({
      stock_diameter: 50.0,
      target_z: 0.0,
      clearance_x: 2.0,
      clearance_z: 2.0,
      feedrate: 300.0,
      spindle_rpm: 1200.0,
      passes: 2,
      depth_per_pass: 1.0,
    });

    assert(ops.length > 0);
    // Facing moves toward center (X=0 or X=-0.5)
    const hasCenterMove = ops.some(
      (op) => op.op === 'move' && op.x !== null && op.x <= 0.0
    );
    assert.equal(hasCenterMove, true);
  });

  test('generates lathe OD turning operations with roughing and finishing', () => {
    const ops = latheTurningOps({
      raw_diameter: 40.0,
      target_diameter: 30.0,
      cut_length: 25.0,
      depth_of_cut: 2.0,
      finish_allowance: 0.5,
      clearance_x: 1.5,
      clearance_z: 1.5,
      rough_feedrate: 250.0,
      finish_feedrate: 150.0,
      spindle_rpm: 1400.0,
    });

    assert(ops.length > 0);
    // Should contain negative Z cuts
    const hasZCut = ops.some(
      (op) => op.op === 'move' && op.z !== null && op.z <= -20.0
    );
    assert.equal(hasZCut, true);
  });

  test('checks 5-axis tool holder collision against stock bounds', () => {
    const ops: Op[] = [
      { op: 'orient', i: 0.0, j: 0.0, k: 1.0 },
      { op: 'move', x: 20.0, y: 20.0, z: -10.0 },
    ];
    const toolpath = resolveIr(ops, RESOLVE_PARAMS);
    const holder: ToolHolder = {
      holder_diameter: 40.0,
      stickout_length: 5.0, // Short stickout causes plunge collision
      collet_diameter: 30.0,
      collet_length: 20.0,
    };
    // Stock bounds: [min_x, max_x, min_y, max_y, min_z, max_z]
    const stockBounds: [number, number, number, number, number, number] = [
      0.0, 100.0, 0.0, 100.0, -50.0, 0.0,
    ];

    const findings = checkToolHolderCollision(toolpath, holder, stockBounds);
    assert(findings.length > 0);
    assert.equal(findings[0].code, 'TOOL_HOLDER_COLLISION');
  });

  test('reverses resolved toolpath into L1 operations', () => {
    const ops: Op[] = [
      { op: 'temperature', nozzle: 210.0 },
      { op: 'fan', speed: 0.75 },
      { op: 'move', x: 0.0, y: 0.0, z: 0.2 },
      { op: 'move', x: 20.0, y: 0.0, z: 0.2 },
    ];
    const toolpath = resolveIr(ops, RESOLVE_PARAMS);
    const reversedOps = reverseToolpath(toolpath);

    assert(reversedOps.length > 0);
    const hasTemp = reversedOps.some(
      (op) => op.op === 'temperature' && op.nozzle === 210.0
    );
    assert.equal(hasTemp, true);
  });
});
