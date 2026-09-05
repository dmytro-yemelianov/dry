// The fluent authoring API. A `Design` is a chain of L1 ops; builders return `this`. The engine calls
// (`gcode`/`simulate`/`ir`) resolve those ops in wasm — the SDK itself holds no toolpath logic.
import type { Metrics, Op, Report, Toolpath } from './ops';
import { PRINTERS } from './ops';
import {
  checkMachineCompatibility,
  resolveBalancedIr,
  resolveBinary,
  resolveGcode,
  resolveIr,
  resolveMetrics,
  resolveOptimizedIr,
  resolveVerify,
} from './engine';
import type { MachineKinematics, FirmwareFlavor, CncFrame } from './engine';
import { pocketOps } from './generators/pocket';
import type { PocketOptions } from './generators/pocket';
import {
  toolpathToInteractiveHtml,
  toolpathToObj,
  toolpathToSvg,
} from './visualizer';

function params(printer: string) {
  const p = PRINTERS[printer];
  if (!p) throw new Error(`unknown printer '${printer}'; known: ${Object.keys(PRINTERS).sort().join(', ')}`);
  return p;
}

/**
 * Normalise build-volume bounds to the flat `[x0,x1,y0,y1,z0,z1]` (mm) the engine expects, or
 * `undefined` when unset. Accepts a structured `[[x0,x1],[y0,y1],[z0,z1]]` or the legacy CSV string.
 */
/**
 * One CSV field as a finite number.
 *
 * `Number` is far more permissive than the Rust parser reading the same documented format: it maps
 * an empty field to 0, `"abc"` to NaN, `"1e400"` to Infinity and `"0x10"` to 16. The first is the
 * dangerous one — `0,100,0,,0,100` became a plausible-looking build volume rather than an error.
 * Require a plain decimal literal and a finite result, so both implementations refuse the same
 * inputs for the same reasons.
 */
const DECIMAL = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

function csvNumber(name: string, token: string, index: number): number {
  const text = token.trim();
  if (text === '') throw new Error(`${name} field ${index + 1} is empty`);
  if (!DECIMAL.test(text)) throw new Error(`${name} field ${index + 1} is not a number: '${text}'`);
  const value = Number(text);
  if (!Number.isFinite(value)) throw new Error(`${name} values must all be finite, got '${text}'`);
  return value;
}

/** Every component of an already-structured input must be finite too. */
function finiteAll(name: string, values: readonly number[]): number[] {
  values.forEach((value, index) => {
    if (!Number.isFinite(value)) {
      throw new Error(`${name} values must all be finite, got ${value} at index ${index}`);
    }
  });
  return [...values];
}

function boundsToFlat(bounds: string | number[][]): Float64Array | undefined {
  if (typeof bounds === 'string') {
    if (bounds.trim() === '') return undefined;
    const parts = bounds.split(',');
    if (parts.length !== 6) throw new Error("bounds CSV must be 'x0,x1,y0,y1,z0,z1'");
    return Float64Array.from(parts.map((token, index) => csvNumber('bounds', token, index)));
  }
  const flat = bounds.flat();
  if (flat.length !== 6) throw new Error('bounds must be [[x0,x1],[y0,y1],[z0,z1]] or a CSV string');
  return Float64Array.from(finiteAll('bounds', flat));
}

/**
 * Normalise a `[min, max]` range to the flat pair the engine expects, or `undefined` when unset.
 * Accepts a structured `[min, max]` or the legacy `"min,max"` CSV string.
 */
function rangeToFlat(name: string, range: string | [number, number]): Float64Array | undefined {
  if (typeof range === 'string') {
    if (range.trim() === '') return undefined;
    const parts = range.split(',');
    if (parts.length !== 2) throw new Error(`${name} CSV must be 'min,max'`);
    return Float64Array.from(parts.map((token, index) => csvNumber(name, token, index)));
  }
  if (range.length !== 2) throw new Error(`${name} must be [min, max] or a CSV string`);
  return Float64Array.from(finiteAll(name, range));
}

