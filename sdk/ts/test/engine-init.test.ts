// Run in its own process (node --test uses process isolation by default on Node 20+),
// importing ONLY the agnostic engine — so no binding is ever set and bind() must throw.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { resolveGcode } from '../src/engine';
import { RESOLVE_PARAMS } from '../src/ops';

test('agnostic engine throws before a wasm binding is set', () => {
  assert.throws(
    () => resolveGcode([{ op: 'move', x: 0, y: 0, z: 0 }], RESOLVE_PARAMS),
    /not initialised/i
  );
});
