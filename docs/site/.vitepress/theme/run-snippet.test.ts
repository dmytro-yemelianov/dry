import { test, expect } from 'vitest';
import { runSnippet } from './run-snippet';
import type { Dry } from './dry-engine';

const fakeDry = {
  Design: class {
    ops: unknown[] = [];
    geometry() { return this; }
    extruder() { return this; }
    point() { return this; }
    gcode() { return ['G1 X10 Y0']; }
  },
  tpms: () => ({ tag: 'tpms-design' }),
} as unknown as Dry;

test('idiomatic snippet with an @dry/sdk import runs and returns its last expression', () => {
  const src = `import { Design } from '@dry/sdk';\nnew Design().geometry(0.6, 0.2).extruder(true).point(0,0,0.2)`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect((r.value as { gcode(): string[] }).gcode()).toEqual(['G1 X10 Y0']);
});

test('a throwing snippet is captured, not propagated', () => {
  const r = runSnippet(`throw new Error('boom')`, fakeDry);
  expect(r.ok).toBe(false);
  if (!r.ok) expect(r.error).toMatch(/boom/);
});

test('destructured generator import resolves from the injected dry', () => {
  const r = runSnippet(`import { tpms } from '@dry/sdk';\ntpms({ surface: 'gyroid' })`, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect(r.value).toEqual({ tag: 'tpms-design' });
});
