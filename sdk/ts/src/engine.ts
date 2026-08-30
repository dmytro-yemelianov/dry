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
    rotaryAxes: string,
    flavor?: string,
    cncFrameJson?: string
  ): string[];
  tpms_ops_json(tpmsOptionsJson: string): string;
  pocket_ops_json(pocketOptionsJson: string): string;
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
  compute_scurve_profile(
    vStart: number,
    vTarget: number,
    maxAcceleration: number,
    maxJerk: number
  ): string;
  import_step_nc_to_ops(stepNcText: string): string;
  generate_lathe_facing_ops_wasm(paramsJson: string): string;
  generate_lathe_od_turning_ops_wasm(paramsJson: string): string;
  check_tool_holder_collision_wasm(
    toolpathJson: string,
    holderJson: string,
    stockBoundsJson: string
  ): string;
  reverse_toolpath_wasm(toolpathJson: string): string;
  verify_gcode_to_report_wasm(gcodeText: string, contractsJson: string): string;
  slice_step_solid_wasm(
    stepContent: string,
    zStart: number,
    zEnd: number,
    layerHeight: number,
    samplesPerSlice: number,
    feedrate: number
  ): string;
  slice_brep_assembly_wasm(
    assemblyJson: string,
    zStart: number,
    zEnd: number,
    layerHeight: number,
    samplesPerSlice: number,
    feedrate: number
  ): string;
  slice_brep_assembly_csg_wasm(
    additivesJson: string,
    voidsJson: string,
    zStart: number,
    zEnd: number,
    layerHeight: number,
    samplesPerSlice: number,
    feedrate: number
  ): string;
  optimize_constant_mrr_wasm(
    opsJson: string,
    paramsJson: string,
    depthOfCut: number,
    targetMrrMm3Min: number,
    minFeedrate: number,
    maxFeedrate: number
  ): string;
  simulate_dexel_stock_wasm(
    opsJson: string,
    paramsJson: string,
    minX: number,
    minY: number,
    minZ: number,
    maxX: number,
    maxY: number,
    maxZ: number,
    resolutionMm: number,
    toolRadius: number,
    isBallnose: boolean
  ): string;
  drape_ops_wasm(optionsJson: string): string;
  parse_obj_mesh_wasm(objText: string): string;
  analyze_machining_physics_wasm(
    toolJson: string,
    material: string,
    paramsJson: string
  ): string;
  optimize_five_axis_lookahead_wasm(toolpathJson: string, paramsJson: string): string;
  segment_to_segment_distance_3d_wasm(
    p1: Float64Array,
    p2: Float64Array,
    q1: Float64Array,
    q2: Float64Array
  ): number;
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
 * Target controller dialects `resolveGcode` can emit. An unknown name is an error, not a silent
 * fall back to Marlin.
 */
export type FirmwareFlavor =
  | 'marlin'
  | 'gcode'
  | 'klipper'
  | 'duet'
  | 'rs274'
  | 'linuxcnc'
  | 'grbl'
  | 'laser'
  | 'krl'
  | 'siemens'
  | 'sinumerik'
  | 'heidenhain'
  | 'tnc'
  | 'haas'
  | 'rapid';

/**
 * Machine preamble for the CNC dialects. Without it those flavors emit motion lines and no work
 * offset, tool change or spindle start (and no `TRAORI` under `fiveAxis`).
 */
