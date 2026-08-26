// Binding-level coverage for the H1 hardening rejections (#192).
//
// Four slices narrowed what the engine accepts — H1.1 (emit gate), H1.2 (ingress validation),
// H1.4 (TPMS vacuity) and H1.3 (structural verify rules) — and each was judged "a coverage gap
// rather than a live risk" on the grounds that nothing which previously *worked* is now refused.
// That reasoning was never checked from a binding surface, which is where it matters: this layer
// has swallowed a refusal before. ADR 0002 §3 records `web/viewer.js` reading a degenerate result
// as success, and H1.1's second review found refused IR reaching `sdk/ts` as an empty array that
// rendered as a successful blank program.
//
// These tests run against the real compiled wasm engine, so they exercise the JS/Rust boundary the
// Rust suite cannot reach.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Design, tpms } from '../src/index';

/** A one-segment five-axis design with an explicit toolframe orientation. */
const orientedDesign = (i: number, j: number, k: number) =>
  new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .orient(i, j, k)
    .point(10, 0, 0.2);

test('a refused TPMS option set throws rather than yielding an empty program', () => {
  // H1.4: isoLevel outside the field's range traces no contour on any layer. Before the fix this
  // resolved, verified with zero findings and simulated to zero volume — a program that "succeeds"
  // and deposits nothing is the confidently-wrong artifact ADR 0002 §4 forbids.
  assert.throws(
    () => tpms({ isoLevel: 2.0, cellsX: 2, cellsY: 2, cellsZ: 2 }),
    /isoLevel/,
    'an option set that can deposit no material must be refused, by name',
  );

  // The control: the same call without the bad isoLevel produces ops.
  const ok = tpms({ cellsX: 2, cellsY: 2, cellsZ: 2 });
  assert.ok(ok.ops.length > 0, 'the refusal must be specific to the bad option set');
});

test('a zero-magnitude toolframe orientation is refused at ingress', () => {
  // H1.2: there is no tool direction to recover from a zero vector, so it cannot be normalised and
  // must not be silently treated as +Z.
  assert.throws(
    () => orientedDesign(0, 0, 0).gcode('generic', true, false, true, 'ab'),
    /non-zero magnitude/,
    'a zero orientation must be refused, not defaulted',
  );
});

test('a non-unit orientation is normalised, so all three rotary models agree', () => {
  // This is the regression test for audit finding C2. `ab` recovers tilt with `atan2` and is
  // scale-invariant, while `ac`/`bc` use `acos(k)` and assume ‖v‖ = 1 — so before H1.1 the same
  // orientation produced *different* angles under different models, and `[0,0,0.5]` put the linear
  // axes at the wrong point entirely (`Z-8.660254 B60`).
  //
  // The fix normalises rather than refusing, which is the stronger choice: a non-unit direction
  // vector is unambiguous, so there is nothing to refuse. What must hold is that scaling the vector
  // changes nothing about the emitted program.
  for (const axes of ['ab', 'ac', 'bc'] as const) {
    const scaled = orientedDesign(0, 0, 0.5).gcode('generic', true, false, true, axes);
    const unit = orientedDesign(0, 0, 1).gcode('generic', true, false, true, axes);
    assert.deepEqual(scaled, unit, `rotary model ${axes} is sensitive to orientation magnitude`);
    assert.ok(scaled.length > 0, `rotary model ${axes} emitted nothing`);
  }
});

test('verify still reports the non-unit orientation that emit tolerates', () => {
  // The two surfaces have different jobs, and both must do theirs: `emit` is robust so it never
  // produces a wrong-point program, while `verify` says the IR is malformed. If only one of them
  // acted, a caller would either get a bad program or no warning about a bad design.
  const report = orientedDesign(0, 0, 0.5).verify('generic');
  assert.ok(
    report.findings.some((f) => f.rule === 'orientation-not-unit'),
    `expected orientation-not-unit, got ${JSON.stringify(report.findings)}`,
  );
});