/**
 * The machine-safety contracts {@link Design.verify} checks against. Every field is optional; an
 * omitted one leaves its rule disabled.
 *
 * This exists because the positional form does not scale: reaching `firstLayerSpeedRange` means
 * writing nine placeholder arguments first, and miscounting them shifts every later contract
 * silently. The engine binding shipped exactly that bug — a thirteen-argument call made with four.
 */
export interface VerifyOptions {
  /** Named printer profile supplying resolve parameters. Defaults to `'generic'`. */
  printer?: string;
  /** Max volumetric flow, mm³/s. */
  maxFlow?: number;
  /** Minimum nozzle temperature required to extrude, °C. */
  minTemp?: number;
  /** Build volume as `[[x0,x1],[y0,y1],[z0,z1]]` or an `'x0,x1,y0,y1,z0,z1'` CSV string. */
  bounds?: string | number[][];
  /** Require Z to be non-decreasing, as in vase mode. */
  monotonicZ?: boolean;
  /** Allowed extruding feedrate range `[min, max]` in mm/min, or a `'min,max'` CSV string. */
  speedRange?: string | [number, number];
  /** Maximum retraction distance, mm. */
  maxRetractionDistance?: number;
  /** Maximum retraction speed, mm/min. */
  maxRetractionSpeed?: number;
  /** Maximum travel distance permitted without a retraction, mm. */
  maxTravelWithoutRetract?: number;
  /** First-layer height limits `[min, max]` in mm, or a `'min,max'` CSV string. */
  firstLayerHeightRange?: string | [number, number];
  /** First-layer speed limits `[min, max]` in mm/min, or a `'min,max'` CSV string. */
  firstLayerSpeedRange?: string | [number, number];
  /** Machine motion limits. Supplying it enables the peak-acceleration and junction-velocity rules. */
  kinematics?: MachineKinematics;
}

const VERIFY_OPTION_KEYS: readonly (keyof VerifyOptions)[] = [
  'printer',
  'maxFlow',
  'minTemp',
  'bounds',
  'monotonicZ',
  'speedRange',
  'maxRetractionDistance',
  'maxRetractionSpeed',
  'maxTravelWithoutRetract',
  'firstLayerHeightRange',
  'firstLayerSpeedRange',
  'kinematics',
];

