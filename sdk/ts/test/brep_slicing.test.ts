import { describe, it } from 'node:test';
import assert from 'node:assert';
import { sliceStepSolid, sliceBrepAssembly, resolveGcode, RESOLVE_PARAMS } from '../src/index';

describe('B-Rep STEP Solid & Multi-Solid Assembly Slicing Suite', () => {
  const stepCylinder = `
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#100 = CYLINDRICAL_SURFACE('', #10, 20.0);
ENDSEC;
END-ISO-10303-21;
`;

  const stepSphere = `
ISO-10303-21;
HEADER;
ENDSEC;
DATA;
#200 = SPHERICAL_SURFACE('', #10, 25.0);
ENDSEC;
END-ISO-10303-21;
`;

  it('slices single STEP CAD solid into continuous L1 ops with 5-axis normals', () => {
    const ops = sliceStepSolid(stepCylinder, 1.0, 5.0, 2.0, 36, 1800.0);
    assert(ops.length > 0);

    const gcode = resolveGcode(ops, RESOLVE_PARAMS);
    assert(gcode.length > 0);
  });

  it('slices multi-solid B-Rep assembly into unified toolpath', () => {
    const ops = sliceBrepAssembly([stepCylinder, stepSphere], 2.0, 6.0, 2.0, 36, 1800.0);
    assert(ops.length > 0);

    const gcode = resolveGcode(ops, RESOLVE_PARAMS);
    assert(gcode.length > 0);
  });
});
