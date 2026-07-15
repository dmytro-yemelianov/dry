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

test('aliased and namespace SDK imports resolve through the injected module', () => {
  const aliased = runSnippet(`import { tpms as makeTpms } from '@dry/sdk';\nmakeTpms({})`, fakeDry);
  const namespace = runSnippet(`import * as dry from '@dry/sdk';\ndry.tpms({})`, fakeDry);
  expect(aliased).toEqual({ ok: true, value: { tag: 'tpms-design' } });
  expect(namespace).toEqual({ ok: true, value: { tag: 'tpms-design' } });
});

test('module-looking text in strings and comments is not rewritten', () => {
  const src = `const text = "import { Missing } from 'not-a-module'";\n// export default Missing\ntext`;
  expect(runSnippet(src, fakeDry)).toEqual({ ok: true, value: "import { Missing } from 'not-a-module'" });
});

test('imports outside the live SDK fail with an explicit error', () => {
  const result = runSnippet(`import thing from 'elsewhere';\nthing`, fakeDry);
  expect(result.ok).toBe(false);
  if (!result.ok) expect(result.error).toContain('unsupported live-docs import: elsewhere');
});

test('semicolonless multi-statement snippets return the last expression', () => {
  const src = `import { Design } from '@dry/sdk';\nconst d = new Design()\nd.point(0, 0, 0.2)\nd`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect((r.value as { gcode(): string[] }).gcode()).toEqual(['G1 X10 Y0']);
});

test('final expressions after declarations are returned', () => {
  const src = `import { Design } from '@dry/sdk';\nfunction make() { return new Design() }\nmake()`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect((r.value as { gcode(): string[] }).gcode()).toEqual(['G1 X10 Y0']);
});

test('trailing comments do not hide semicolonless final expressions', () => {
  const src = `import { Design } from '@dry/sdk';\nconst d = new Design() // setup\nd`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect((r.value as { gcode(): string[] }).gcode()).toEqual(['G1 X10 Y0']);
});

test('trailing comments on the final expression do not comment out the wrapper', () => {
  const src = `import { Design } from '@dry/sdk';\nconst d = new Design()\nd // preview result`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(true);
  if (r.ok) expect((r.value as { gcode(): string[] }).gcode()).toEqual(['G1 X10 Y0']);
});

test('a final expression followed by a semicolon is returned', () => {
  const result = runSnippet(`const value = 41;\nvalue + 1;`, fakeDry);
  expect(result).toEqual({ ok: true, value: 42 });
});

test('else clauses are not rewritten as final expressions', () => {
  const src = `if (false) {\n  throw new Error('bad')\n}\nelse {\n  throw new Error('expected')\n}`;
  const r = runSnippet(src, fakeDry);
  expect(r.ok).toBe(false);
  if (!r.ok) expect(r.error).toMatch(/expected/);
});
