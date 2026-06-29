// Cross-SDK identity (P4): the TS SDK's TPMS generator delegates Op generation to the Rust engine, so
// authoring `tpms(options)` and resolving/emitting through the SDK yields g-code byte-identical to the
// engine's own `resolve_tpms_gcode` (options -> ops -> resolve -> emit, all in Rust). Before the
// delegation the JS `Math` path drifted sub-micron from `libm`; this test guards that they now agree.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as path from 'node:path';
import { createRequire } from 'node:module';
import { tpms, type TpmsOptions, RESOLVE_PARAMS } from '../src/index';

// Load the same nodejs wasm glue the engine binding uses (dist/test -> dist -> package root -> wasm).
interface TpmsWasm {
  resolve_tpms_gcode(
    tpmsOptionsJson: string,
    paramsJson: string,
    relativeE: boolean,
    travelG1E0: boolean,
    fiveAxis: boolean,
    kinematics: string
  ): string[];
}
const requireWasm = createRequire(__filename);
const wasm: TpmsWasm = requireWasm(path.join(__dirname, '..', '..', 'wasm', 'dry_wasm.js'));

// Resolve + emit the TPMS options directly in the engine, with the exact defaults `Design.gcode()` uses
// (generic printer params, relative E, AB kinematics) so the two paths are comparable line-for-line.
function engineGcode(options: TpmsOptions): string[] {
  return wasm.resolve_tpms_gcode(JSON.stringify(options), JSON.stringify(RESOLVE_PARAMS), true, false, false, 'ab');
}

test('TS-delegated TPMS g-code is byte-identical to the Rust engine path', () => {
  // gyroid + a second surface, sliced into real multi-layer geometry.
  const cases: TpmsOptions[] = [
    { surface: 'gyroid', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 10, samplesPerCell: 12, layerHeight: 1.5, minPathLength: 0 },
    { surface: 'schwarz-p', cellsX: 1, cellsY: 1, cellsZ: 1, cellSize: 10, samplesPerCell: 12, layerHeight: 1.5, minPathLength: 0 },
  ];
  for (const options of cases) {
    const tsGcode = tpms(options).gcode();
    const rustGcode = engineGcode(options);
    assert.ok(tsGcode.length > 10, `${options.surface}: expected non-trivial g-code`);
    assert.deepEqual(tsGcode, rustGcode, `${options.surface}: TS vs Rust g-code must be byte-identical`);
  }
});

test('TS-delegated TPMS ops match the engine path with perimeter + adaptive slicing', () => {
  const options: TpmsOptions = {
    surface: 'gyroid',
    cellsX: 1,
    cellsY: 1,
    cellsZ: 1,
    cellSize: 10,
    samplesPerCell: 8,
    layerHeight: 1.2,
    minPathLength: 0,
    perimeter: true,
    adaptive: true,
    adaptiveMinLayerHeight: 0.15,
    adaptiveMaxLayerHeight: 0.3,
  };
  assert.deepEqual(tpms(options).gcode(), engineGcode(options));
});
