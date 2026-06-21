import { Design } from '../design';
import type { Op } from '../ops';

const TAU = Math.PI * 2;
const EPS = 1e-9;
const DEFAULT_LAYER_HEIGHT = 0.28;
const DEFAULT_ADAPTIVE_MIN_LAYER_HEIGHT = 0.14;
const DEFAULT_ADAPTIVE_MAX_LAYER_HEIGHT = 0.32;
const DEFAULT_ADAPTIVE_MAX_LENGTH_DELTA = 0.35;
const DEFAULT_ADAPTIVE_MAX_POINT_DELTA = 0.45;
const DEFAULT_ADAPTIVE_MAX_DEPTH = 4;

export type TpmsSurface =
  | 'gyroid'
  | 'schwarz-p'
  | 'schwarz-d'
  | 'iwp'
  | 'neovius'
  | 'fischer-koch-s'
  | 'fischer-koch-y'
  | 'frd'
  | 'lidinoid'
  | 'split-p';

export interface TpmsSurfaceSpec {
  surface: TpmsSurface;
  label: string;
  equation: string;
}

interface InternalTpmsSurfaceSpec extends TpmsSurfaceSpec {
  field: (x: number, y: number, z: number) => number;
}

export interface TpmsOptions {
  surface?: TpmsSurface;
  /** Isosurface value f(x,y,z)=isoLevel. */
  isoLevel?: number;
  /** Cubic unit-cell size in mm. */
  cellSize?: number;
  cellsX?: number;
  cellsY?: number;
  cellsZ?: number;
  /** XY marching-squares resolution per unit cell. */
  samplesPerCell?: number;
  layerHeight?: number;
  z0?: number;
  centerX?: number;
  centerY?: number;
  beadWidth?: number;
  beadHeight?: number;
  nozzleTemp?: number;
  printSpeed?: number;
  flow?: number;
  /** Phase offsets in unit-cell periods, useful for moving seams/features. */
  phaseX?: number;
  phaseY?: number;
  phaseZ?: number;
  /** Add a single-wall rectangular perimeter around every sliced layer for infill-style previews. */
  perimeter?: boolean;
  perimeterInset?: number;
  /** Drop very short stitched contours. Defaults to one grid cell. */
  minPathLength?: number;
  /** Insert extra Z slices in intervals that are too tall or change contour topology sharply. */
  adaptive?: boolean;
  adaptiveMinLayerHeight?: number;
  adaptiveMaxLayerHeight?: number;
  adaptiveMaxLengthDelta?: number;
  adaptiveMaxPointDelta?: number;
  adaptiveMaxDepth?: number;
}

interface Point {
  x: number;
  y: number;
}

interface Segment {
  a: Point;
  b: Point;
}

interface Path {
  points: Point[];
}

interface LayerSlice {
  zLocal: number;
  paths: Path[];
  pathCount: number;
  pointCount: number;
  length: number;
}

interface AdaptiveSliceOptions {
  minLayerHeight: number;
  maxLayerHeight: number;
  maxLengthDelta: number;
  maxPointDelta: number;
  maxDepth: number;
}

