import { Design } from '../design';
import type { Op } from '../ops';

const TAU = Math.PI * 2;
const DEG = Math.PI / 180;
const EPS = 1e-9;

export type StarPolygonFamily = 'M1' | 'M2' | 'M3' | 'M4';
export type StarPolygonRegime = 'star' | 'star-limit' | 'convex' | 'uniqueness-limit';
export type StarPolygonBasis = 'triangular' | 'square';

export interface StarPolygonFamilySpec {
  family: StarPolygonFamily;
  topology: string;
  starSides: number;
  alphaSplDeg: number;
  alphaUlDeg: number;
  basis: StarPolygonBasis;
  isotropicInPlane: boolean;
}

interface FamilyInternalSpec extends StarPolygonFamilySpec {
  connectorSteps: [number, number][];
  starRotationDeg: number;
  outerRadiusRatio: number;
  preferOddWidth: boolean;
}

export interface NormalizedStarPolygonAlpha {
  inputDeg: number;
  effectiveDeg: number;
  mirrored: boolean;
  regime: StarPolygonRegime;
}

export interface StarPolygonLatticeOptions {
  /** Paper lattice sub-family. */
  family?: StarPolygonFamily;
  /** Star-polygon angle alpha in degrees. Valid range is 0..2*alphaUL. */
  alphaDeg?: number;
  /** Unit cells across the generated patch. M2/M3/M4 default to odd width, matching the paper's print strategy. */
  cols?: number;
  /** Unit-cell rows. The paper specimens used three rows. */
  rows?: number;
  /** Unit-cell edge length LUC in mm. */
  unit?: number;
  /** Printed layers. The paper specimens used three single-extrusion layers. */
  layers?: number;
  /** Distance between repeated layers in mm. */
  layerHeight?: number;
  /** First layer Z in mm. */
  z0?: number;
  /** XY center of the generated patch. */
  centerX?: number;
  centerY?: number;
  /** Extrusion bead geometry in mm. */
  beadWidth?: number;
  beadHeight?: number;
  /** Process settings from the manufacturing appendix defaults. */
  nozzleTemp?: number;
  printSpeed?: number;
  flow?: number;
  /** Override motif size as a fraction of unit length. */
  outerRadiusRatio?: number;
  /** Emit inter-cell struts in addition to star-polygon cell loops. */
  includeConnectors?: boolean;
  /** Force an odd column count for M2/M3/M4 so each layer starts/ends on the same side more easily. */
  completeWidth?: boolean;
}

interface Point {
  x: number;
  y: number;
}

interface Cell {
  col: number;
  row: number;
  center: Point;
  outer: Point[];
  loop: Point[];
}

interface Path {
  points: Point[];
  closed: boolean;
}

const INTERNAL_FAMILIES: Record<StarPolygonFamily, FamilyInternalSpec> = {
  M1: {
    family: 'M1',
    topology: '4 . 4*alpha . 4**alpha',
    starSides: 4,
    alphaSplDeg: 90,
    alphaUlDeg: 135,
    basis: 'triangular',
    isotropicInPlane: true,
    connectorSteps: [[1, 0], [0, 1]],
    starRotationDeg: 45,
    outerRadiusRatio: 0.31,
    preferOddWidth: false,
  },
  M2: {
    family: 'M2',
    topology: '3 . 6*alpha . 6**alpha',
    starSides: 6,
    alphaSplDeg: 120,
    alphaUlDeg: 150,
    basis: 'triangular',
    isotropicInPlane: true,
    connectorSteps: [[1, 0], [0, 1], [1, -1]],
    starRotationDeg: 30,
    outerRadiusRatio: 0.23,
    preferOddWidth: true,
  },
  M3: {
    family: 'M3',
    topology: '6 . 3*alpha . 3**alpha',
    starSides: 3,
    alphaSplDeg: 60,
    alphaUlDeg: 120,
    basis: 'triangular',
    isotropicInPlane: true,
    connectorSteps: [[1, 0], [0, 1], [1, -1]],
    starRotationDeg: -90,
    outerRadiusRatio: 0.32,
    preferOddWidth: true,
  },
  M4: {
    family: 'M4',
    topology: '3 . 3*alpha . 3 . 3**alpha',
    starSides: 3,
    alphaSplDeg: 60,
    alphaUlDeg: 120,
    basis: 'square',
    isotropicInPlane: false,
    connectorSteps: [[1, 0], [0, 1], [1, 1]],
    starRotationDeg: -90,
    outerRadiusRatio: 0.30,
    preferOddWidth: true,
  },
};

