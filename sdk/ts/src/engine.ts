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
  resolve_metrics(opsJson: string, paramsJson: string): string;
  resolve_ir(opsJson: string, paramsJson: string): string;
  resolve_optimized_ir(opsJson: string, paramsJson: string): string;
  resolve_verify(
    opsJson: string,
    paramsJson: string,
    maxFlow: number,
    minTemp: number,
    bounds: string,
    monotonicZ: boolean,
    speedRange: string
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

/** Resolve a design and verify it against safety contracts. */
export function resolveVerify(
  ops: Op[],
  params: ResolveParams,
  maxFlow = 0,
  minTemp = 0,
  bounds = '',
  monotonicZ = false,
  speedRange = ''
): Report {
  return JSON.parse(
    wasm.resolve_verify(
      JSON.stringify(ops),
      JSON.stringify(params),
      maxFlow,
      minTemp,
      bounds,
      monotonicZ,
      speedRange
    )
  );
}