const TPMS_INTERNAL: Record<TpmsSurface, InternalTpmsSurfaceSpec> = {
  gyroid: {
    surface: 'gyroid',
    label: 'Gyroid',
    equation: 'sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)',
    field: (x, y, z) => Math.sin(x) * Math.cos(y) + Math.sin(y) * Math.cos(z) + Math.sin(z) * Math.cos(x),
  },
  'schwarz-p': {
    surface: 'schwarz-p',
    label: 'Schwarz P',
    equation: 'cos(x) + cos(y) + cos(z)',
    field: (x, y, z) => Math.cos(x) + Math.cos(y) + Math.cos(z),
  },
  'schwarz-d': {
    surface: 'schwarz-d',
    label: 'Schwarz D / Diamond',
    equation: 'sin(x)sin(y)sin(z) + sin(x)cos(y)cos(z) + cos(x)sin(y)cos(z) + cos(x)cos(y)sin(z)',
    field: (x, y, z) =>
      Math.sin(x) * Math.sin(y) * Math.sin(z) +
      Math.sin(x) * Math.cos(y) * Math.cos(z) +
      Math.cos(x) * Math.sin(y) * Math.cos(z) +
      Math.cos(x) * Math.cos(y) * Math.sin(z),
  },
  iwp: {
    surface: 'iwp',
    label: 'Schoen I-WP',
    equation: '2(cos(x)cos(y)+cos(y)cos(z)+cos(z)cos(x)) - (cos(2x)+cos(2y)+cos(2z))',
    field: (x, y, z) =>
      2 * (Math.cos(x) * Math.cos(y) + Math.cos(y) * Math.cos(z) + Math.cos(z) * Math.cos(x)) -
      (Math.cos(2 * x) + Math.cos(2 * y) + Math.cos(2 * z)),
  },
  neovius: {
    surface: 'neovius',
    label: 'Neovius',
    equation: '3(cos(x)+cos(y)+cos(z)) + 4cos(x)cos(y)cos(z)',
    field: (x, y, z) => 3 * (Math.cos(x) + Math.cos(y) + Math.cos(z)) + 4 * Math.cos(x) * Math.cos(y) * Math.cos(z),
  },
  'fischer-koch-s': {
    surface: 'fischer-koch-s',
    label: 'Fischer-Koch S',
    equation: 'cos(2x)sin(y)cos(z) + cos(2y)sin(z)cos(x) + cos(2z)sin(x)cos(y)',
    field: (x, y, z) =>
      Math.cos(2 * x) * Math.sin(y) * Math.cos(z) +
      Math.cos(2 * y) * Math.sin(z) * Math.cos(x) +
      Math.cos(2 * z) * Math.sin(x) * Math.cos(y),
  },
  'fischer-koch-y': {
    surface: 'fischer-koch-y',
    label: 'Fischer-Koch Y',
    equation: 'cos(x)cos(y)cos(z)+sin(x)sin(y)sin(z)+sin(2x)sin(y)+sin(2y)sin(z)+sin(x)sin(2z)+sin(2x)cos(z)+cos(x)sin(2y)+cos(y)sin(2z)',
    field: (x, y, z) =>
      Math.cos(x) * Math.cos(y) * Math.cos(z) +
      Math.sin(x) * Math.sin(y) * Math.sin(z) +
      Math.sin(2 * x) * Math.sin(y) +
      Math.sin(2 * y) * Math.sin(z) +
      Math.sin(x) * Math.sin(2 * z) +
      Math.sin(2 * x) * Math.cos(z) +
      Math.cos(x) * Math.sin(2 * y) +
      Math.cos(y) * Math.sin(2 * z),
  },
  frd: {
    surface: 'frd',
    label: 'Schoen F-RD',
    equation: '4cos(x)cos(y)cos(z) - (cos(2x)cos(2y)+cos(2y)cos(2z)+cos(2z)cos(2x))',
    field: (x, y, z) =>
      4 * Math.cos(x) * Math.cos(y) * Math.cos(z) -
      (Math.cos(2 * x) * Math.cos(2 * y) +
        Math.cos(2 * y) * Math.cos(2 * z) +
        Math.cos(2 * z) * Math.cos(2 * x)),
  },
  lidinoid: {
    surface: 'lidinoid',
    label: 'Lidinoid',
    equation: 'sin(2x)cos(y)sin(z)+sin(2y)cos(z)sin(x)+sin(2z)cos(x)sin(y)-cos(2x)cos(2y)-cos(2y)cos(2z)-cos(2z)cos(2x)+0.3',
    field: (x, y, z) =>
      Math.sin(2 * x) * Math.cos(y) * Math.sin(z) +
      Math.sin(2 * y) * Math.cos(z) * Math.sin(x) +
      Math.sin(2 * z) * Math.cos(x) * Math.sin(y) -
      Math.cos(2 * x) * Math.cos(2 * y) -
      Math.cos(2 * y) * Math.cos(2 * z) -
      Math.cos(2 * z) * Math.cos(2 * x) +
      0.3,
  },
  'split-p': {
    surface: 'split-p',
    label: 'Split P',
    equation: '1.1(sum sin(2a)sin(c)cos(b)) - 0.2(sum cos(2a)cos(2b)) - 0.4(sum cos(2a))',
    field: (x, y, z) =>
      1.1 *
        (Math.sin(2 * x) * Math.sin(z) * Math.cos(y) +
          Math.sin(2 * y) * Math.sin(x) * Math.cos(z) +
          Math.sin(2 * z) * Math.sin(y) * Math.cos(x)) -
      0.2 *
        (Math.cos(2 * x) * Math.cos(2 * y) +
          Math.cos(2 * y) * Math.cos(2 * z) +
          Math.cos(2 * z) * Math.cos(2 * x)) -
      0.4 * (Math.cos(2 * x) + Math.cos(2 * y) + Math.cos(2 * z)),
  },
};