export const STAR_POLYGON_FAMILIES: Record<StarPolygonFamily, StarPolygonFamilySpec> = {
  M1: publicSpec(INTERNAL_FAMILIES.M1),
  M2: publicSpec(INTERNAL_FAMILIES.M2),
  M3: publicSpec(INTERNAL_FAMILIES.M3),
  M4: publicSpec(INTERNAL_FAMILIES.M4),
};

function publicSpec(spec: FamilyInternalSpec): StarPolygonFamilySpec {
  return {
    family: spec.family,
    topology: spec.topology,
    starSides: spec.starSides,
    alphaSplDeg: spec.alphaSplDeg,
    alphaUlDeg: spec.alphaUlDeg,
    basis: spec.basis,
    isotropicInPlane: spec.isotropicInPlane,
  };
}

export function starPolygonFamilySpec(family: StarPolygonFamily): StarPolygonFamilySpec {
  const spec = STAR_POLYGON_FAMILIES[family];
  if (!spec) throw new Error(`unknown star-polygon lattice family '${family}'`);
  return spec;
}

export function normalizeStarPolygonAlpha(
  family: StarPolygonFamily,
  alphaDeg: number
): NormalizedStarPolygonAlpha {
  const spec = INTERNAL_FAMILIES[family];
  if (!spec) throw new Error(`unknown star-polygon lattice family '${family}'`);
  finite('alphaDeg', alphaDeg);

  const max = spec.alphaUlDeg * 2;
  if (alphaDeg < 0 || alphaDeg > max) {
    throw new Error(`${family} alphaDeg must be in 0..${max} degrees`);
  }

  const mirrored = alphaDeg > spec.alphaUlDeg;
  const effectiveDeg = mirrored ? max - alphaDeg : alphaDeg;
  const regime =
    Math.abs(effectiveDeg - spec.alphaUlDeg) <= EPS ? 'uniqueness-limit'
      : Math.abs(effectiveDeg - spec.alphaSplDeg) <= EPS ? 'star-limit'
        : effectiveDeg < spec.alphaSplDeg ? 'star'
          : 'convex';

  return { inputDeg: alphaDeg, effectiveDeg, mirrored, regime };
}

/**
 * Ratio of dent radius to star-point radius for an equiangular/equilateral star n-gon.
 * At alphaSPL it equals cos(pi / n); at alphaUL it reaches 1.0.
 */
export function starPolygonDentRadiusRatio(starSides: number, alphaDeg: number): number {
  const n = integer('starSides', starSides, 3);
  finite('alphaDeg', alphaDeg);
  const phi = Math.PI / n;
  const t = Math.tan((alphaDeg * DEG) / 2);
  if (Math.abs(t) <= EPS) return 0;
  return t / (Math.sin(phi) + t * Math.cos(phi));
}

