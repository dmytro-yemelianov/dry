export type Op =
  | { op: 'geometry'; width: number; height: number }
  | { op: 'extruder'; on: boolean }
  | { op: 'speed'; print: number }
  | { op: 'move'; x?: number | null; y?: number | null; z?: number | null }
  | { op: 'arc'; cx: number; cy: number; x?: number | null; y?: number | null; z?: number | null; clockwise: boolean }
  | { op: 'temperature'; nozzle?: number; bed?: number }
  | { op: 'fan'; speed: number }
  | { op: 'retract'; distance: number; speed: number }
  | { op: 'unretract'; distance: number; speed: number }
  | { op: 'spline'; points: [number, number, number][] }
  | { op: 'dwell'; seconds: number }
  | { op: 'spindle'; speed: number; clockwise?: boolean }
  | { op: 'laser'; power: number };

export interface ResolveParams {
  print_speed: number;
  travel_speed: number;
  dia: number;
  retraction_speed?: number;
  retraction_distance?: number;
}

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

export interface Segment {
  start?: [number, number, number];
  end?: [number, number, number];
  kind?: string;
  extruder_on?: boolean;
  speed?: number;
}

export interface Toolpath {
  segments: Segment[];
  header?: Record<string, unknown>;
}

export interface MachineProfile {
  id: string;
  name: string;
  manufacturer: string;
  build_volume: {
    x: [number, number];
    y: [number, number];
    z: [number, number];
  };
  max_feedrates: {
    x: number;
    y: number;
    z: number;
    e: number;
  };
  max_acceleration: number;
  firmware: string;
}

export interface ParameterDef {
  id: string;
  label: string;
  defaultValue: number;
  min: number;
  max: number;
  step: number;
  unit: string;
  title?: string;
}

export interface DesignDef {
  key: string;
  label: string;
  group: string;
  tags: string[];
  params: ParameterDef[];
  build?: (params: Record<string, number>) => Op[];
  ops?: Op[];
}