export const TPMS_SURFACES: Record<TpmsSurface, TpmsSurfaceSpec> = Object.fromEntries(
  Object.entries(TPMS_INTERNAL).map(([key, spec]) => [
    key,
    { surface: spec.surface, label: spec.label, equation: spec.equation },
  ])
) as Record<TpmsSurface, TpmsSurfaceSpec>;

export function tpmsSurfaceSpec(surface: TpmsSurface): TpmsSurfaceSpec {
  const spec = TPMS_SURFACES[surface];
  if (!spec) throw new Error(`unknown TPMS surface '${surface}'`);
  return spec;
}

export function tpmsField(surface: TpmsSurface, x: number, y: number, z: number): number {
  const spec = TPMS_INTERNAL[surface];
  if (!spec) throw new Error(`unknown TPMS surface '${surface}'`);
  return spec.field(x, y, z);
}

export function tpmsOps(options: TpmsOptions = {}): Op[] {
  const surface = options.surface ?? 'gyroid';
  const spec = TPMS_INTERNAL[surface];
  if (!spec) throw new Error(`unknown TPMS surface '${surface}'`);

  const isoLevel = finite('isoLevel', options.isoLevel ?? 0);
  const cellSize = positive('cellSize', options.cellSize ?? 12);
  const cellsX = integer('cellsX', options.cellsX ?? 2, 1);
  const cellsY = integer('cellsY', options.cellsY ?? 2, 1);
  const cellsZ = integer('cellsZ', options.cellsZ ?? 2, 1);
  const samplesPerCell = integer('samplesPerCell', options.samplesPerCell ?? 18, 4);
  const layerHeight = positive('layerHeight', options.layerHeight ?? DEFAULT_LAYER_HEIGHT);
  const z0 = positiveOrZero('z0', options.z0 ?? 0.2);
  const beadWidth = positive('beadWidth', options.beadWidth ?? 0.45);
  const beadHeight = positive('beadHeight', options.beadHeight ?? layerHeight);
  const centerX = finite('centerX', options.centerX ?? 50);
  const centerY = finite('centerY', options.centerY ?? 50);
  const nozzleTemp = positive('nozzleTemp', options.nozzleTemp ?? 210);
  const printSpeed = positive('printSpeed', options.printSpeed ?? 1200);
  const flow = positive('flow', options.flow ?? 1);
  const phaseX = finite('phaseX', options.phaseX ?? 0);
  const phaseY = finite('phaseY', options.phaseY ?? 0);
  const phaseZ = finite('phaseZ', options.phaseZ ?? 0);
  const perimeter = options.perimeter ?? false;
  const adaptive = Boolean(options.adaptive ?? false);
  const adaptiveMinLayerHeight = positive(
    'adaptiveMinLayerHeight',
    options.adaptiveMinLayerHeight ?? Math.min(layerHeight, DEFAULT_ADAPTIVE_MIN_LAYER_HEIGHT)
  );
  const adaptiveMaxLayerHeight = positive(
    'adaptiveMaxLayerHeight',
    options.adaptiveMaxLayerHeight ?? Math.min(layerHeight, DEFAULT_ADAPTIVE_MAX_LAYER_HEIGHT)
  );
  const adaptiveMaxLengthDelta = positive(
    'adaptiveMaxLengthDelta',
    options.adaptiveMaxLengthDelta ?? DEFAULT_ADAPTIVE_MAX_LENGTH_DELTA
  );
  const adaptiveMaxPointDelta = positive(
    'adaptiveMaxPointDelta',
    options.adaptiveMaxPointDelta ?? DEFAULT_ADAPTIVE_MAX_POINT_DELTA
  );
  const adaptiveMaxDepth = integer('adaptiveMaxDepth', options.adaptiveMaxDepth ?? DEFAULT_ADAPTIVE_MAX_DEPTH, 0);
  if (adaptiveMinLayerHeight - adaptiveMaxLayerHeight > EPS) {
    throw new Error('adaptiveMinLayerHeight must be <= adaptiveMaxLayerHeight');
  }

  const width = cellsX * cellSize;
  const depth = cellsY * cellSize;
  const height = cellsZ * cellSize;
  const nx = cellsX * samplesPerCell;
  const ny = cellsY * samplesPerCell;
  const dx = width / nx;
  const dy = depth / ny;
  const minPathLength = positiveOrZero('minPathLength', options.minPathLength ?? Math.min(dx, dy));
  const perimeterInset = Math.min(
    positiveOrZero('perimeterInset', options.perimeterInset ?? beadWidth),
    Math.max(0, width / 2 - EPS),
    Math.max(0, depth / 2 - EPS)
  );

  const ops: Op[] = [
    { op: 'geometry', width: beadWidth, height: beadHeight },
    { op: 'temperature', nozzle: nozzleTemp },
    { op: 'speed', print: printSpeed },
  ];
  if (Math.abs(flow - 1) > EPS) ops.push({ op: 'flow', ratio: flow });

  const buildLayer = (zLocal: number): LayerSlice => {
    const segments = marchingSquaresLayer(spec, isoLevel, width, depth, cellSize, nx, ny, zLocal, phaseX, phaseY, phaseZ);
    const paths = stitchSegments(segments).filter((path) => path.points.length >= 2 && pathLength(path.points) >= minPathLength);
    return layerSlice(zLocal, paths);
  };
  const layerSlices = buildLayerSlices(height, layerHeight, buildLayer, adaptive ? {
    minLayerHeight: adaptiveMinLayerHeight,
    maxLayerHeight: adaptiveMaxLayerHeight,
    maxLengthDelta: adaptiveMaxLengthDelta,
    maxPointDelta: adaptiveMaxPointDelta,
    maxDepth: adaptiveMaxDepth,
  } : null);

  let previousLocal: Point | null = null;
  for (const slice of layerSlices) {
    const zLocal = slice.zLocal;
    const z = z0 + zLocal;
    if (perimeter) {
      const rectLocal = rectanglePath(width, depth, perimeterInset);
      const rect = rectLocal.map((p) => ({ x: p.x - width / 2 + centerX, y: p.y - depth / 2 + centerY }));
      appendPath(ops, rect, z);
      previousLocal = rectLocal[rectLocal.length - 1];
    }
    const paths = orderPaths(slice.paths, previousLocal);
    for (const path of paths) {
      const points = path.points.map((p) => ({ x: p.x - width / 2 + centerX, y: p.y - depth / 2 + centerY }));
      appendPath(ops, points, z);
      previousLocal = path.points[path.points.length - 1];
    }
  }
  ops.push({ op: 'extruder', on: false });
  return ops;
}

