// The TS SDK authors a design and the engine reproduces the FullControl oracle (clean-room proof):
// the same Rust engine, via wasm, emits g-code byte-identical to `conformance/gcode/*.json`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Design, resolveGcode, RESOLVE_PARAMS } from '../src/index';

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
