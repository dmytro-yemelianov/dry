// The Dry L1 op vocabulary and the engine's data shapes. The SDK is logic-free: it builds these ops
// and hands them to the wasm engine, which resolves / simulates / emits them. This is the same op
// shape the Python SDK and the conformance oracle use.

/** Authoring operation in Dry L1, before resolution into concrete toolpath segments. */
export type Op =
  | { op: 'geometry'; width: number; height: number }
  | { op: 'extruder'; on: boolean }
  | { op: 'speed'; print: number }
  | { op: 'move'; x: number | null; y: number | null; z: number | null }
  | {
      op: 'arc';
      cx: number;
      cy: number;
      x: number | null;
      y: number | null;
      z: number | null;
      clockwise: boolean;
    }
  | { op: 'spline'; points: [number | null, number | null, number | null][] }
  | {
      op: 'clothoid';
      corner_x: number;
      corner_y: number;
      x: number | null;
      y: number | null;
      z: number | null;
      blend: number;
    }
  // process channels (§3): typed, defaulted, propagated by the engine.
  | { op: 'temperature'; nozzle: number }
  | { op: 'fan'; speed: number }
  | { op: 'flow'; ratio: number }
  | { op: 'tool'; index: number }
  /** Spindle/laser power in the target controller's `S`-word units (RPM for a spindle, PWM counts
   * for a laser). Must be finite and >= 0; `0` means commanded off, which is distinct from never
   * commanding the channel at all. Rendered by the `grbl` flavor; any other flavor refuses a
   * toolpath that carries it rather than dropping the command silently. */
  | { op: 'power'; level: number }
  | { op: 'orient'; i: number; j: number; k: number }
  | { op: 'dwell'; seconds: number }
  | { op: 'manual_gcode'; text: string }
  | { op: 'retract'; distance: number | null; speed: number | null }
  | { op: 'unretract'; distance: number | null; speed: number | null }
  | { op: 'deposit'; volume: number; speed: number };

/** The lowering defaults (print/travel feedrate, filament diameter, retraction) — mirrors the engine's ResolveParams. */
export interface ResolveParams {
  /** Default extrusion feedrate in mm/min. */
  print_speed: number;
  /** Default travel feedrate in mm/min. */
  travel_speed: number;
  /** Filament diameter in mm. */
  dia: number;
  /** Default retraction speed in mm/min. */
  retraction_speed?: number;
  /** Default retraction distance in mm. */
  retraction_distance?: number;
}

/** Simulation metrics returned by `simulate`. */
export interface Metrics {
  /** Total estimated time in seconds. */
  total_time_s: number;
  /** Estimated extruding move time in seconds. */
  print_time_s: number;
  /** Estimated travel move time in seconds. */
  travel_time_s: number;
  /** Total extruding path length in mm. */
  extruding_distance: number;
  /** Total non-extruding path length in mm. */
  travel_distance: number;
  /** Deposited material volume in cubic mm. */
  extruded_volume: number;
  /** Filament consumed in mm. */
  filament_length: number;
  /** Number of resolved toolpath segments. */
  segment_count: number;
  /** Maximum observed flow rate in cubic mm/s. */
  max_flow_rate: number;
}

/** One resolved L2 motion segment. */
export type SegmentKind =
  | 'line'
  | 'arc'
  | 'spline'
  | 'dwell'
  | 'retract'
  | 'unretract'
  | 'deposit'
  | 'manual_gcode';

/** One resolved motion or process segment in the Dry L2 IR. */
export interface Segment {
  start: (number | null)[];
  end: (number | null)[];
  travel: boolean;
  speed: number;
  length: number;
  volume: number;
  filament: number;
  width: number | null;
  height: number | null;
  kind: SegmentKind;
  centre: [number, number] | null;
  clockwise: boolean;
  // process channels — present only when set (omitted from the IR otherwise).
  temperature?: number;
  fan?: number;
  flow?: number;
  tool?: number;
  /** Commanded spindle/laser power (`S`-word units); `0` is commanded off, absent is never commanded. */
  power?: number;
  dwell_s?: number;
  manual_gcode?: string;
  orientation?: [number, number, number];
  control_points?: [number, number, number][];
}

