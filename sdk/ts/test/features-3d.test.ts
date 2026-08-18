import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Design, FeatureProgram, feature } from '../src/index';

test('FeatureProgram 3D quaternion rotation transforms positions and orientations', () => {
  const local = new Design()
    .point(0, 0, 0)
    .orient(0, 0, 1)
    .point(10, 0, 0);

  // 90 deg rotation around Y axis: q = (0, sin(45°), 0, cos(45°)) = (0, 0.70710678, 0, 0.70710678)
  const qY90 = {
    x: 0,
    y: Math.SQRT1_2,
    z: 0,
    w: Math.SQRT1_2,
  };

  const program = new FeatureProgram().add(
    feature(local, { x: 20, y: 30, z: 40, rotation: qY90 }, 'angled_arm')
  );

  const ops = program.expand().ops;
  assert.equal(ops.length, 3);

  // First move: (0,0,0) -> (20, 30, 40)
  assert.deepEqual(ops[0], { op: 'move', x: 20, y: 30, z: 40 });

  // Orient op: (0,0,1) rotated around Y by 90° -> (1, 0, 0)
  assert.equal(ops[1].op, 'orient');
  if (ops[1].op === 'orient') {
    assert.ok(Math.abs(ops[1].i - 1.0) < 1e-6);
    assert.ok(Math.abs(ops[1].j) < 1e-6);
    assert.ok(Math.abs(ops[1].k) < 1e-6);
  }

  // Second move: (10,0,0) rotated around Y by 90° -> (0, 0, -10), translated -> (20, 30, 30)
  assert.equal(ops[2].op, 'move');
  if (ops[2].op === 'move') {
    assert.ok(Math.abs((ops[2].x ?? 0) - 20) < 1e-6);
    assert.ok(Math.abs((ops[2].y ?? 0) - 30) < 1e-6);
    assert.ok(Math.abs((ops[2].z ?? 0) - 30) < 1e-6);
  }
});