/** The options form is anything that is not the legacy leading `printer` string. */
function isVerifyOptions(value: unknown): value is VerifyOptions {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Emission settings for {@link Design.gcode}. */
export interface GcodeOptions {
  /** Named printer profile supplying resolve parameters. Defaults to `'generic'`. */
  printer?: string;
  /** Emit relative extrusion (`M83`). Defaults to `true`. */
  relativeE?: boolean;
  /** Emit travels as `G1 ... E0` rather than `G0`. Defaults to `false`. */
  travelG1E0?: boolean;
  /** Derive rotary words from the toolframe orientation. Defaults to `false`. */
  fiveAxis?: boolean;
  /** Rotary axis pair to emit when `fiveAxis` is set. Defaults to `'ab'`. */
  rotaryAxes?: string;
  /** Target controller dialect. Defaults to `'marlin'`; an unknown name throws. */
  flavor?: FirmwareFlavor;
  /** Machine preamble for the CNC dialects (work offset, tool, spindle, coolant). */
  cncFrame?: CncFrame;
}

const GCODE_OPTION_KEYS: readonly (keyof GcodeOptions)[] = [
  'printer',
  'relativeE',
  'travelG1E0',
  'fiveAxis',
  'rotaryAxes',
  'flavor',
  'cncFrame',
];

function isGcodeOptions(value: unknown): value is GcodeOptions {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Reject a key this API does not have.
 *
 * An options object is only safer than a positional list if a misspelling is an error. `{ maxflow:
 * 5 }` would otherwise leave the flow rule disabled and report nothing, which is the same silent
 * weakening the positional form allows — just spelled differently.
 */
function checkedOptions<T extends object>(
  api: string,
  options: T,
  known: readonly (keyof T)[]
): T {
  const unknown = Object.keys(options).filter(
    (key) => !(known as readonly string[]).includes(key)
  );
  if (unknown.length > 0) {
    throw new Error(
      `unknown ${api} option${unknown.length > 1 ? 's' : ''} ${unknown
        .map((key) => `'${key}'`)
        .join(', ')}; known: ${(known as readonly string[]).join(', ')}`
    );
  }
  return options;
}

function checkedVerifyOptions(options: VerifyOptions): VerifyOptions {
  return checkedOptions('verify', options, VERIFY_OPTION_KEYS);
}

/** Map the deprecated positional argument list onto the same options shape. */
function positionalVerifyOptions(
  printer: string | undefined,
  rest: readonly unknown[]
): VerifyOptions {
  const [
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
    kinematics,
  ] = rest;
  return {
    printer,
    maxFlow: maxFlow as number | undefined,
    minTemp: minTemp as number | undefined,
    bounds: bounds as VerifyOptions['bounds'],
    monotonicZ: monotonicZ as boolean | undefined,
    speedRange: speedRange as VerifyOptions['speedRange'],
    maxRetractionDistance: maxRetractionDistance as number | undefined,
    maxRetractionSpeed: maxRetractionSpeed as number | undefined,
    maxTravelWithoutRetract: maxTravelWithoutRetract as number | undefined,
    firstLayerHeightRange: firstLayerHeightRange as VerifyOptions['firstLayerHeightRange'],
    firstLayerSpeedRange: firstLayerSpeedRange as VerifyOptions['firstLayerSpeedRange'],
    kinematics: kinematics as MachineKinematics | undefined,
  };
}

/** Fluent builder for Dry L1 authoring operations and engine-backed resolution calls. */
export class Design {
  readonly ops: Op[] = [];

  /** Create an L1 design from an existing canonical op list. */
  static fromOps(ops: readonly Op[]): Design {
    const design = new Design();
    design.ops.push(...ops);
    return design;
  }

  /** Set the extrusion bead cross-section (mm). */
  geometry(width: number, height: number): this {
    this.ops.push({ op: 'geometry', width, height });
    return this;
  }

  /** Turn the extruder on/off (off => subsequent moves are travels). */
  extruder(on: boolean): this {
    this.ops.push({ op: 'extruder', on });
    return this;
  }

  /** Set the print feedrate (mm/min). */
  speed(printSpeed: number): this {
    this.ops.push({ op: 'speed', print: printSpeed });
    return this;
  }

  /** Move to a point; an omitted axis is inherited from the running position. */
  point(x: number | null = null, y: number | null = null, z: number | null = null): this {
    this.ops.push({ op: 'move', x, y, z });
    return this;
  }

  /** A circular arc about (cx, cy) to an end point; clockwise => G2, else G3. */
  arc(a: { cx: number; cy: number; x?: number | null; y?: number | null; z?: number | null; clockwise?: boolean }): this {
    this.ops.push({
      op: 'arc',
      cx: a.cx,
      cy: a.cy,
      x: a.x ?? null,
      y: a.y ?? null,
      z: a.z ?? null,
      clockwise: a.clockwise ?? false,
    });
    return this;
  }

  /** A Catmull-Rom spline from the running position through each (x, y, z) control point. */
  spline(points: [number | null, number | null, number | null][]): this {
    this.ops.push({ op: 'spline', points: points.map((p) => [p[0] ?? null, p[1] ?? null, p[2] ?? null]) });
    return this;
  }

  /**
   * A clothoid (Euler-spiral) corner blend around construction corner `(corner_x, corner_y)`,
   * consuming `blend` mm of tangent length from each leg on the way to `(x, y, z)`.
   */
  clothoid(a: {
    corner_x: number;
    corner_y: number;
    blend: number;
    x?: number | null;
    y?: number | null;
    z?: number | null;
  }): this {
    this.ops.push({
      op: 'clothoid',
      corner_x: a.corner_x,
      corner_y: a.corner_y,
      blend: a.blend,
      x: a.x ?? null,
      y: a.y ?? null,
      z: a.z ?? null,
    });
    return this;
  }

  // ---- process channels (§3) ----

  /** Set the nozzle temperature channel (°C). */
  temperature(nozzle: number): this {
    this.ops.push({ op: 'temperature', nozzle });
    return this;
  }

  /** Set the part-cooling fan channel (0..1). */
  fan(speed: number): this {
    this.ops.push({ op: 'fan', speed });
    return this;
  }

  /** Set the flow multiplier channel (scales deposited volume; default 1.0). */
  flow(ratio: number): this {
    this.ops.push({ op: 'flow', ratio });
    return this;
  }

  /** Set the active tool channel. */
  tool(index: number): this {
    this.ops.push({ op: 'tool', index });
    return this;
  }

  /**
   * Set the spindle/laser power channel, in the target controller's `S`-word units (RPM for a
   * spindle, PWM counts for a laser). `0` commands it off — distinct from never setting it. Only
   * the `grbl` flavor renders the channel; the others refuse a toolpath that carries it.
   *
   * NOTE: `gcode()` here always emits with the default (Marlin) flavor, so it *refuses* a design
   * carrying this channel. The channel still reaches `ir()` / `optimizedIr()` / `verify()`; to
   * render it, emit through the CLI (`dry emit --format grbl`) or a `grbl` profile.
   */
  power(level: number): this {
    this.ops.push({ op: 'power', level });
    return this;
  }

  /** Set the toolframe orientation: the tool-direction vector (i, j, k). Identity is +Z. */
  orient(i: number, j: number, k: number): this {
    this.ops.push({ op: 'orient', i, j, k });
    return this;
  }

  /** Pause in place for `seconds` (emits a `G4` dwell). */
  dwell(seconds: number): this {
    this.ops.push({ op: 'dwell', seconds });
    return this;
  }

  /** Inject verbatim custom G-code. */
  manualGcode(text: string): this {
    this.ops.push({ op: 'manual_gcode', text });
    return this;
  }

  /** Retract filament. */
  retract(distance: number | null = null, speed: number | null = null): this {
    this.ops.push({ op: 'retract', distance, speed });
    return this;
  }

  /** Prime filament back after a retraction. */
  unretract(distance: number | null = null, speed: number | null = null): this {
    this.ops.push({ op: 'unretract', distance, speed });
    return this;
  }

  /** Stationary extrusion of a set volume (mm³) at feedrate (mm/min). */
  deposit(volume: number, speed: number): this {
    this.ops.push({ op: 'deposit', volume, speed });
    return this;
  }

  /** Append CNC pocket/profile milling ops generated from options. */
  pocket(options: PocketOptions): this {
    this.ops.push(...pocketOps(options));
    return this;
  }

  // ---- engine calls ----

  /**
   * Resolve + emit motion g-code (an array of lines). `rotaryAxes` is the rotary-axes selector (the
   * ab/ac/bc STRING) choosing which two rotary axes carry the toolframe orientation in 5-axis emit —
   * distinct from the machine motion-limits `kinematics` object used by `balancedIr` / `verify`.
   */
  gcode(options?: GcodeOptions): string[];
  /**
   * @deprecated Pass a {@link GcodeOptions} object instead. Three of these arguments are
   * consecutive booleans, so transposing two of them type-checks cleanly and changes the emitted
   * program.
   */
  gcode(
    printer?: string,
    relativeE?: boolean,
    travelG1E0?: boolean,
    fiveAxis?: boolean,
    rotaryAxes?: string
  ): string[];
  gcode(first?: GcodeOptions | string, ...rest: readonly unknown[]): string[] {
    const o = isGcodeOptions(first)
      ? checkedOptions('gcode', first, GCODE_OPTION_KEYS)
      : {
          printer: first,
          relativeE: rest[0] as boolean | undefined,
          travelG1E0: rest[1] as boolean | undefined,
          fiveAxis: rest[2] as boolean | undefined,
          rotaryAxes: rest[3] as string | undefined,
        };

    return resolveGcode(
      this.ops,
      params(o.printer ?? 'generic'),
      o.relativeE ?? true,
      o.travelG1E0 ?? false,
      o.fiveAxis ?? false,
      o.rotaryAxes ?? 'ab',
      o.flavor,
      o.cncFrame
    );
  }

  /** Resolve + simulate; returns metrics (time, distances, material, peak flow). */
  simulate(printer = 'generic'): Metrics {
    return resolveMetrics(this.ops, params(printer));
  }

  /** Resolve to the L2 Dry IR ({ version, segments }). */
  ir(printer = 'generic'): Toolpath {
    return resolveIr(this.ops, params(printer));
  }

  /** Resolve through the standard L2 optimization pipeline. */
  optimizedIr(printer = 'generic'): Toolpath {
    return resolveOptimizedIr(this.ops, params(printer));
  }

  /**
   * Resolve through the kinematics-aware balanced optimization pipeline. When `kinematics` is
   * provided its acceleration/junction-velocity limits shape the output (arc centripetal speed
   * clamping + junction-velocity capping) on top of all standard optimizations. Omitting
   * `kinematics` falls back to the safe pipeline (same as `optimizedIr`).
   */
  balancedIr(printer = 'generic', kinematics?: MachineKinematics): Toolpath {
    return resolveBalancedIr(this.ops, params(printer), kinematics);
  }

  /** Resolve + encode to the binary DRY1 format; returns the raw bytes. */
  binary(printer = 'generic'): Uint8Array {
    return resolveBinary(this.ops, params(printer));
  }

  /**
   * Resolve + verify against machine-safety contracts; returns the safety report findings. The
   * structured limits cross to the engine as native typed contracts (no CSV round-trip):
   *
   *  - `bounds` — build volume as `[[x0,x1],[y0,y1],[z0,z1]]` (mm). The legacy CSV string
   *    `"x0,x1,y0,y1,z0,z1"` is still accepted for backward compatibility.
   *  - `speedRange` — extruding feedrate `[min, max]` (mm/min); the legacy `"min,max"` CSV is accepted.
   *  - `maxFlow` (mm³/s), `minTemp` (°C), `monotonicZ` (bool); 0 means unset for the scalar ceilings.
   *  - `maxRetractionDistance` (mm), `maxRetractionSpeed` (mm/min), `maxTravelWithoutRetract` (mm) —
   *    retraction / stringing limits.
   *  - `firstLayerHeightRange`, `firstLayerSpeedRange` — first-layer adhesion limits, each `[min, max]`
   *    (or a `"min,max"` CSV string).
   *  - `kinematics` — machine motion limits (`max_acceleration_mm_s2` and/or
   *    `max_junction_velocity_mm_s`). When supplied, enables the `peak-acceleration` and
   *    `junction-velocity` verify rules; omitting it disables them.
   */
  verify(options?: VerifyOptions): Report;
  /**
   * @deprecated Pass a {@link VerifyOptions} object instead. Reaching the tenth contract
   * positionally means writing out nine placeholders first, and a miscount silently shifts every
   * argument after it — this surface has already shipped one such bug.
   */
  verify(
    printer?: string,
    maxFlow?: number,
    minTemp?: number,
    bounds?: string | number[][],
    monotonicZ?: boolean,
    speedRange?: string | [number, number],
    maxRetractionDistance?: number,
    maxRetractionSpeed?: number,
    maxTravelWithoutRetract?: number,
    firstLayerHeightRange?: string | [number, number],
    firstLayerSpeedRange?: string | [number, number],
    kinematics?: MachineKinematics
  ): Report;
  verify(first?: VerifyOptions | string, ...rest: readonly unknown[]): Report {
    const o = isVerifyOptions(first)
      ? checkedVerifyOptions(first)
      : positionalVerifyOptions(first, rest);

    return resolveVerify(
      this.ops,
      params(o.printer ?? 'generic'),
      o.maxFlow ?? 0,
      o.minTemp ?? 0,
      boundsToFlat(o.bounds ?? ''),
      o.monotonicZ ?? false,
      rangeToFlat('speedRange', o.speedRange ?? ''),
      o.maxRetractionDistance ?? 0,
      o.maxRetractionSpeed ?? 0,
      o.maxTravelWithoutRetract ?? 0,
      rangeToFlat('firstLayerHeightRange', o.firstLayerHeightRange ?? ''),
      rangeToFlat('firstLayerSpeedRange', o.firstLayerSpeedRange ?? ''),
      o.kinematics
    );
  }

  /**
   * Pre-flight check toolpath against machine capabilities (D2.2).
   *
   * The rules live in the engine (`dry_core::check_compatibility`), not here. This method
   * previously carried its own copy of the loop, implementing five of the engine's seven rule
   * codes; it omitted `ARC_OUT_OF_BOUNDS_X` and `ARC_OUT_OF_BOUNDS_Y`, so an arc whose swept circle
   * leaves the build envelope was reported compatible here and refused by the engine. The engine
   * bounds an arc by its full circle deliberately — refusing a safe program is recoverable, passing
   * an unsafe one is not.
   *
   * `capabilities` keeps the SDK's camelCase shape; it is adapted to the engine's wire form here.
   */
  checkCompatibility(capabilities: MachineCapabilities, printer = 'generic'): CompatibilityReport {
    const engineCaps: Record<string, unknown> = {
      name: capabilities.name ?? 'unnamed',
      x_range: { min: capabilities.xRange.min, max: capabilities.xRange.max },
      y_range: { min: capabilities.yRange.min, max: capabilities.yRange.max },
      z_range: { min: capabilities.zRange.min, max: capabilities.zRange.max },
    };
    if (capabilities.maxFeedrate !== undefined) {
      engineCaps.max_feedrate_mm_min = capabilities.maxFeedrate;
    }
    if (capabilities.maxSpindleRpm !== undefined) {
      engineCaps.max_spindle_rpm = capabilities.maxSpindleRpm;
    }

    const raw = checkMachineCompatibility(this.ops, params(printer), JSON.stringify(engineCaps));
    const findings: CompatibilityFinding[] = raw.findings.map((f) => ({
      severity: f.severity,
      code: f.code,
      message: f.message,
      segmentIndex: f.segment_index,
    }));
    return { compatible: raw.compatible, findings };
  }

  /** Export toolpath as a 3D Wavefront .obj mesh string. */
  toObj(includeTravel = false, printer = 'generic'): string {
    return toolpathToObj(this.ir(printer), includeTravel);
  }

  /** Export toolpath as a 2D (XY) vector SVG projection string. */
  toSvg(width = 800, height = 800, padding = 40.0, printer = 'generic'): string {
    return toolpathToSvg(this.ir(printer), width, height, padding);
  }

  /** Export toolpath as a standalone interactive 3D WebGL HTML viewer string. */
  toHtml(
    title = 'Dry 3D Toolpath Viewer',
    bounds?: [number, number, number, number, number, number],
    printer = 'generic'
  ): string {
    return toolpathToInteractiveHtml(this.ir(printer), title, bounds);
  }
}

export interface AxisRange {
  min: number;
  max: number;
}

export interface MachineCapabilities {
  name?: string;
  xRange: AxisRange;
  yRange: AxisRange;
  zRange: AxisRange;
  maxFeedrate?: number;
  maxSpindleRpm?: number;
}

export interface CompatibilityFinding {
  severity: 'Warning' | 'Error';
  code: string;
  message: string;
  segmentIndex?: number;
}

export interface CompatibilityReport {
  compatible: boolean;
  findings: CompatibilityFinding[];
}
