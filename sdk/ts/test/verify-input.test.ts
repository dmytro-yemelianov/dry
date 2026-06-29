// The SDK accepts structured machine limits, not just comma-strings; both forms must agree.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Design } from '../src/index';

test('verify accepts structured bounds/speedRange and agrees with the CSV form', () => {
  const d = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(150, 0, 0.2);

  const structured = d.verify('generic', 0, 0, [[0, 100], [0, 100], [0, 50]], false, [300, 9000]);
  const csv = d.verify('generic', 0, 0, '0,100,0,100,0,50', false, '300,9000');

  assert.deepEqual(structured, csv);
  assert.ok(structured.findings.some((f) => f.rule === 'bounds'));
});
