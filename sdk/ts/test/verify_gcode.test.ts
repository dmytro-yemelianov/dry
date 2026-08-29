import assert from 'node:assert/strict';
import test, { describe } from 'node:test';
import { verifyGcode } from '../src/index';

describe('Serverless Wasm G-Code Verification Suite', () => {
  test('verifies compliant raw G-code text with 0 findings in-process via Wasm', () => {
    const gcode = `
; Dry G-code Test
G92 E0
G1 F1200 X0 Y0 Z0.2 E0
G1 F1800 X10 Y0 Z0.2 E0.5
G1 F1800 X10 Y10 Z0.2 E1.0
G1 F1800 X0 Y10 Z0.2 E1.5
G1 F1800 X0 Y0 Z0.2 E2.0
`;
    const report = verifyGcode(gcode);

    assert(report !== null);
    assert(report.segments_inspected !== undefined && report.segments_inspected > 0);
    assert.equal(report.findings.length, 0);
  });

  test('surfaces out-of-bounds contract violations from raw G-code without server/container', () => {
    const gcode = `
G1 F1800 X0 Y0 Z0.2 E0
G1 F1800 X500 Y0 Z0.2 E5.0
`;
    // Enforce tight machine envelope bounds: X [0, 200], Y [0, 200], Z [0, 200]
    const contracts = {
      bounds: [
        [0.0, 200.0],
        [0.0, 200.0],
        [0.0, 200.0],
      ],
    };

    const report = verifyGcode(gcode, contracts);
    assert(report.findings.length > 0);
    const boundsFinding = report.findings.find((f) => f.rule === 'bounds');
    assert(boundsFinding !== undefined);
    assert.equal(boundsFinding.severity, 'error');
  });
});
