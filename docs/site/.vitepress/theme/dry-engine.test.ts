import { test, expect } from 'vitest';
import { setWasmBinding, type DryWasm } from '@sdk/engine';
import { getDry } from './dry-engine';

const fake = {
  resolve_ir: () =>
    JSON.stringify({ version: 1, segments: [{ start: [0, 0, 0], end: [10, 0, 0], travel: false, kind: 'line' }] }),
  resolve_gcode: () => ['G1 X10 Y0'],
} as unknown as DryWasm;

test('getDry() exposes the real Design API over the injected binding', () => {
  setWasmBinding(fake);
  const dry = getDry();
  expect(typeof dry.Design).toBe('function');
  expect(typeof dry.tpms).toBe('function');
  const ir = new dry.Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(10, 0, 0.2).ir();
  expect(ir.segments).toHaveLength(1);
  expect(dry.PRINTERS.generic.dia).toBe(1.75);
});
