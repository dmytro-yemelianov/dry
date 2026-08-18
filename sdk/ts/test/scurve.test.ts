import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { computeSCurveProfile, importStepNc } from '../src/index.js';

describe('S-Curve Profiler & STEP-NC Importer Suite', () => {
  it('computes 7-phase S-curve profile with bounded jerk', () => {
    const profile = computeSCurveProfile(0, 200, 2000, 20000);
    assert.strictEqual(profile.peak_acceleration, 2000);
    assert(Math.abs(profile.t_jerk_inc - 0.1) < 1e-5);
    assert(profile.total_duration > 0);
    assert(profile.total_distance > 0);
  });

  it('imports ISO 14649 STEP-NC XML workingsteps into Dry L1 ops', () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<stepnc xmlns="urn:iso:std:iso-10303-14649">
  <workingsteps>
    <workingstep id="ws-1" type="hole" x="20" y="30" diameter="6.0" depth="10.0" feed="800"/>
  </workingsteps>
</stepnc>`;

    const ops = importStepNc(xml);
    assert(ops.length > 0);
    // Hole should include rapid move, plunge cut, and retract
    const json = JSON.stringify(ops);
    assert(json.includes('"x":20') || json.includes('"x": 20'));
    assert(json.includes('"y":30') || json.includes('"y": 30'));
    assert(json.includes('"z":-10') || json.includes('"z": -10'));
  });
});
