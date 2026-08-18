// The fluent authoring API. A `Design` is a chain of L1 ops; builders return `this`. The engine calls
// (`gcode`/`simulate`/`ir`) resolve those ops in wasm — the SDK itself holds no toolpath logic.
import type { Metrics, Op, Report, Toolpath } from './ops';
import { PRINTERS } from './ops';
import {
  resolveBalancedIr,
  resolveBinary,
  resolveGcode,
  resolveIr,
  resolveMetrics,
  resolveOptimizedIr,
  resolveVerify,
} from './engine';
import type { MachineKinematics } from './engine';

function params(printer: string) {
  const p = PRINTERS[printer];
  if (!p) throw new Error(`unknown printer '${printer}'; known: ${Object.keys(PRINTERS).sort().join(', ')}`);
  return p;
}

/**
 * Normalise build-volume bounds to the flat `[x0,x1,y0,y1,z0,z1]` (mm) the engine expects, or
 * `undefined` when unset. Accepts a structured `[[x0,x1],[y0,y1],[z0,z1]]` or the legacy CSV string.
 */
function boundsToFlat(bounds: string | number[][]): Float64Array | undefined {
  if (typeof bounds === 'string') {
    if (bounds.trim() === '') return undefined;
    const flat = bounds.split(',').map(Number);
    if (flat.length !== 6) throw new Error("bounds CSV must be 'x0,x1,y0,y1,z0,z1'");
    return Float64Array.from(flat);
  }
  const flat = bounds.flat();
  if (flat.length !== 6) throw new Error('bounds must be [[x0,x1],[y0,y1],[z0,z1]] or a CSV string');
  return Float64Array.from(flat);
}

/**
 * Normalise a `[min, max]` range to the flat pair the engine expects, or `undefined` when unset.
 * Accepts a structured `[min, max]` or the legacy `"min,max"` CSV string.
 */
function rangeToFlat(name: string, range: string | [number, number]): Float64Array | undefined {
  if (typeof range === 'string') {
    if (range.trim() === '') return undefined;
    const flat = range.split(',').map(Number);
    if (flat.length !== 2) throw new Error(`${name} CSV must be 'min,max'`);
    return Float64Array.from(flat);
  }
  if (range.length !== 2) throw new Error(`${name} must be [min, max] or a CSV string`);
  return Float64Array.from(range);
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

  // ---- engine calls ----

  /**
   * Resolve + emit motion g-code (an array of lines). `rotaryAxes` is the rotary-axes selector (the
   * ab/ac/bc STRING) choosing which two rotary axes carry the toolframe orientation in 5-axis emit —
   * distinct from the machine motion-limits `kinematics` object used by `balancedIr` / `verify`.
   */
  gcode(
    printer = 'generic',
    relativeE = true,
    travelG1E0 = false,
    fiveAxis = false,
    rotaryAxes = 'ab'
  ): string[] {
    return resolveGcode(this.ops, params(printer), relativeE, travelG1E0, fiveAxis, rotaryAxes);
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
  verify(
    printer = 'generic',
    maxFlow = 0,
    minTemp = 0,
    bounds: string | number[][] = '',
    monotonicZ = false,
    speedRange: string | [number, number] = '',
    maxRetractionDistance = 0,
    maxRetractionSpeed = 0,
    maxTravelWithoutRetract = 0,
    firstLayerHeightRange: string | [number, number] = '',
    firstLayerSpeedRange: string | [number, number] = '',
    kinematics?: MachineKinematics
  ): Report {
    return resolveVerify(
      this.ops,
      params(printer),
      maxFlow,
      minTemp,
      boundsToFlat(bounds),
      monotonicZ,
      rangeToFlat('speedRange', speedRange),
      maxRetractionDistance,
      maxRetractionSpeed,
      maxTravelWithoutRetract,
      rangeToFlat('firstLayerHeightRange', firstLayerHeightRange),
      rangeToFlat('firstLayerSpeedRange', firstLayerSpeedRange),
      kinematics
    );
  }
}
