// The Dry L1 op vocabulary and the engine's data shapes. The SDK is logic-free: it builds these ops
// and hands them to the wasm engine, which resolves / simulates / emits them. This is the same op
// shape the Python SDK and the conformance oracle use.

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
  // process channels (§3): typed, defaulted, propagated by the engine.
  | { op: 'temperature'; nozzle: number }
  | { op: 'fan'; speed: number }
  | { op: 'flow'; ratio: number }
  | { op: 'tool'; index: number }
  | { op: 'orient'; i: number; j: number; k: number }
  | { op: 'dwell'; seconds: number };

/** The lowering defaults (print/travel feedrate, filament diameter) — mirrors the engine's ResolveParams. */
export interface ResolveParams {
  print_speed: number;
  travel_speed: number;
  dia: number;
}

/** Simulation metrics returned by `simulate`. */
export interface Metrics {
  total_time_s: number;
  print_time_s: number;
  travel_time_s: number;
  extruding_distance: number;
  travel_distance: number;
  extruded_volume: number;
  filament_length: number;
  segment_count: number;
  max_flow_rate: number;
}

/** One resolved L2 motion segment. */
export type SegmentKind = 'line' | 'arc' | 'spline' | 'dwell';

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
  dwell_s?: number;
  orientation?: [number, number, number];
  control_points?: [number, number, number][];
}

/** The resolved L2 Dry IR. */
export interface Toolpath {
  version: number;
  segments: Segment[];
}

/** Device defaults (the generic printer). More profiles land with the device-profile work. */
export const PRINTERS: Record<string, ResolveParams> = {
  generic: { print_speed: 1000, travel_speed: 8000, dia: 1.75 },
};

export const RESOLVE_PARAMS: ResolveParams = PRINTERS.generic;