/** Optional provenance and invariant metadata attached to a resolved toolpath. */
export interface ToolpathMeta {
  /** Name of the generator or pipeline that produced the toolpath. */
  generator?: string;
  /** Coordinate and unit convention, normally millimeters. */
  units?: string;
  /** Stable source hash when the toolpath was derived from an external artifact. */
  source_hash?: string;
  /** Human-readable invariants the toolpath is expected to satisfy. */
  invariants?: string[];
}

/** The resolved L2 Dry IR. */
export interface Toolpath {
  version: number;
  meta?: ToolpathMeta;
  segments: Segment[];
}

/** Severity level for a verification finding. */
export type Severity = 'error' | 'warning';

/** Single verification finding, optionally tied to a resolved segment index. */
export interface Finding {
  /** Stable rule identifier, such as `bounds` or `peak-acceleration`. */
  rule: string;
  /** Whether the finding blocks the contract or is advisory. */
  severity: Severity;
  /** Zero-based segment index, or null when the finding is global. */
  segment: number | null;
  /** Human-readable finding details. */
  message: string;
}

/** Verification result containing all findings emitted by enabled rules. */
export interface Report {
  findings: Finding[];
  /**
   * How many segments the pass actually inspected. Zero means it proved nothing — an empty
   * `findings` array is equally true of a clean program and of one that was never looked at.
   *
   * Optional so reports produced by engines older than Dry 0.5 stay assignable to this type.
   */
  segments_inspected?: number;
  /** The rule ids that were in force. "Clean under 11 rules" is a weaker claim than under 27. */
  rules_evaluated?: string[];
  /** The limits the toolpath was checked against. */
  contracts?: Record<string, unknown>;
  /**
   * The passive license stamp embedded by report-producing commands (parity with
   * `dry_core::verify::Report`'s `license` field / `dry_core::LicenseStamp`). Optional so
   * reports produced by engines that predate the license product stay assignable to this type.
   */
  license?: { mode: string; licensee?: string; tier?: string };
}

/** Parameters for CNC Lathe Facing operation. */
export interface LatheFacingParams {
  stock_diameter: number;
  target_z?: number;
  clearance_x?: number;
  clearance_z?: number;
  feedrate?: number;
  spindle_rpm?: number;
  passes?: number;
  depth_per_pass?: number;
}

/** Parameters for CNC Lathe Outer Diameter (OD) Roughing & Finishing. */
export interface LatheTurningParams {
  raw_diameter: number;
  target_diameter: number;
  cut_length: number;
  depth_of_cut?: number;
  finish_allowance?: number;
  clearance_x?: number;
  clearance_z?: number;
  rough_feedrate?: number;
  finish_feedrate?: number;
  spindle_rpm?: number;
}

/** A defined axial segment along a stepped/tapered tool holder assembly. */
export interface ToolHolderSection {
  diameter: number;
  length: number;
}

/** Physical dimensions of a tool holder assembly for collision detection. */
export interface ToolHolder {
  holder_diameter: number;
  stickout_length: number;
  collet_diameter?: number;
  collet_length?: number;
  sections?: ToolHolderSection[];
}

/** Collision finding for tool holder interference. */
export interface CollisionFinding {
  severity: Severity;
  code: string;
  message: string;
  segment_index: number;
  plunge_depth: number;
}

/** Device defaults (the generic printer). More profiles land with the device-profile work. */
export const PRINTERS: Record<string, ResolveParams> = {
  generic: { print_speed: 1000, travel_speed: 8000, dia: 1.75 },
};

/** Default resolver parameters for the generic built-in printer profile. */
export const RESOLVE_PARAMS: ResolveParams = PRINTERS.generic;