export function tpms(options: TpmsOptions = {}): Design {
  const design = new Design();
  design.ops.push(...tpmsOps(options));
  return design;
}

function buildLayerSlices(
  height: number,
  layerHeight: number,
  buildLayer: (zLocal: number) => LayerSlice,
  adaptive: AdaptiveSliceOptions | null
): LayerSlice[] {
  const baseZ = baseLayerZs(height, layerHeight);
  const cache = new Map<number, LayerSlice>();
  const sample = (zLocal: number): LayerSlice => {
    const key = round(zLocal);
    let slice = cache.get(key);
    if (!slice) {
      slice = buildLayer(zLocal);
      cache.set(key, slice);
    }
    return slice;
  };
  const slices = [sample(baseZ[0])];
  for (let i = 1; i < baseZ.length; i++) {
    const a = slices[slices.length - 1];
    const b = sample(baseZ[i]);
    if (adaptive) {
      const inserted: LayerSlice[] = [];
      refineAdaptiveLayerInterval(a, b, sample, adaptive, inserted, 0);
      slices.push(...inserted);
    }
    slices.push(b);
  }
  return slices;
}

function baseLayerZs(height: number, layerHeight: number): number[] {
  const zValues = [0];
  for (let z = layerHeight; z < height - EPS; z += layerHeight) zValues.push(round(z));
  if (height > EPS && Math.abs(zValues[zValues.length - 1] - height) > EPS) zValues.push(round(height));
  return zValues;
}

function refineAdaptiveLayerInterval(
  a: LayerSlice,
  b: LayerSlice,
  sample: (zLocal: number) => LayerSlice,
  options: AdaptiveSliceOptions,
  out: LayerSlice[],
  depth: number
): void {
  if (!needsAdaptiveLayer(a, b, options, depth)) return;
  const midZ = round((a.zLocal + b.zLocal) / 2);
  if (midZ - a.zLocal < options.minLayerHeight - EPS || b.zLocal - midZ < options.minLayerHeight - EPS) return;
  const mid = sample(midZ);
  refineAdaptiveLayerInterval(a, mid, sample, options, out, depth + 1);
  out.push(mid);
  refineAdaptiveLayerInterval(mid, b, sample, options, out, depth + 1);
}

