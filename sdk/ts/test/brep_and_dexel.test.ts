import assert from 'node:assert/strict';
import test, { describe } from 'node:test';
import {
  sliceBrepAssemblyCsg,
  optimizeConstantMrr,
  simulateDexelStock,
  segmentToSegmentDistance3d,
  RESOLVE_PARAMS,
  Op,
} from '../src/index';

describe('B-Rep CSG, Dexel Simulation & Continuous Distance Suite', () => {
  const outerStep = `
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (0.0, 0.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 25.0);
ENDSEC;
END-ISO-10303-21;
`;

  const voidStep = `
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (0.0, 0.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 10.0);
ENDSEC;
END-ISO-10303-21;
`;

  test('slices B-Rep assembly with CSG boolean void subtraction', () => {
    const ops = sliceBrepAssemblyCsg([outerStep], [voidStep], 2.0, 6.0, 2.0, 36, 1500.0);
    assert(ops.length > 0);
    const hasOrient = ops.some((op) => op.op === 'orient');
    assert.equal(hasOrient, true);
  });

  test('optimizes toolpath for Constant MRR', () => {
    const ops: Op[] = [
      { op: 'geometry', width: 0.5, height: 0.2 },
      { op: 'speed', print: 1000 },
      { op: 'extruder', on: true },
      { op: 'move', x: 0, y: 0, z: 0 },
      { op: 'move', x: 50, y: 0, z: 0 },
    ];
    const tp = optimizeConstantMrr(ops, RESOLVE_PARAMS, 2.0, 800.0, 100.0, 3000.0);
    assert(tp.segments.length > 0);
    const cutSeg = tp.segments.find((s) => !s.travel && s.length > 0);
    assert(cutSeg !== undefined);
    assert(Math.abs(cutSeg.speed - 800.0) < 1e-4);
  });

  test('simulates 3D Dexel stock subtraction', () => {
    const ops: Op[] = [
      { op: 'speed', print: 1200 },
      { op: 'extruder', on: true },
      { op: 'move', x: 10, y: 20, z: 15 },
      { op: 'move', x: 70, y: 20, z: 15 },
    ];
    const report = simulateDexelStock(ops, RESOLVE_PARAMS, [0, 0, 0, 100, 50, 20], 1.0, 5.0, false);
    assert(report.initial_volume_mm3 === 100000);
    assert(report.removed_volume_mm3 > 0);
    assert(report.remaining_volume_mm3 < report.initial_volume_mm3);
  });

  test('calculates 3D segment to segment distance', () => {
    const distParallel = segmentToSegmentDistance3d(
      [0, 0, 0],
      [10, 0, 0],
      [0, 0, 10],
      [10, 0, 10]
    );
    assert(Math.abs(distParallel - 10.0) < 1e-5);

    const distSkew = segmentToSegmentDistance3d(
      [-5, 0, 0],
      [5, 0, 0],
      [0, -5, 5],
      [0, 5, 5]
    );
    assert(Math.abs(distSkew - 5.0) < 1e-5);
  });
});