test('a verify report crossing the boundary states its own coverage', () => {
  // H1.3: `findings: []` is equally true of a clean program and one that was never inspected, and
  // this SDK previously had no way to tell the two apart.
  const report = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .verify('generic');

  assert.deepEqual(report.findings, []);
  assert.ok(
    (report.segments_inspected ?? 0) > 0,
    'a clean report over a real design must say it inspected something',
  );
  for (const rule of ['continuity', 'segment-length', 'arc-length', 'negative-quantity']) {
    assert.ok(
      report.rules_evaluated?.includes(rule),
      `${rule} missing from ${JSON.stringify(report.rules_evaluated)}`,
    );
  }
  assert.ok(report.contracts, 'the limits checked against must cross the boundary too');
});

test('refusals arrive as thrown errors with messages, never as blank success', () => {
  // The historic failure mode this file exists for: a refusal that reaches JavaScript as an empty
  // array reads as a successful program with no moves, and both `web/viewer.js` and this SDK
  // rendered it that way.
  const refusals: Array<[string, () => unknown]> = [
    ['tpms vacuity', () => tpms({ isoLevel: 2.0, cellsX: 2, cellsY: 2, cellsZ: 2 })],
    ['zero orientation', () => orientedDesign(0, 0, 0).gcode('generic', true, false, true, 'ab')],
  ];

  for (const [name, call] of refusals) {
    let threw = false;
    try {
      const result = call();
      assert.fail(
        `[${name}] returned ${JSON.stringify(result)} instead of throwing — a refusal that ` +
          'arrives as a value is indistinguishable from an empty successful program',
      );
    } catch (e) {
      threw = true;
      assert.ok(e instanceof Error, `[${name}] threw a non-Error: ${String(e)}`);
      assert.ok(e.message.length > 0, `[${name}] threw an Error with no message`);
    }
    assert.ok(threw, `[${name}] did not throw`);
  }
});

// The SDK parses the same documented CSV contract formats the Rust CLI does, and used to be far
// more permissive about them: `Number` maps an empty field to 0, 'abc' to NaN and '1e400' to
// Infinity. The empty-field case was the dangerous one — `0,100,0,,0,100` produced a
// plausible-looking build volume instead of an error, and the engine now treats a non-finite
// contract as not in force, so it would have silently dropped the check rather than applying it.
//
// `verify` takes its contracts positionally: (printer, maxFlow, minTemp, bounds, monotonicZ,
// speedRange, ...).
const contractDesign = () =>
  new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(50, 0, 0.2);

test('bounds CSV refuses what the Rust parser refuses', () => {
  for (const bad of ['a,b,c,d,e,f', '0,100,0,,0,100', '0,1e400,0,100,0,100', '0,100,0,100,0,0x10']) {
    assert.throws(
      () => contractDesign().verify('generic', 0, 0, bad),
      /not a number|is empty|must all be finite/,
      `bounds '${bad}' must be refused`,
    );
  }

  const report = contractDesign().verify('generic', 0, 0, '0,100,0,100,0,100');
  assert.ok(
    (report.rules_evaluated ?? []).some((r: string) => r.includes('bounds')),
    'a finite build volume must be in force',
  );
});

test('ranges refuse non-finite components, in CSV and structured form', () => {
  assert.throws(() => contractDesign().verify('generic', 0, 0, '', false, '60,'), /is empty/);
  assert.throws(() => contractDesign().verify('generic', 0, 0, '', false, '60,abc'), /not a number/);
  assert.throws(
    () => contractDesign().verify('generic', 0, 0, '', false, '60,1e400'),
    /must all be finite/,
  );
  assert.throws(
    () => contractDesign().verify('generic', 0, 0, '', false, [60, Number.NaN]),
    /must all be finite/,
  );
  assert.throws(
    () => contractDesign().verify('generic', 0, 0, [[0, 100], [0, Number.POSITIVE_INFINITY], [0, 100]]),
    /must all be finite/,
  );

  const report = contractDesign().verify('generic', 0, 0, '', false, [60, 9000]);
  assert.ok((report.rules_evaluated ?? []).some((r: string) => r.includes('speed')));
});