function needsAdaptiveLayer(a: LayerSlice, b: LayerSlice, options: AdaptiveSliceOptions, depth: number): boolean {
  const dz = b.zLocal - a.zLocal;
  if (dz <= options.minLayerHeight + EPS) return false;
  if (dz > options.maxLayerHeight + EPS) return depth < options.maxDepth;
  if (depth >= options.maxDepth) return false;
  if (Math.abs(a.pathCount - b.pathCount) >= 2) return true;
  const lengthScale = Math.max(a.length, b.length, 1);
  const pointScale = Math.max(a.pointCount, b.pointCount, 1);
  return (
    Math.abs(a.length - b.length) / lengthScale > options.maxLengthDelta ||
    Math.abs(a.pointCount - b.pointCount) / pointScale > options.maxPointDelta
  );
}

function layerSlice(zLocal: number, paths: Path[]): LayerSlice {
  return {
    zLocal: round(zLocal),
    paths,
    pathCount: paths.length,
    pointCount: paths.reduce((total, path) => total + path.points.length, 0),
    length: paths.reduce((total, path) => total + pathLength(path.points), 0),
  };
}

function rectanglePath(width: number, depth: number, inset: number): Point[] {
  return [
    { x: inset, y: inset },
    { x: width - inset, y: inset },
    { x: width - inset, y: depth - inset },
    { x: inset, y: depth - inset },
    { x: inset, y: inset },
  ];
}

function marchingSquaresLayer(
  spec: InternalTpmsSurfaceSpec,
  isoLevel: number,
  width: number,
  depth: number,
  cellSize: number,
  nx: number,
  ny: number,
  zLocal: number,
  phaseX: number,
  phaseY: number,
  phaseZ: number
): Segment[] {
  const dx = width / nx;
  const dy = depth / ny;
  const values = new Array((nx + 1) * (ny + 1));
  const valueAt = (i: number, j: number) => values[j * (nx + 1) + i] as number;

  for (let j = 0; j <= ny; j++) {
    for (let i = 0; i <= nx; i++) {
      const x = ((i * dx) / cellSize + phaseX) * TAU;
      const y = ((j * dy) / cellSize + phaseY) * TAU;
      const z = (zLocal / cellSize + phaseZ) * TAU;
      values[j * (nx + 1) + i] = spec.field(x, y, z) - isoLevel;
    }
  }

  const segments: Segment[] = [];
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      const p00 = { x: i * dx, y: j * dy };
      const p10 = { x: (i + 1) * dx, y: j * dy };
      const p11 = { x: (i + 1) * dx, y: (j + 1) * dy };
      const p01 = { x: i * dx, y: (j + 1) * dy };
      const v00 = scrubZero(valueAt(i, j));
      const v10 = scrubZero(valueAt(i + 1, j));
      const v11 = scrubZero(valueAt(i + 1, j + 1));
      const v01 = scrubZero(valueAt(i, j + 1));

      const crossings: Point[] = [];
      if (crosses(v00, v10)) crossings.push(interpolate(p00, p10, v00, v10));
      if (crosses(v10, v11)) crossings.push(interpolate(p10, p11, v10, v11));
      if (crosses(v11, v01)) crossings.push(interpolate(p11, p01, v11, v01));
      if (crosses(v01, v00)) crossings.push(interpolate(p01, p00, v01, v00));

      if (crossings.length === 2) {
        segments.push({ a: crossings[0], b: crossings[1] });
      } else if (crossings.length === 4) {
        const xc = ((i + 0.5) * dx / cellSize + phaseX) * TAU;
        const yc = ((j + 0.5) * dy / cellSize + phaseY) * TAU;
        const zc = (zLocal / cellSize + phaseZ) * TAU;
        if (spec.field(xc, yc, zc) - isoLevel >= 0) {
          segments.push({ a: crossings[0], b: crossings[1] }, { a: crossings[2], b: crossings[3] });
        } else {
          segments.push({ a: crossings[0], b: crossings[3] }, { a: crossings[1], b: crossings[2] });
        }
      }
    }
  }
  return segments;
}