export interface CncFrame {
  /** Work coordinate system, 54..=59 → `G54..G59`. */
  wcs?: number;
  /** Tool number for the tool-change line. */
  tool?: number;
  /** Spindle speed in RPM; must be positive. */
  spindle_rpm?: number;
  /** Flood coolant on/off. */
  coolant?: boolean;
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
  rotaryAxes = 'ab',
  flavor?: FirmwareFlavor,
  cncFrame?: CncFrame
): string[] {
  return bind().resolve_gcode(
    JSON.stringify(ops),
    JSON.stringify(params),
    relativeE,
    travelG1E0,
    fiveAxis,
    rotaryAxes,
    flavor,
    cncFrame === undefined ? undefined : JSON.stringify(cncFrame)
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

/**
 * Generate a CNC pocket/profile milling design's L1 op list in the Rust engine.
 * `optionsJson` is the camelCase `PocketOptions` wire form; the returned JSON is the `Op[]` list.
 */
export function pocketOps(optionsJson: string): string {
  return bind().pocket_ops_json(optionsJson);
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

export interface SCurveProfile {
  t_jerk_inc: number;
  t_const_acc: number;
  t_jerk_dec: number;
  total_duration: number;
  total_distance: number;
  peak_acceleration: number;
}

export function computeSCurveProfile(
  vStart: number,
  vTarget: number,
  maxAcceleration: number,
  maxJerk: number
): SCurveProfile {
  return JSON.parse(
    bind().compute_scurve_profile(vStart, vTarget, maxAcceleration, maxJerk)
  );
}

export function importStepNc(stepNcText: string): Op[] {
  return JSON.parse(bind().import_step_nc_to_ops(stepNcText));
}

export function latheFacingOps(params: import('./ops').LatheFacingParams): Op[] {
  return JSON.parse(bind().generate_lathe_facing_ops_wasm(JSON.stringify(params)));
}

export function latheTurningOps(params: import('./ops').LatheTurningParams): Op[] {
  return JSON.parse(bind().generate_lathe_od_turning_ops_wasm(JSON.stringify(params)));
}

export function checkToolHolderCollision(
  toolpath: Toolpath,
  holder: import('./ops').ToolHolder,
  stockBounds: [number, number, number, number, number, number]
): import('./ops').CollisionFinding[] {
  return JSON.parse(
    bind().check_tool_holder_collision_wasm(
      JSON.stringify(toolpath),
      JSON.stringify(holder),
      JSON.stringify(stockBounds)
    )
  );
}

export function reverseToolpath(toolpath: Toolpath): Op[] {
  return JSON.parse(bind().reverse_toolpath_wasm(JSON.stringify(toolpath)));
}

/**
 * Directly verify raw G-code text against safety contracts without server or container infrastructure.
 * Parses G-code, simulates motion, and executes all verification passes in-process via Wasm.
 */
export function verifyGcode(
  gcodeText: string,
  contracts?: Record<string, unknown>
): Report {
  const contractsJson = contracts !== undefined ? JSON.stringify(contracts) : '';
  return JSON.parse(bind().verify_gcode_to_report_wasm(gcodeText, contractsJson));
}

/**
 * Slice an ISO 10303-21 STEP CAD solid directly into L1 ops with analytical surface normals.
 */
export function sliceStepSolid(
  stepContent: string,
  zStart: number = 0.0,
  zEnd: number = 10.0,
  layerHeight: number = 0.2,
  samplesPerSlice: number = 36,
  feedrate: number = 1800.0
): Op[] {
  return JSON.parse(
    bind().slice_step_solid_wasm(
      stepContent,
      zStart,
      zEnd,
      layerHeight,
      samplesPerSlice,
      feedrate
    )
  );
}

/**
 * Slice a multi-solid B-Rep assembly directly into L1 ops with 5-axis surface normals.
 */
export function sliceBrepAssembly(
  stepSolids: string[],
  zStart: number = 0.0,
  zEnd: number = 10.0,
  layerHeight: number = 0.2,
  samplesPerSlice: number = 36,
  feedrate: number = 1800.0
): Op[] {
  return JSON.parse(
    bind().slice_brep_assembly_wasm(
      JSON.stringify(stepSolids),
      zStart,
      zEnd,
      layerHeight,
      samplesPerSlice,
      feedrate
    )
  );
}

/**
 * Slice a multi-solid B-Rep assembly with CSG boolean subtraction of voids from additive solids.
 */
export function sliceBrepAssemblyCsg(
  stepAdditives: string[],
  stepVoids: string[],
  zStart: number = 0.0,
  zEnd: number = 10.0,
  layerHeight: number = 0.2,
  samplesPerSlice: number = 36,
  feedrate: number = 1800.0
): Op[] {
  return JSON.parse(
    bind().slice_brep_assembly_csg_wasm(
      JSON.stringify(stepAdditives),
      JSON.stringify(stepVoids),
      zStart,
      zEnd,
      layerHeight,
      samplesPerSlice,
      feedrate
    )
  );
}

/**
 * Dynamically optimize toolpath feedrate to maintain Constant Material Removal Rate (MRR).
 */
export function optimizeConstantMrr(
  ops: Op[],
  params: Partial<ResolveParams> | undefined,
  depthOfCut: number,
  targetMrrMm3Min: number,
  minFeedrate: number = 100.0,
  maxFeedrate: number = 5000.0
): Toolpath {
  const opsJson = JSON.stringify(ops);
  const paramsJson = params !== undefined ? JSON.stringify(params) : '{}';
  return JSON.parse(
    bind().optimize_constant_mrr_wasm(
      opsJson,
      paramsJson,
      depthOfCut,
      targetMrrMm3Min,
      minFeedrate,
      maxFeedrate
    )
  );
}

export interface DexelSimulationReport {
  initial_volume_mm3: number;
  remaining_volume_mm3: number;
  removed_volume_mm3: number;
  material_removal_ratio: number;
  min_height_mm: number;
  max_height_mm: number;
}

/**
 * Simulate 3D Dexel grid stock subtraction against a toolpath.
 */
export function simulateDexelStock(
  ops: Op[],
  params: Partial<ResolveParams> | undefined,
  stockBounds: [number, number, number, number, number, number],
  resolutionMm: number = 1.0,
  toolRadius: number = 3.0,
  isBallnose: boolean = false
): DexelSimulationReport {
  const opsJson = JSON.stringify(ops);
  const paramsJson = params !== undefined ? JSON.stringify(params) : '{}';
  const [minX, minY, minZ, maxX, maxY, maxZ] = stockBounds;
  return JSON.parse(
    bind().simulate_dexel_stock_wasm(
      opsJson,
      paramsJson,
      minX,
      minY,
      minZ,
      maxX,
      maxY,
      maxZ,
      resolutionMm,
      toolRadius,
      isBallnose
    )
  );
}

/** Toolpath patterns `drapeOps` can project over a mesh. */
export type DrapePattern =
  | 'raster-x'
  | 'raster-y'
  | 'zigzag-x'
  | 'zigzag-y'
  | 'spiral-concentric';

/**
 * Options for {@link drapeOps}. `mesh` is a serialized `TriangleMesh` — use {@link parseObjMesh} to
 * build one from OBJ text.
 */
export interface DrapeOptions {
  mesh: unknown;
  pattern?: DrapePattern;
  x_range?: [number, number] | null;
  y_range?: [number, number] | null;
  stepover: number;
  resolution: number;
  standoff_offset: number;
  safe_z?: number | null;
  feedrate: number;
  plunge_feed: number;
  width: number;
  height: number;
}

/**
 * Project a conformal 5-axis toolpath over a triangle mesh (BVH-accelerated ray casting).
 */
export function drapeOps(options: DrapeOptions): Op[] {
  return JSON.parse(bind().drape_ops_wasm(JSON.stringify(options)));
}

/**
 * Parse OBJ text into the serialized `TriangleMesh` that {@link DrapeOptions.mesh} expects.
 */
export function parseObjMesh(objText: string): unknown {
  return JSON.parse(bind().parse_obj_mesh_wasm(objText));
}

/** Workpiece materials the physics simulator carries coefficients for. */
export type WorkpieceMaterial =
  | 'Aluminum6061'
  | 'Steel4140'
  | 'TitaniumTi6Al4V'
  | 'Inconel718'
  | 'ThermoplasticPLA'
  | 'ThermoplasticPEEK';

export interface CuttingToolGeometry {
  diameter_mm: number;
  flute_count: number;
  stickout_length_mm: number;
  core_diameter_ratio: number;
  modulus_gpa: number;
  corner_radius_mm: number;
}

export interface MachiningOperationParams {
  spindle_rpm: number;
  feedrate_mm_min: number;
  axial_depth_ap_mm: number;
  radial_depth_ae_mm: number;
  ambient_temp_c: number;
}

export interface PhysicsAnalysisReport {
  cutting_speed_m_min: number;
  feed_per_tooth_mm: number;
  material_removal_rate_cm3_min: number;
  tangential_force_n: number;
  spindle_power_kw: number;
  spindle_torque_nm: number;
  tool_deflection_um: number;
  shear_temperature_c: number;
  estimated_tool_life_min: number;
  surface_roughness_ra_um: number;
  chatter_risk: boolean;
  /**
   * True when a clamp bound the result, so `shear_temperature_c` or `estimated_tool_life_min` is a
   * guardrail rather than a computed value. Check it before reading either as a prediction.
   */
  model_saturated: boolean;
}

export interface FiveAxisLookaheadParams {
  max_linear_accel: number;
  max_linear_jerk: number;
  max_rotary_speed_deg_s: number;
  max_rotary_accel_deg_s2: number;
  max_rotary_jerk_deg_s3: number;
}

/**
 * Run the digital-twin machining physics analysis.
 *
 * The estimates are analytic closed-form models with textbook coefficients; nothing in this repo
 * validates them against a dynamometer, a thermocouple or a real cut. Treat them as indicative,
 * not as a process guarantee (`docs/14-known-limitations.md`).
 */
export function analyzeMachiningPhysics(
  tool: CuttingToolGeometry,
  material: WorkpieceMaterial,
  params: MachiningOperationParams
): PhysicsAnalysisReport {
  return JSON.parse(
    bind().analyze_machining_physics_wasm(
      JSON.stringify(tool),
      material,
      JSON.stringify(params)
    )
  );
}

/**
 * Apply the synchronised 5-axis jerk-limited lookahead optimiser to a resolved toolpath.
 */
export function optimizeFiveAxisLookahead(
  toolpath: unknown,
  params: FiveAxisLookaheadParams
): unknown {
  return JSON.parse(
    bind().optimize_five_axis_lookahead_wasm(
      JSON.stringify(toolpath),
      JSON.stringify(params)
    )
  );
}

/**
 * Calculate minimum Euclidean distance between two 3D line segments.
 */
export function segmentToSegmentDistance3d(
  p1: [number, number, number],
  p2: [number, number, number],
  q1: [number, number, number],
  q2: [number, number, number]
): number {
  return bind().segment_to_segment_distance_3d_wasm(
    new Float64Array(p1),
    new Float64Array(p2),
    new Float64Array(q1),
    new Float64Array(q2)
  );
}