export function starPolygonLatticeOps(options: StarPolygonLatticeOptions = {}): Op[] {
  const family = options.family ?? 'M1';
  const spec = INTERNAL_FAMILIES[family];
  if (!spec) throw new Error(`unknown star-polygon lattice family '${family}'`);

  const alpha = normalizeStarPolygonAlpha(family, options.alphaDeg ?? spec.alphaSplDeg / 2);
  let cols = integer('cols', options.cols ?? 5, 1);
  const rows = integer('rows', options.rows ?? 3, 1);
  const unit = positive('unit', options.unit ?? 14);
  const layers = integer('layers', options.layers ?? 3, 1);
  const layerHeight = positive('layerHeight', options.layerHeight ?? 0.167);
  const z0 = positiveOrZero('z0', options.z0 ?? layerHeight);
  const beadWidth = positive('beadWidth', options.beadWidth ?? 0.5);
  const beadHeight = positive('beadHeight', options.beadHeight ?? layerHeight);
  const centerX = finite('centerX', options.centerX ?? 50);
  const centerY = finite('centerY', options.centerY ?? 50);
  const nozzleTemp = positive('nozzleTemp', options.nozzleTemp ?? 210);
  const printSpeed = positive('printSpeed', options.printSpeed ?? 1000);
  const flow = positive('flow', options.flow ?? 1);
  const includeConnectors = options.includeConnectors ?? true;
  const completeWidth = options.completeWidth ?? true;
  const outerRadiusRatio = positive('outerRadiusRatio', options.outerRadiusRatio ?? spec.outerRadiusRatio);

  if (completeWidth && spec.preferOddWidth && cols % 2 === 0) cols += 1;

  const paths = centerPaths(buildPaths(spec, alpha, cols, rows, unit, outerRadiusRatio, includeConnectors), centerX, centerY);
  const ordered = orderPaths(paths);
  const ops: Op[] = [
    { op: 'geometry', width: beadWidth, height: beadHeight },
    { op: 'temperature', nozzle: nozzleTemp },
    { op: 'speed', print: printSpeed },
  ];
  if (Math.abs(flow - 1) > EPS) ops.push({ op: 'flow', ratio: flow });

  for (let layer = 0; layer < layers; layer++) {
    const z = z0 + layer * layerHeight;
    const layerPaths = layer % 2 === 0 ? ordered : [...ordered].reverse().map(reversePath);
    appendLayerOps(ops, layerPaths, z);
  }
  ops.push({ op: 'extruder', on: false });
  return ops;
}

export function starPolygonLattice(options: StarPolygonLatticeOptions = {}): Design {
  const design = new Design();
  design.ops.push(...starPolygonLatticeOps(options));
  return design;
}

function buildPaths(
  spec: FamilyInternalSpec,
  alpha: NormalizedStarPolygonAlpha,
  cols: number,
  rows: number,
  unit: number,
  outerRadiusRatio: number,
  includeConnectors: boolean
): Path[] {
  const [a1, a2] = basisVectors(spec.basis, unit);
  const outerRadius = unit * outerRadiusRatio;
  const cells: Cell[] = [];
  const byKey = new Map<string, Cell>();
  const handedness = alpha.mirrored ? -1 : 1;

  for (let row = 0; row < rows; row++) {
    for (let col = 0; col < cols; col++) {
      const center = add(scale(a1, col), scale(a2, row));
      const loop = starPolygonPoints(
        spec.starSides,
        outerRadius,
        alpha.effectiveDeg,
        center,
        spec.starRotationDeg * DEG,
        handedness
      );
      const outer = loop.filter((_, i) => i % 2 === 0);
      const cell = { col, row, center, outer, loop: [...loop, loop[0]] };
      cells.push(cell);
      byKey.set(cellKey(col, row), cell);
    }
  }

  const paths: Path[] = cells.map((cell) => ({ points: cell.loop, closed: true }));
  if (!includeConnectors) return paths;

  for (const cell of cells) {
    for (const [dc, dr] of spec.connectorSteps) {
      const neighbor = byKey.get(cellKey(cell.col + dc, cell.row + dr));
      if (!neighbor) continue;
      const [p, q] = closestPair(cell.outer, neighbor.outer);
      paths.push({ points: [p, q], closed: false });
    }
  }

  return paths;
}

function starPolygonPoints(
  sides: number,
  outerRadius: number,
  alphaDeg: number,
  center: Point,
  rotation: number,
  handedness: 1 | -1
): Point[] {
  const dentRadius = outerRadius * starPolygonDentRadiusRatio(sides, alphaDeg);
  const points: Point[] = [];
  for (let i = 0; i < sides * 2; i++) {
    const radius = i % 2 === 0 ? outerRadius : dentRadius;
    const angle = rotation + handedness * ((i * Math.PI) / sides);
    points.push({ x: center.x + radius * Math.cos(angle), y: center.y + radius * Math.sin(angle) });
  }
  return points;
}

