import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Design, FeatureProgram, feature, group, repeat } from '../src/index';

function localLine(): Design {
  return new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, null, null);
}

test('FeatureProgram expands repeat and pose through the Rust engine', () => {
  const expanded = new FeatureProgram()
    .add(repeat(feature(localLine(), { x: 5 }, 'line'), 2, { x: 20 }))
    .expand();
  const moves = expanded.ops.filter((op) => op.op === 'move');
  assert.deepEqual(
    moves.map((op) => op.x),
    [5, 15, 25, 35]
  );
  assert.ok(moves.every((op) => op.y === 0 && op.z === 0.2));

  const hand = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(5, 0, 0.2)
    .point(15, 0, 0.2)
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(25, 0, 0.2)
    .point(35, 0, 0.2);
  assert.deepEqual(expanded.ir(), hand.ir());
});

test('Group preserves child order', () => {
  const temperature = feature(new Design().temperature(205));
  const fan = feature(new Design().fan(0.5));
  const expanded = new FeatureProgram().add(group(temperature, fan)).expand();
  assert.deepEqual(expanded.ops, [
    { op: 'temperature', nozzle: 205 },
    { op: 'fan', speed: 0.5 },
  ]);
});

test('transformed manual g-code fails closed', () => {
  const program = new FeatureProgram().add(
    feature(new Design().manualGcode('G28'), { x: 1 })
  );
  assert.throws(() => program.expand(), /cannot be transformed safely/);
});
