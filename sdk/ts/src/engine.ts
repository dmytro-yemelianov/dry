// Loads the Dry wasm engine (nodejs target, CommonJS) and exposes typed, low-level resolve calls.
// The .wasm + JS glue are built into ../wasm by build.sh. This module is the only place that touches
// the binding; everything else works in terms of typed ops.
import * as path from 'node:path';
import { createRequire } from 'node:module';
import type { Metrics, Op, Report, ResolveParams, Toolpath } from './ops';

interface DryWasm {
  resolve_gcode(
    opsJson: string,
    paramsJson: string,
    relativeE: boolean,
    travelG1E0: boolean,
    fiveAxis: boolean,
    kinematics: string
  ): string[];
  tpms_ops_json(tpmsOptionsJson: string): string;
  resolve_metrics(opsJson: string, paramsJson: string): string;
  resolve_ir(opsJson: string, paramsJson: string): string;
  resolve_optimized_ir(opsJson: string, paramsJson: string): string;
  resolve_verify(
    opsJson: string,
    paramsJson: string,
    maxFlow: number,
    minTemp: number,
    bounds: Float64Array | undefined,
    monotonicZ: boolean,
    speedRange: Float64Array | undefined,
    maxRetractionDistance: number,
    maxRetractionSpeed: number,
    maxTravelWithoutRetract: number,
    firstLayerHeightRange: Float64Array | undefined,
    firstLayerSpeedRange: Float64Array | undefined
  ): string;
}

// compiled to dist/src/engine.js, so the wasm dir is two levels up (dist/src -> dist -> ts/wasm... two
// `..` reach the package root). Resolved relative to this file so it works regardless of cwd.
const requireWasm = createRequire(__filename);
const wasm: DryWasm = requireWasm(path.join(__dirname, '..', '..', 'wasm', 'dry_wasm.js'));

/** Resolve a design and emit motion g-code. */
export function resolveGcode(
  ops: Op[],
  params: ResolveParams,
  relativeE = true,
  travelG1E0 = false,
  fiveAxis = false,
  kinematics = 'ab'
): string[] {
  return wasm.resolve_gcode(
    JSON.stringify(ops),
    JSON.stringify(params),
    relativeE,
    travelG1E0,
    fiveAxis,
    kinematics
  );
}

/**
 * Generate a TPMS infill design's L1 op list in the Rust engine. `optionsJson` is the camelCase
 * `TpmsOptions` wire form; the returned JSON is the `Op[]` list (before resolve/emit). The TS SDK's
 * TPMS generator delegates here so its ops are byte-identical to the native/wasm path (`libm` math).
 */
export function tpmsOps(optionsJson: string): string {
  return wasm.tpms_ops_json(optionsJson);
}

/** Resolve a design and return its simulation metrics. */
export function resolveMetrics(ops: Op[], params: ResolveParams): Metrics {
  return JSON.parse(wasm.resolve_metrics(JSON.stringify(ops), JSON.stringify(params)));
}

/** Resolve a design to the L2 Dry IR. */
export function resolveIr(ops: Op[], params: ResolveParams): Toolpath {
  return JSON.parse(wasm.resolve_ir(JSON.stringify(ops), JSON.stringify(params)));
}

/** Resolve a design through the standard L2 optimization pipeline. */
export function resolveOptimizedIr(ops: Op[], params: ResolveParams): Toolpath {
  return JSON.parse(wasm.resolve_optimized_ir(JSON.stringify(ops), JSON.stringify(params)));
}

/**
 * Resolve a design and verify it against safety contracts. The structured limits cross to the wasm
 * engine as native typed values — `bounds` flat as `[x0,x1,y0,y1,z0,z1]` and each range as `[min,max]`
 * (a `Float64Array`, or `undefined` to disable that check); the scalar ceilings use 0 to mean unset.
 */
export function resolveVerify(
  ops: Op[],
  params: ResolveParams,
  maxFlow = 0,
  minTemp = 0,
  bounds?: Float64Array,
  monotonicZ = false,
  speedRange?: Float64Array,
  maxRetractionDistance = 0,
  maxRetractionSpeed = 0,
  maxTravelWithoutRetract = 0,
  firstLayerHeightRange?: Float64Array,
  firstLayerSpeedRange?: Float64Array
): Report {
  return JSON.parse(
    wasm.resolve_verify(
      JSON.stringify(ops),
      JSON.stringify(params),
      maxFlow,
      minTemp,
      bounds,
      monotonicZ,
      speedRange,
      maxRetractionDistance,
      maxRetractionSpeed,
      maxTravelWithoutRetract,
      firstLayerHeightRange,
      firstLayerSpeedRange
    )
  );
}
