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

test('verify surfaces retraction and first-layer findings from structured limits', () => {
  const d = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(50, 0, 0.2) // first-layer extrude at z=0.2, height 0.2, speed 1000
    .extruder(false)
    .point(120, 0, 0.2) // 70 mm travel without a retraction
    .retract(5, 3000); // retraction distance 5 mm at 3000 mm/min

  const report = d.verify(
    'generic',
    0, // maxFlow (unset)
    0, // minTemp (unset)
    '', // bounds (unset)
    false, // monotonicZ
    '', // speedRange (unset)
    2, // maxRetractionDistance (mm)
    1000, // maxRetractionSpeed (mm/min)
    30, // maxTravelWithoutRetract (mm)
    [0.3, 0.5], // firstLayerHeightRange (mm)
    [2000, 3000] // firstLayerSpeedRange (mm/min)
  );

  const rules = new Set(report.findings.map((f) => f.rule));
  assert.ok(rules.has('retraction-distance'), 'expected retraction-distance finding');
  assert.ok(rules.has('retraction-speed'), 'expected retraction-speed finding');
  assert.ok(rules.has('travel-without-retraction'), 'expected travel-without-retraction finding');
  assert.ok(rules.has('first-layer-height'), 'expected first-layer-height finding');
  assert.ok(rules.has('first-layer-speed'), 'expected first-layer-speed finding');
});

test('verify first-layer ranges accept legacy CSV strings equal to the structured form', () => {
  const d = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(50, 0, 0.2);

  const structured = d.verify('generic', 0, 0, '', false, '', 0, 0, 0, [0.3, 0.5], [2000, 3000]);
  const csv = d.verify('generic', 0, 0, '', false, '', 0, 0, 0, '0.3,0.5', '2000,3000');

  assert.deepEqual(structured, csv);
  assert.ok(structured.findings.some((f) => f.rule === 'first-layer-height'));
  assert.ok(structured.findings.some((f) => f.rule === 'first-layer-speed'));
});