function basisVectors(basis: StarPolygonBasis, unit: number): [Point, Point] {
  if (basis === 'square') return [{ x: unit, y: 0 }, { x: 0, y: unit }];
  return [
    { x: Math.cos(Math.PI / 3) * unit, y: Math.sin(Math.PI / 3) * unit },
    { x: -Math.cos(Math.PI / 3) * unit, y: Math.sin(Math.PI / 3) * unit },
  ];
}

function appendLayerOps(ops: Op[], paths: Path[], z: number): void {
  for (const path of paths) {
    if (path.points.length < 2) continue;
    const [start, ...rest] = path.points;
    ops.push({ op: 'extruder', on: false }, move(start, z), { op: 'extruder', on: true });
    for (const point of rest) ops.push(move(point, z));
  }
}

function move(point: Point, z: number): Op {
  return { op: 'move', x: round(point.x), y: round(point.y), z: round(z) };
}

function centerPaths(paths: Path[], centerX: number, centerY: number): Path[] {
  const all = paths.flatMap((path) => path.points);
  const minX = Math.min(...all.map((p) => p.x));
  const maxX = Math.max(...all.map((p) => p.x));
  const minY = Math.min(...all.map((p) => p.y));
  const maxY = Math.max(...all.map((p) => p.y));
  const dx = centerX - (minX + maxX) / 2;
  const dy = centerY - (minY + maxY) / 2;
  return paths.map((path) => ({
    closed: path.closed,
    points: path.points.map((p) => ({ x: p.x + dx, y: p.y + dy })),
  }));
}

function orderPaths(paths: Path[]): Path[] {
  const remaining = [...paths];
  const ordered: Path[] = [];
  let cursor: Point | null = null;

  while (remaining.length) {
    let bestIndex = 0;
    let bestPath = preparePath(remaining[0], cursor);
    let bestDistance = cursor ? distance(cursor, bestPath.points[0]) : 0;

    for (let i = 1; i < remaining.length; i++) {
      const candidate = preparePath(remaining[i], cursor);
      const d = cursor ? distance(cursor, candidate.points[0]) : 0;
      if (d < bestDistance) {
        bestIndex = i;
        bestPath = candidate;
        bestDistance = d;
      }
    }

    ordered.push(bestPath);
    remaining.splice(bestIndex, 1);
    cursor = bestPath.points[bestPath.points.length - 1];
  }

  return ordered;
}

function preparePath(path: Path, cursor: Point | null): Path {
  if (!cursor || path.points.length < 2) return path;
  if (!path.closed) {
    const first = path.points[0];
    const last = path.points[path.points.length - 1];
    return distance(cursor, last) < distance(cursor, first) ? reversePath(path) : path;
  }

  const ring = path.points.slice(0, -1);
  let best = 0;
  let bestDistance = distance(cursor, ring[0]);
  for (let i = 1; i < ring.length; i++) {
    const d = distance(cursor, ring[i]);
    if (d < bestDistance) {
      best = i;
      bestDistance = d;
    }
  }
  const rotated = [...ring.slice(best), ...ring.slice(0, best)];
  return { closed: true, points: [...rotated, rotated[0]] };
}

function reversePath(path: Path): Path {
  if (!path.closed) return { closed: false, points: [...path.points].reverse() };
  const ring = path.points.slice(0, -1).reverse();
  return { closed: true, points: [...ring, ring[0]] };
}

function closestPair(a: Point[], b: Point[]): [Point, Point] {
  let best: [Point, Point] = [a[0], b[0]];
  let bestDistance = distance(a[0], b[0]);
  for (const p of a) {
    for (const q of b) {
      const d = distance(p, q);
      if (d < bestDistance) {
        best = [p, q];
        bestDistance = d;
      }
    }
  }
  return best;
}

function cellKey(col: number, row: number): string {
  return `${col},${row}`;
}

function add(a: Point, b: Point): Point {
  return { x: a.x + b.x, y: a.y + b.y };
}

function scale(a: Point, k: number): Point {
  return { x: a.x * k, y: a.y * k };
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
