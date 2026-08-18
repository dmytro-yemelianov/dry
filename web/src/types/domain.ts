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
  start?: [number | null, number | null, number | null];
  end?: [number | null, number | null, number | null];
  kind?: string;
  travel?: boolean;
  extruder_on?: boolean;
  speed?: number;
  length?: number;
  volume?: number;
  filament?: number;
  width?: number;
  height?: number;
  centre?: [number, number] | null;
  clockwise?: boolean;
  tags?: RowGroupTags;
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

export type GroupingKind = 'revolution' | 'figure' | 'layer' | 'routine';
export type GroupingMode = 'auto' | 'revolutions' | 'figures' | 'layers' | 'multi';

export interface RowGroupTags {
  layer?: number;
  layerZ?: number;
  figure?: number;
  figureType?: 'perimeter' | 'infill' | 'travel' | 'skirt' | 'bridge';
  turn?: number;
  turnAngleDeg?: number;
  feature?: string;
}

export interface GcodeRowMeta {
  index: number;
  raw: string;
  cmd: string;
  args: Record<string, string>;
  tags: RowGroupTags;
}

export interface HierarchyGroupNode {
  id: string;
  kind: 'layer' | 'figure' | 'turn' | 'feature';
  label: string;
  badge: string;
  startLine: number;
  endLine: number;
  startSeg: number;
  endSeg: number;
  lineCount: number;
  z?: number;
  children?: HierarchyGroupNode[];
}

export interface GcodeSection {
  index: number;
  line: number;
  segmentIndex: number;
  kind: GroupingKind;
  label: string;
  subLabel?: string;
  zRange?: [number, number];
  angleRangeDeg?: [number, number];
  moveCount?: number;
  volume?: number;
  time?: number;
}

export type RenderStyle = 'beads' | 'wireframe';
export type PlasticMaterial = 'cyan' | 'obsidian' | 'gold' | 'orange' | 'white' | 'resin';
export type SlicingFilterMode = 'all' | 'upToSection' | 'singleSection' | 'multiFilter';
export type GcodeViewFormat = 'stream' | 'table';