function stitchSegments(segments: Segment[]): Path[] {
  const unused = new Set<number>(segments.map((_, i) => i));
  const endpoints = new Map<string, number[]>();
  for (let i = 0; i < segments.length; i++) {
    addEndpoint(endpoints, pointKey(segments[i].a), i);
    addEndpoint(endpoints, pointKey(segments[i].b), i);
  }

  const paths: Path[] = [];
  while (unused.size) {
    const first = unused.values().next().value as number;
    unused.delete(first);
    const path = [segments[first].a, segments[first].b];
    extendPath(path, segments, endpoints, unused, false);
    extendPath(path, segments, endpoints, unused, true);
    paths.push({ points: dedupeConsecutive(path) });
  }
  return paths;
}

function extendPath(
  path: Point[],
  segments: Segment[],
  endpoints: Map<string, number[]>,
  unused: Set<number>,
  atStart: boolean
): void {
  while (true) {
    const endpoint = atStart ? path[0] : path[path.length - 1];
    const next = (endpoints.get(pointKey(endpoint)) || []).find((idx) => unused.has(idx));
    if (next === undefined) return;
    unused.delete(next);
    const segment = segments[next];
    const key = pointKey(endpoint);
    const nextPoint = pointKey(segment.a) === key ? segment.b : segment.a;
    if (atStart) path.unshift(nextPoint);
    else path.push(nextPoint);
  }
}

function orderPaths(paths: Path[], cursor: Point | null): Path[] {
  const remaining = [...paths];
  const ordered: Path[] = [];
  let current = cursor;
  while (remaining.length) {
    let bestIndex = 0;
    let best = orientPath(remaining[0], current);
    let bestDistance = current ? distance(current, best.points[0]) : 0;
    for (let i = 1; i < remaining.length; i++) {
      const candidate = orientPath(remaining[i], current);
      const d = current ? distance(current, candidate.points[0]) : 0;
      if (d < bestDistance) {
        bestIndex = i;
        best = candidate;
        bestDistance = d;
      }
    }
    ordered.push(best);
    remaining.splice(bestIndex, 1);
    current = best.points[best.points.length - 1];
  }
  return ordered;
}

function orientPath(path: Path, cursor: Point | null): Path {
  if (!cursor || path.points.length < 2) return path;
  const first = path.points[0];
  const last = path.points[path.points.length - 1];
  return distance(cursor, last) < distance(cursor, first) ? { points: [...path.points].reverse() } : path;
}

function appendPath(ops: Op[], points: Point[], z: number): void {
  const [start, ...rest] = points;
  ops.push({ op: 'extruder', on: false }, move(start, z), { op: 'extruder', on: true });
  for (const point of rest) ops.push(move(point, z));
}

function move(point: Point, z: number): Op {
  return { op: 'move', x: round(point.x), y: round(point.y), z: round(z) };
}

function interpolate(a: Point, b: Point, va: number, vb: number): Point {
  const t = va / (va - vb);
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
}

function crosses(a: number, b: number): boolean {
  return (a < 0 && b > 0) || (a > 0 && b < 0);
}

function scrubZero(value: number): number {
  if (Math.abs(value) > EPS) return value;
  return value < 0 ? -EPS : EPS;
}

function addEndpoint(map: Map<string, number[]>, key: string, index: number): void {
  const list = map.get(key);
  if (list) list.push(index);
  else map.set(key, [index]);
}

function dedupeConsecutive(points: Point[]): Point[] {
  const out: Point[] = [];
  for (const point of points) {
    const prev = out[out.length - 1];
    if (!prev || distance(prev, point) > 1e-7) out.push(point);
  }
  return out;
}

function pathLength(points: Point[]): number {
  let total = 0;
  for (let i = 1; i < points.length; i++) total += distance(points[i - 1], points[i]);
  return total;
}

function pointKey(point: Point): string {
  return `${Math.round(point.x * 1e6)},${Math.round(point.y * 1e6)}`;
}

function distance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function finite(name: string, value: number): number {
  if (!Number.isFinite(value)) throw new Error(`${name} must be finite`);
  return value;
}

function positive(name: string, value: number): number {
  finite(name, value);
  if (value <= 0) throw new Error(`${name} must be > 0`);
  return value;
}

function positiveOrZero(name: string, value: number): number {
  finite(name, value);
  if (value < 0) throw new Error(`${name} must be >= 0`);
  return value;
}

function integer(name: string, value: number, min: number): number {
  finite(name, value);
  if (!Number.isInteger(value) || value < min) throw new Error(`${name} must be an integer >= ${min}`);
  return value;
}

function round(value: number): number {
  return Math.round(value * 1e6) / 1e6;
}
