/**
 * Cross-target parity for the Phase 7/8 kernel additions.
 *
 * `optimizeFiveAxisLookahead` and `analyzeMachiningPhysics` reached `dry-core` without reaching any
 * binding (`docs/14-known-limitations.md`). These pin the TypeScript half of closing that, and the
 * physics numbers are checked against the Python SDK's in `py/tests/test_physics_and_lookahead.py` —
 * both call the same engine, so they must agree.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  Design,
  analyzeMachiningPhysics,
  optimizeFiveAxisLookahead,
  drapeOps,
  parseObjMesh,
  type CuttingToolGeometry,
  type MachiningOperationParams,
} from '../src/index';

const tool: CuttingToolGeometry = {
  diameter_mm: 10.0,
  flute_count: 4,
  stickout_length_mm: 40.0,
  core_diameter_ratio: 0.6,
  modulus_gpa: 600.0,
  corner_radius_mm: 0.5,
};

const params: MachiningOperationParams = {
  spindle_rpm: 8000.0,
  feedrate_mm_min: 1200.0,
  axial_depth_ap_mm: 5.0,
  radial_depth_ae_mm: 2.0,
  ambient_temp_c: 20.0,
};

describe('Machining physics', () => {
  it('reports every documented metric', () => {
    const r = analyzeMachiningPhysics(tool, 'Aluminum6061', params);
    assert.ok(r.cutting_speed_m_min > 0);
    assert.ok(r.material_removal_rate_cm3_min > 0);
    assert.ok(r.tangential_force_n > 0);
    assert.equal(typeof r.chatter_risk, 'boolean');
  });

  it('reports a clamped result as saturated rather than as a prediction', () => {
    const r = analyzeMachiningPhysics(tool, 'TitaniumTi6Al4V', params);
    assert.equal(r.model_saturated, true);
    assert.equal(r.estimated_tool_life_min, 0.1);
    assert.equal(r.shear_temperature_c, 1220);

    const sane = { ...params, spindle_rpm: 6000, feedrate_mm_min: 900, axial_depth_ap_mm: 2, radial_depth_ae_mm: 3 };
    assert.equal(analyzeMachiningPhysics(tool, 'Aluminum6061', sane).model_saturated, false);
  });

  it('distinguishes a hard alloy from aluminium', () => {
    const alu = analyzeMachiningPhysics(tool, 'Aluminum6061', params);
    const inc = analyzeMachiningPhysics(tool, 'Inconel718', params);
    assert.ok(inc.tangential_force_n > alu.tangential_force_n);
    assert.ok(inc.estimated_tool_life_min < alu.estimated_tool_life_min);
  });

  it('refuses an unknown material rather than defaulting', () => {
    assert.throws(() =>
      // @ts-expect-error — deliberately outside the union, to prove the engine refuses it too
      analyzeMachiningPhysics(tool, 'Unobtainium', params)
    );
  });
});

describe('Five-axis lookahead', () => {
  it('preserves segment count and never speeds a segment up', () => {
    let d = new Design().geometry(0.4, 0.2).point(0, 0, 0.2);
    for (let i = 1; i <= 5; i++) d = d.point(i * 10, 0, 0.2);
    const tp = d.ir() as { segments: { speed: number }[] };

    const out = optimizeFiveAxisLookahead(tp, {
      max_linear_accel: 500,
      max_linear_jerk: 5000,
      max_rotary_speed_deg_s: 60,
      max_rotary_accel_deg_s2: 300,
      max_rotary_jerk_deg_s3: 3000,
    }) as { segments: { speed: number }[] };

    assert.equal(out.segments.length, tp.segments.length);
    out.segments.forEach((s, i) => {
      assert.ok(s.speed <= tp.segments[i].speed + 1e-9);
    });
  });
});

describe('Industrial dialects from TypeScript', () => {
  const frame = { wcs: 54, tool: 3, spindle_rpm: 8000, coolant: true };
  const square = () =>
    new Design().geometry(0.4, 0.2).point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2);

  it('reaches every Phase 8 dialect', () => {
    const cases: [Parameters<Design['gcode']>[0], string][] = [];
    for (const [flavor, marker, fiveAxis] of [
      ['siemens', 'TRAORI', true],
      ['heidenhain', 'BEGIN PGM', false],
      ['haas', 'G187', false],
      ['rapid', 'MODULE DryProgram', false],
    ] as const) {
      const lines = square().gcode({ flavor, fiveAxis, cncFrame: frame });
      assert.ok(
        lines.some((l) => l.includes(marker)),
        `${flavor}: no ${marker} in ${lines.slice(0, 8).join(' | ')}`
      );
    }
    void cases;
  });

  it('emits no machine preamble without a cncFrame', () => {
    const bare = square().gcode({ flavor: 'siemens', fiveAxis: true });
    assert.ok(!bare.some((l) => l.includes('TRAORI')));
    const framed = square().gcode({ flavor: 'siemens', fiveAxis: true, cncFrame: frame });
    assert.ok(framed.some((l) => l.includes('TRAORI')));
    assert.ok(framed.some((l) => l.includes('S8000 M3')));
  });

  it('refuses an unknown flavor rather than emitting Marlin', () => {
    assert.throws(() =>
      // @ts-expect-error — outside the union on purpose; the engine must refuse it too
      square().gcode({ flavor: 'sinumerik840d' })
    );
  });

  it('refuses an invalid cncFrame', () => {
    assert.throws(() => square().gcode({ flavor: 'siemens', cncFrame: { wcs: 99 } }));
    assert.throws(() => square().gcode({ flavor: 'siemens', cncFrame: { spindle_rpm: 0 } }));
  });
});

describe('Mesh drape from TypeScript', () => {
  const PLANE = 'v 0 0 5\nv 40 0 5\nv 40 40 5\nv 0 40 5\nf 1 2 3\nf 1 3 4\n';

  it('projects a toolpath over a mesh parsed from OBJ text', () => {
    const mesh = parseObjMesh(PLANE);
    const ops = drapeOps({
      mesh,
      pattern: 'raster-x',
      stepover: 5,
      resolution: 2,
      standoff_offset: 0,
      feedrate: 1800,
      plunge_feed: 600,
      width: 0.45,
      height: 0.2,
    });
    assert.ok(ops.length > 10, `expected real motion, got ${ops.length} ops`);
  });

  it('refuses OBJ text with no triangles rather than returning an empty path', () => {
    // A toolpath export: vertices and lines, no faces. Silently draping nothing would look like a
    // successful zero-length program.
    assert.throws(() => parseObjMesh('v 0 0 0\nv 1 0 0\nl 1 2\n'));
  });
});
