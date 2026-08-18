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
