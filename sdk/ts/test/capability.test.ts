import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Design, MachineCapabilities } from '../src/index';

describe('Pre-flight machine capability checks (D2.2)', () => {
  it('detects out-of-bounds moves and feedrate overshoots', () => {
    const design = new Design()
      .point(0, 0, 0)
      .speed(3500)
      .point(250, 50, 10);

    const caps: MachineCapabilities = {
      name: 'Small-CNC',
      xRange: { min: 0, max: 200 },
      yRange: { min: 0, max: 200 },
      zRange: { min: 0, max: 100 },
      maxFeedrate: 3000,
    };

    const report = design.checkCompatibility(caps);
    assert.strictEqual(report.compatible, false);
    assert.ok(report.findings.some((f) => f.code === 'OUT_OF_BOUNDS_X'));
    assert.ok(report.findings.some((f) => f.code === 'EXCEEDS_MAX_FEEDRATE'));
  });

  it('refuses an arc whose swept circle leaves the envelope', () => {
    // Both endpoints sit inside X [0, 80]. The circle about (50, 50) with radius 40 spans
    // X [10, 90] and leaves it, so only the arc rule can refuse this program — exactly what a
    // check that walks segment endpoints alone gets wrong.
    const design = new Design()
      .point(50, 10, 0)
      .arc({ cx: 50, cy: 50, x: 50, y: 90, z: 0 });

    const caps: MachineCapabilities = {
      name: 'Small-Mill',
      xRange: { min: 0, max: 80 },
      yRange: { min: 0, max: 100 },
      zRange: { min: 0, max: 50 },
    };

    const report = design.checkCompatibility(caps);
    const codes = report.findings.map((f) => f.code);
    assert.ok(codes.includes('ARC_OUT_OF_BOUNDS_X'), codes.join(','));
    assert.ok(!codes.includes('OUT_OF_BOUNDS_X'), codes.join(','));
    assert.strictEqual(report.compatible, false);
  });

  it('passes a fully compliant design', () => {
    const design = new Design()
      .point(10, 10, 0)
      .speed(1500)
      .point(50, 50, 10);

    const caps: MachineCapabilities = {
      name: 'Small-CNC',
      xRange: { min: 0, max: 200 },
      yRange: { min: 0, max: 200 },
      zRange: { min: 0, max: 100 },
      maxFeedrate: 10000,
    };

    const report = design.checkCompatibility(caps);
    assert.strictEqual(report.compatible, true);
    assert.strictEqual(report.findings.length, 0);
  });
});
