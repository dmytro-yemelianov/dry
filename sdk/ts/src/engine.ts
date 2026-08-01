// Binding-agnostic Dry wasm engine wrapper: exposes typed, low-level resolve calls over whichever
// wasm binding a platform loader installs (engine.node.ts on Node, engine.web.ts in the browser).
// This module is the only place that touches the binding; everything else works in terms of typed ops.
import type { FeatureProgramDocument } from './features';
import type { Metrics, Op, Report, ResolveParams, Toolpath } from './ops';

/**
 * Machine kinematic limits used by `resolveBalancedIr` and `resolveVerify`. Field names are
 * snake_case to match the Rust serde serialization. All fields are optional; an unset field
 * disables the corresponding check.
 *
 *  - `max_acceleration_mm_s2` — peak centripetal acceleration ceiling (mm/s²).
 *  - `max_junction_velocity_mm_s` — per-junction speed-change ceiling (mm/s).
 */
export interface MachineKinematics {
  max_acceleration_mm_s2?: number;
  max_junction_velocity_mm_s?: number;
}

export interface DryWasm {
  expand_features(programJson: string): string;
  resolve_gcode(
    opsJson: string,
    paramsJson: string,
    relativeE: boolean,
    travelG1E0: boolean,
    fiveAxis: boolean,
    rotaryAxes: string
  ): string[];
  tpms_ops_json(tpmsOptionsJson: string): string;
  resolve_metrics(opsJson: string, paramsJson: string): string;
  metrics_ir(irJson: string): string;
  resolve_ir(opsJson: string, paramsJson: string): string;
  resolve_binary(opsJson: string, paramsJson: string): Uint8Array;
  resolve_optimized_ir(opsJson: string, paramsJson: string): string;
  resolve_balanced_ir(opsJson: string, paramsJson: string, kinematicsJson: string): string;
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
    firstLayerSpeedRange: Float64Array | undefined,
    kinematicsJson: string
  ): string;
}

// The wasm binding is injected by a platform loader (engine.node.ts on Node, engine.web.ts in the
// browser). Keeping engine.ts binding-agnostic is what lets the same Design API run client-side.
let wasm: DryWasm | undefined;

/** Install the resolved wasm binding. Called once by a platform loader before any resolve call. */
export function setWasmBinding(binding: DryWasm): void {
  wasm = binding;
}

function bind(): DryWasm {
  if (!wasm) {
    throw new Error(
      'Dry wasm engine not initialised: import the Node entry (@dry/sdk) or call initDryWeb() first'
    );
  }
  return wasm;
}

/** Expand a bounded L0 feature graph into the canonical L1 op list. */
export function expandFeatures(program: FeatureProgramDocument): Op[] {
  return JSON.parse(bind().expand_features(JSON.stringify(program)));
}

/**
 * Resolve a design and emit motion g-code. `rotaryAxes` is the rotary-axes selector (the ab/ac/bc
 * STRING) choosing which two rotary axes carry the toolframe orientation in 5-axis emit — distinct
 * from the machine motion-limits `MachineKinematics` object used by `resolveBalancedIr` /
 * `resolveVerify`.
 */
export function resolveGcode(
  ops: Op[],
  params: ResolveParams,
  relativeE = true,
  travelG1E0 = false,
  fiveAxis = false,
  rotaryAxes = 'ab'
): string[] {
  return bind().resolve_gcode(
    JSON.stringify(ops),
    JSON.stringify(params),
    relativeE,
    travelG1E0,
    fiveAxis,
    rotaryAxes
  );
}

/**
 * Generate a TPMS infill design's L1 op list in the Rust engine. `optionsJson` is the camelCase
 * `TpmsOptions` wire form; the returned JSON is the `Op[]` list (before resolve/emit). The TS SDK's
 * TPMS generator delegates here so its ops are byte-identical to the native/wasm path (`libm` math).
 */
export function tpmsOps(optionsJson: string): string {
  return bind().tpms_ops_json(optionsJson);
}

/** Resolve a design and return its simulation metrics. */
export function resolveMetrics(ops: Op[], params: ResolveParams): Metrics {
  return JSON.parse(bind().resolve_metrics(JSON.stringify(ops), JSON.stringify(params)));
}

/**
 * Simulate an already-resolved Dry IR (`{ version, segments }`) and return its metrics. Unlike
 * `resolveMetrics`, which simulates an L1 design, this takes a toolpath IR directly — so a caller can
 * report the before/after time and peak flow of an optimized or balanced IR (which has no originating
 * op-list). `irJson` is the JSON string of a `Toolpath` (e.g. the result of `JSON.stringify` on an
 * `optimizedIr`/`balancedIr` toolpath).
 */
export function resolveMetricsIr(irJson: string): Metrics {
  return JSON.parse(bind().metrics_ir(irJson));
}

/** Resolve a design to the L2 Dry IR. */
export function resolveIr(ops: Op[], params: ResolveParams): Toolpath {
  return JSON.parse(bind().resolve_ir(JSON.stringify(ops), JSON.stringify(params)));
}

/** Resolve a design and return the L2 Dry IR encoded as the binary DRY1 format (raw bytes). */
export function resolveBinary(ops: Op[], params: ResolveParams): Uint8Array {
  return bind().resolve_binary(JSON.stringify(ops), JSON.stringify(params));
}

/** Resolve a design through the standard L2 optimization pipeline. */
export function resolveOptimizedIr(ops: Op[], params: ResolveParams): Toolpath {
  return JSON.parse(bind().resolve_optimized_ir(JSON.stringify(ops), JSON.stringify(params)));
}

/**
 * Resolve a design through the kinematics-aware balanced optimization pipeline. When `kinematics`
 * is provided its acceleration/junction-velocity limits shape the output (acceleration clamping +
 * junction-velocity capping). Omitting `kinematics` falls back to the safe pipeline (same as
 * `resolveOptimizedIr`).
 */
export function resolveBalancedIr(
  ops: Op[],
  params: ResolveParams,
  kinematics?: MachineKinematics
): Toolpath {
  const kinematicsJson = kinematics !== undefined ? JSON.stringify(kinematics) : '';
  return JSON.parse(
    bind().resolve_balanced_ir(JSON.stringify(ops), JSON.stringify(params), kinematicsJson)
  );
}

/**
 * Resolve a design and verify it against safety contracts. The structured limits cross to the wasm
 * engine as native typed values — `bounds` flat as `[x0,x1,y0,y1,z0,z1]` and each range as `[min,max]`
 * (a `Float64Array`, or `undefined` to disable that check); the scalar ceilings use 0 to mean unset.
 * The optional `kinematics` arg enables the `peak-acceleration` and `junction-velocity` verify rules.
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
  firstLayerSpeedRange?: Float64Array,
  kinematics?: MachineKinematics
): Report {
  const kinematicsJson = kinematics !== undefined ? JSON.stringify(kinematics) : '';
  return JSON.parse(
    bind().resolve_verify(
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
      firstLayerSpeedRange,
      kinematicsJson
    )
  );
}
