import { test } from 'node:test';
import assert from 'node:assert/strict';
import { pocket, pocketOps, type PocketOptions } from '../src/index';

test('pocket generates valid rectangular pocket ops and gcode', () => {
  const options: PocketOptions = {
    shape: 'rect',
    x: 0,
    y: 0,
    width: 40,
    height: 30,
    toolDiameter: 6,
    depth: 5,
    depthPerPass: 2.5,
    cutFeed: 1200,
    plungeFeed: 300,
  };
  const ops = pocketOps(options);
  assert.ok(ops.length > 5, 'rectangular pocket should produce ops');

  const design = pocket(options);
  const gcode = design.gcode();
  assert.ok(gcode.length > 5, 'pocket design should emit gcode');
  assert.ok(gcode.some((line) => line.includes('Z-2.5') || line.includes('Z-5')));
});

test('pocket generates valid circular pocket ops', () => {
  const options: PocketOptions = {
    shape: 'circle',
    cx: 50,
    cy: 50,
    radius: 20,
    toolDiameter: 4,
    depth: 3,
    depthPerPass: 1.5,
  };
  const ops = pocketOps(options);
  assert.ok(ops.length > 5, 'circular pocket should produce ops');
  const design = pocket(options);
  const ir = design.ir();
  assert.ok(ir.segments.length > 5, 'circular pocket should resolve to toolpath segments');
});

test('pocket throws descriptive error when tool is larger than pocket', () => {
  const options: PocketOptions = {
    shape: 'rect',
    x: 0,
    y: 0,
    width: 5,
    height: 5,
    toolDiameter: 10,
    depth: 2,
  };
  assert.throws(() => pocketOps(options), /tool_diameter|pocket/);
});

test('Design.pocket fluent builder method appends pocket ops', () => {
  const { Design } = require('../src/index');
  const d = new Design();
  d.pocket({
    shape: 'rect',
    x: 0,
    y: 0,
    width: 40,
    height: 30,
    toolDiameter: 6,
    depth: 5,
    depthPerPass: 2.5,
  });
  assert.ok(d.ops.length > 5, 'Design.pocket should append ops to design');
  const ir = d.ir();
  assert.ok(ir.segments.length > 5, 'Design with pocket should resolve to toolpath');
});

