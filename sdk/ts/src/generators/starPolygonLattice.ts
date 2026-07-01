import { Design } from '../design';
import type { Op } from '../ops';

const DEG = Math.PI / 180;
const EPS = 1e-9;

/** Star-polygon lattice family from the original M1-M4 construction. */
export type StarPolygonFamily = 'M1' | 'M2' | 'M3' | 'M4';
/** Geometric regime after normalizing the input alpha angle. */
export type StarPolygonRegime = 'star' | 'star-limit' | 'convex' | 'uniqueness-limit';
/** Base tiling used by a star-polygon lattice family. */
export type StarPolygonBasis = 'triangular' | 'square';

/** Static metadata for one star-polygon lattice family. */
export interface StarPolygonFamilySpec {
  /** Family identifier. */
  family: StarPolygonFamily;
  /** Human-readable topology label. */
  topology: string;
  /** Number of star sides in the underlying polygon. */
  starSides: number;
  /** Star-polygon limit angle in degrees. */
  alphaSplDeg: number;
  /** Uniqueness-limit angle in degrees. */
  alphaUlDeg: number;
  /** Base tiling used by this family. */
  basis: StarPolygonBasis;
  /** Whether the family is isotropic in the print plane. */
  isotropicInPlane: boolean;
}

/** Normalized alpha angle and regime classification for a lattice family. */
export interface NormalizedStarPolygonAlpha {
  /** Caller-provided alpha angle in degrees. */
  inputDeg: number;
  /** Mirrored/effective alpha angle in degrees. */
  effectiveDeg: number;
  /** Whether the input angle was mirrored around the uniqueness limit. */
  mirrored: boolean;
  /** Classified geometric regime. */
  regime: StarPolygonRegime;
}

/** Options for generating a star-polygon lattice toolpath. */
export interface StarPolygonLatticeOptions {
  /** Paper lattice sub-family. */
  family?: StarPolygonFamily;
  /** Colab star-polygon angle alpha in degrees. The original notebook defaults to 30. */
  alphaDeg?: number;
  /** Unit cells along the print length. The original notebook calls this units_x. */
  cols?: number;
  /** Unit cells in the print width. The original notebook calls this units_y. */
  rows?: number;
  /** Strut length in mm. This is the original notebook's seg_length parameter. */
  segLength?: number;
  /** Backward-compatible alias for segLength. */
  unit?: number;
  /** Printed layers. The original notebook defaults to two layers. */
  layers?: number;
  /** Distance between repeated layers in mm. */
  layerHeight?: number;
  /** First layer Z in mm. Defaults to 0.8 * layerHeight like the original notebook. */
  z0?: number;
  /** XY start offset. */
  startX?: number;
  startY?: number;
  /** Backward-compatible aliases for the old motif-centered generator. */
  centerX?: number;
  centerY?: number;
  /** Extrusion bead geometry in mm. */
  beadWidth?: number;
  beadHeight?: number;
  /** Process settings from the original notebook defaults. */
  nozzleTemp?: number;
  printSpeed?: number;
  flow?: number;
  /** Keep the three printed return lines that condition the next layer in the original notebook. */
  sacrificialReturn?: boolean;
  /** For M4, round odd row counts up to an even row-pair width like the original notebook. */
  completeWidth?: boolean;
  /** Deprecated no-op from the older motif approximation. */
  outerRadiusRatio?: number;
  /** Deprecated no-op from the older motif approximation. */
  includeConnectors?: boolean;
}

interface FcPoint {
  kind: 'point';
  x: number | null;
  y: number | null;
  z: number | null;
}

interface FcExtruder {
  kind: 'extruder';
  on: boolean;
}

type FcStep = FcPoint | FcExtruder;

interface FcVector {
  x?: number;
  y?: number;
  z?: number;
}

interface BuiltLattice {
  steps: FcStep[];
  repeatOffsetX: number;
}

const FAMILY_SPECS: Record<StarPolygonFamily, StarPolygonFamilySpec> = {
  M1: {
    family: 'M1',
    topology: '4 . 4*alpha . 4**alpha',
    starSides: 4,
    alphaSplDeg: 90,
    alphaUlDeg: 135,
    basis: 'triangular',
    isotropicInPlane: true,
  },
  M2: {
    family: 'M2',
    topology: '3 . 6*alpha . 6**alpha',
    starSides: 6,
    alphaSplDeg: 120,
    alphaUlDeg: 150,
    basis: 'triangular',
    isotropicInPlane: true,
  },
  M3: {
    family: 'M3',
    topology: '6 . 3*alpha . 3**alpha',
    starSides: 3,
    alphaSplDeg: 60,
    alphaUlDeg: 120,
    basis: 'triangular',
    isotropicInPlane: true,
  },
  M4: {
    family: 'M4',
    topology: '3 . 3*alpha . 3 . 3**alpha',
    starSides: 3,
    alphaSplDeg: 60,
    alphaUlDeg: 120,
    basis: 'square',
    isotropicInPlane: false,
  },
};

/** Metadata catalog for the supported star-polygon lattice families. */
export const STAR_POLYGON_FAMILIES: Record<StarPolygonFamily, StarPolygonFamilySpec> = FAMILY_SPECS;

/** Return static metadata for a star-polygon lattice family. */
export function starPolygonFamilySpec(family: StarPolygonFamily): StarPolygonFamilySpec {
  const spec = STAR_POLYGON_FAMILIES[family];
  if (!spec) throw new Error(`unknown star-polygon lattice family '${family}'`);
  return spec;
}

/** Normalize an alpha angle into its effective value and geometric regime. */
export function normalizeStarPolygonAlpha(
  family: StarPolygonFamily,
  alphaDeg: number
): NormalizedStarPolygonAlpha {
  const spec = FAMILY_SPECS[family];
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

/** Generate Dry L1 authoring operations for a star-polygon lattice. */
export function starPolygonLatticeOps(options: StarPolygonLatticeOptions = {}): Op[] {
  const family = options.family ?? 'M1';
  if (!FAMILY_SPECS[family]) throw new Error(`unknown star-polygon lattice family '${family}'`);

  const alphaDeg = finite('alphaDeg', options.alphaDeg ?? 30);
  const cols = integer('cols', options.cols ?? 10, 1);
  const rows = integer('rows', options.rows ?? 3, 1);
  const segLength = positive('segLength', options.segLength ?? options.unit ?? 4.33);
  const layers = integer('layers', options.layers ?? 2, 1);
  const layerHeight = positive('layerHeight', options.layerHeight ?? 0.2);
  const z0 = positiveOrZero('z0', options.z0 ?? 0.8 * layerHeight);
  const startX = finite('startX', options.startX ?? options.centerX ?? 30);
  const startY = finite('startY', options.startY ?? options.centerY ?? 30);
  const beadWidth = positive('beadWidth', options.beadWidth ?? 0.5);
  const beadHeight = positive('beadHeight', options.beadHeight ?? layerHeight);
  const nozzleTemp = positive('nozzleTemp', options.nozzleTemp ?? 210);
  const printSpeed = positive('printSpeed', options.printSpeed ?? 1000);
  const flow = positive('flow', options.flow ?? 1);
  const completeWidth = options.completeWidth ?? true;
  const sacrificialReturn = options.sacrificialReturn ?? true;

  const built = buildColabLattice(family, alphaDeg, segLength, cols, rows, completeWidth);
  const lattice = sacrificialReturn ? [...built.steps, ...returnLines(built.repeatOffsetX)] : built.steps;
  const layered = copyMoveSteps(lattice, { z: layerHeight }, layers);
  const shifted = moveSteps(layered, { x: startX, y: startY, z: z0 });

  const ops: Op[] = [
    { op: 'geometry', width: beadWidth, height: beadHeight },
    { op: 'temperature', nozzle: nozzleTemp },
    { op: 'speed', print: printSpeed },
  ];
  if (Math.abs(flow - 1) > EPS) ops.push({ op: 'flow', ratio: flow });

  appendStepOps(ops, shifted);
  return ops;
}

/** Generate a fluent `Design` containing a star-polygon lattice toolpath. */
export function starPolygonLattice(options: StarPolygonLatticeOptions = {}): Design {
  const design = new Design();
  design.ops.push(...starPolygonLatticeOps(options));
  return design;
}

function buildColabLattice(
  family: StarPolygonFamily,
  alphaDeg: number,
  segLength: number,
  cols: number,
  rows: number,
  completeWidth: boolean
): BuiltLattice {
  switch (family) {
    case 'M1':
      return buildM1(alphaDeg, segLength, cols, rows);
    case 'M2':
      return buildM2(alphaDeg, segLength, cols, rows);
    case 'M3':
      return buildM3(alphaDeg, segLength, cols, rows);
    case 'M4':
      return buildM4(alphaDeg, segLength, cols, rows, completeWidth);
  }
}

function buildM1(alphaDeg: number, segLength: number, cols: number, rows: number): BuiltLattice {
  const unit: FcStep[] = [point(0, 0, 0)];
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-alphaDeg / 2)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-60 + alphaDeg / 2)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(120 - alphaDeg / 2)));

  const repeatOffsetX = requireAxis('repeatOffsetX', pointAt(unit, 3).x);
  const rowOffsetX = -(requireAxis('unit[2].x', pointAt(unit, 2).x) - requireAxis('unit[1].x', pointAt(unit, 1).x));
  const rowOffsetY = -(requireAxis('unit[1].y', pointAt(unit, 1).y) + requireAxis('unit[2].y', pointAt(unit, 2).y));

  const row1 = copyMoveSteps(unit, { x: repeatOffsetX }, cols);
  const row2 = reflectXReverse(row1);
  const rowPath = [...row1, ...row2];
  const lattice: FcStep[] = [];
  const rowCount = rows * 2 - 1;
  for (let i = 0; i < rowCount; i++) {
    lattice.push(...moveSteps(rowPath, { x: i % 2 === 1 ? rowOffsetX : 0, y: rowOffsetY * i }));
  }
  return { steps: lattice, repeatOffsetX };
}

function buildM2(alphaDeg: number, segLength: number, cols: number, rows: number): BuiltLattice {
  const devAngle = Math.atan((1 - Math.cos(rad(alphaDeg))) / (Math.sqrt(3) + Math.sin(rad(alphaDeg)))) / DEG;

  const unit: FcStep[] = [point(0, 0, 0)];
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle + 90 - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle + 30)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle - 150 - alphaDeg)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(devAngle + 30 - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle - 150)));
  unit.push(extruder(true));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(devAngle - 90)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle - 30)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle + 30)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle + 90)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle + 150)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(devAngle - 30)));
  unit.push(extruder(true));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(devAngle + 150 - alphaDeg)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(devAngle - 30 - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(devAngle - 90)));
  unit.push(extruder(true));

  const repeatOffsetX = requireAxis('repeatOffsetX', pointAt(unit, 19).x);
  const rowOffsetX = requireAxis('unit[16].x', pointAt(unit, 16).x) - requireAxis('unit[8].x', pointAt(unit, 8).x);
  const rowOffsetY = requireAxis('unit[3].y', pointAt(unit, 3).y) - requireAxis('unit[9].y', pointAt(unit, 9).y);
  const row1 = copyMoveSteps(unit, { x: repeatOffsetX }, cols);

  const backStartX = requireAxis('unit[1].x', pointAt(unit, 1).x) + cols * repeatOffsetX + rowOffsetX;
  const backStartY = requireAxis('unit[1].y', pointAt(unit, 1).y) + rowOffsetY;
  const back: FcStep[] = [point(backStartX, backStartY)];
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle - 90 - alphaDeg)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle + 90)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle + 150 - alphaDeg)));
  back.push(extruder(false));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(devAngle - 30 - alphaDeg)));
  back.push(extruder(true));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(devAngle + 150)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle - 150 - alphaDeg)));
  back.push(extruder(false));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(devAngle + 30 - alphaDeg)));
  back.push(extruder(true));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(devAngle - 150)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle - 90)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle - 30)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle + 30)));
  back.push(extruder(false));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(devAngle + 90)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle + 150)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(devAngle - 150)));
  back.push(extruder(true));

  const row2 = copyMoveSteps(back, { x: -repeatOffsetX }, cols);
  const lattice = copyMoveSteps([...row1, ...row2], { y: 2 * rowOffsetY }, rows);
  return { steps: lattice, repeatOffsetX };
}

function buildM3(alphaDeg: number, segLength: number, cols: number, rows: number): BuiltLattice {
  let unit: FcStep[] = [point(0, 0, 0)];
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(120)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(0)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(180 - alphaDeg)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(-alphaDeg)));
  unit.push(extruder(true));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(-120)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(60 - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-120)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(0)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(120)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(-60)));
  unit.push(extruder(true));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(120 - alphaDeg)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(-60)));
  unit.push(extruder(true));

  const rotationAngle = -Math.atan(
    (requireAxis('unit[15].y', pointAt(unit, 15).y) - requireAxis('unit[1].y', pointAt(unit, 1).y)) /
      (requireAxis('unit[15].x', pointAt(unit, 15).x) - requireAxis('unit[1].x', pointAt(unit, 1).x))
  );
  unit = rotateSteps(unit, pointAt(unit, 1), rotationAngle);

  const repeatOffsetX = requireAxis('unit[17].x', pointAt(unit, 17).x) - requireAxis('unit[0].x', pointAt(unit, 0).x);
  const rowOffsetY = requireAxis('unit[3].y', pointAt(unit, 3).y) - requireAxis('unit[9].y', pointAt(unit, 9).y);
  const backStartX =
    requireAxis('unit[3].x', pointAt(unit, 3).x) -
    requireAxis('unit[9].x', pointAt(unit, 9).x) +
    (cols + 1) * repeatOffsetX;

  const row1 = copyMoveSteps(unit, { x: repeatOffsetX }, cols);
  const back: FcStep[] = [
    clonePoint(pointAt(unit, 0)),
    clonePoint(pointAt(unit, 2)),
    clonePoint(pointAt(unit, 3)),
    extruder(false),
    clonePoint(pointAt(unit, 2)),
    extruder(true),
    clonePoint(pointAt(unit, 1)),
    clonePoint(pointAt(unit, 0)),
    extruder(false),
    clonePoint(pointAt(unit, 1)),
    extruder(true),
    movePoint(pointAt(unit, 10), { x: -repeatOffsetX }),
    movePoint(pointAt(unit, 11), { x: -repeatOffsetX }),
    extruder(false),
    movePoint(pointAt(unit, 10), { x: -repeatOffsetX }),
    extruder(true),
    movePoint(pointAt(unit, 9), { x: -repeatOffsetX }),
    movePoint(pointAt(unit, 8), { x: -repeatOffsetX }),
    movePoint(pointAt(unit, 0), { x: -repeatOffsetX }),
  ];
  const movedBack = moveSteps(back, { x: backStartX, y: rowOffsetY });
  const row2 = copyMoveSteps(movedBack, { x: -repeatOffsetX }, cols + 1);
  const lattice = copyMoveSteps([...row1, ...row2], { y: 2 * rowOffsetY }, rows);
  return { steps: lattice, repeatOffsetX };
}

function buildM4(
  alphaInputDeg: number,
  segLength: number,
  cols: number,
  rows: number,
  completeWidth: boolean
): BuiltLattice {
  const alphaDeg = Math.abs(alphaInputDeg - 150) <= EPS ? 120 : alphaInputDeg;
  const devAngle =
    Math.abs(
      Math.acos(
        -Math.sqrt(1 + Math.sin(2 * rad(alphaDeg))) /
          Math.sqrt(3 - 2 * Math.cos(rad(alphaDeg)) + 2 * Math.sin(rad(alphaDeg)))
      )
    ) / DEG;
  finite('M4 devAngle', devAngle);

  const unit: FcStep[] = [point(0, 0, 0)];
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(90 - devAngle)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(270 - devAngle - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(180 - devAngle)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(-devAngle)));
  unit.push(extruder(true));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(180 - devAngle - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(90 - devAngle - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-devAngle - alphaDeg)));
  unit.push(extruder(false));
  unit.push(polarToPoint(pointAt(unit, -2), segLength, rad(-90 - devAngle - alphaDeg)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-180 - devAngle - alphaDeg)));
  unit.push(extruder(true));

  const repeatOffsetX = requireAxis('unit[12].x', pointAt(unit, 12).x) - requireAxis('unit[0].x', pointAt(unit, 0).x);
  const rowOffsetY = requireAxis('unit[3].y', pointAt(unit, 3).y) - requireAxis('unit[8].y', pointAt(unit, 8).y);
  const row1 = copyMoveSteps(unit, { x: repeatOffsetX }, cols);

  const backStartX =
    requireAxis('row1[-2].x', pointAt(row1, -2).x) +
    (requireAxis('unit[1].x', pointAt(unit, 1).x) - requireAxis('unit[0].x', pointAt(unit, 0).x));
  const backStartY =
    rowOffsetY -
    (requireAxis('unit[7].y', pointAt(unit, 7).y) -
      requireAxis('unit[8].y', pointAt(unit, 8).y) -
      (requireAxis('unit[5].y', pointAt(unit, 5).y) - requireAxis('unit[7].y', pointAt(unit, 7).y)));

  const back: FcStep[] = [point(backStartX, backStartY)];
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(-90 - devAngle)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(90 - devAngle - alphaDeg)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(-devAngle - alphaDeg)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(-90 - devAngle - alphaDeg)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(180 - devAngle)));
  back.push(extruder(false));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(-devAngle)));
  back.push(extruder(true));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(180 - devAngle - alphaDeg)));
  back.push(extruder(false));
  back.push(polarToPoint(pointAt(back, -2), segLength, rad(90 - devAngle - alphaDeg)));
  back.push(polarToPoint(pointAt(back, -1), segLength, rad(-devAngle - alphaDeg)));
  back.push(extruder(true));

  const row2 = copyMoveSteps(back, { x: -repeatOffsetX }, cols);
  const widthRows = completeWidth && rows % 2 !== 0 ? rows + 1 : rows;
  const rowPairs = Math.max(1, Math.floor(widthRows / 2));
  const lattice = copyMoveSteps([...row1, ...row2], { y: 2 * rowOffsetY }, rowPairs);
  return { steps: lattice, repeatOffsetX };
}

function appendStepOps(ops: Op[], steps: FcStep[]): void {
  ops.push({ op: 'extruder', on: false });
  let placed = false;
  for (const step of steps) {
    if (step.kind === 'extruder') {
      ops.push({ op: 'extruder', on: step.on });
      continue;
    }
    ops.push({
      op: 'move',
      x: nullableRound(step.x),
      y: nullableRound(step.y),
      z: nullableRound(step.z),
    });
    if (!placed) {
      ops.push({ op: 'extruder', on: true });
      placed = true;
    }
  }
  ops.push({ op: 'extruder', on: false });
}

function returnLines(repeatOffsetX: number): FcStep[] {
  return [point(-repeatOffsetX, null, null), point(null, 0, null), point(0, null, null)];
}

function point(x: number | null = null, y: number | null = null, z: number | null = null): FcPoint {
  return { kind: 'point', x, y, z };
}

function extruder(on: boolean): FcExtruder {
  return { kind: 'extruder', on };
}

function pointAt(steps: FcStep[], index: number): FcPoint {
  const resolved = index < 0 ? steps.length + index : index;
  const step = steps[resolved];
  if (!step || step.kind !== 'point') throw new Error(`expected point at step index ${index}`);
  return step;
}

function cloneStep(step: FcStep): FcStep {
  return step.kind === 'point' ? clonePoint(step) : extruder(step.on);
}

function clonePoint(p: FcPoint): FcPoint {
  return point(p.x, p.y, p.z);
}

function polarToPoint(centre: FcPoint, radius: number, angleRad: number): FcPoint {
  return point(
    requireAxis('centre.x', centre.x) + radius * Math.cos(angleRad),
    requireAxis('centre.y', centre.y) + radius * Math.sin(angleRad),
    centre.z
  );
}

function moveSteps(steps: FcStep[], vector: FcVector): FcStep[] {
  return steps.map((step) => moveStep(step, vector));
}

function copyMoveSteps(steps: FcStep[], vector: FcVector, quantity: number): FcStep[] {
  const out: FcStep[] = [];
  for (let i = 0; i < quantity; i++) {
    out.push(...steps.map((step) => moveStep(step, scaleVector(vector, i))));
  }
  return out;
}

function moveStep(step: FcStep, vector: FcVector): FcStep {
  return step.kind === 'point' ? movePoint(step, vector) : cloneStep(step);
}

function movePoint(p: FcPoint, vector: FcVector): FcPoint {
  return point(moveAxis(p.x, vector.x), moveAxis(p.y, vector.y), moveAxis(p.z, vector.z));
}

function scaleVector(vector: FcVector, scale: number): FcVector {
  return {
    x: vector.x === undefined ? undefined : vector.x * scale,
    y: vector.y === undefined ? undefined : vector.y * scale,
    z: vector.z === undefined ? undefined : vector.z * scale,
  };
}

function moveAxis(value: number | null, delta: number | undefined): number | null {
  if (value === null || delta === undefined) return value;
  return value + delta;
}

function reflectXReverse(steps: FcStep[]): FcStep[] {
  return [...steps].reverse().map((step) => {
    if (step.kind === 'extruder') return cloneStep(step);
    return point(step.x, step.y === null ? null : -step.y, step.z);
  });
}

function rotateSteps(steps: FcStep[], centre: FcPoint, angleRad: number): FcStep[] {
  const cx = requireAxis('rotation centre x', centre.x);
  const cy = requireAxis('rotation centre y', centre.y);
  const c = Math.cos(angleRad);
  const s = Math.sin(angleRad);
  return steps.map((step) => {
    if (step.kind === 'extruder') return cloneStep(step);
    const x = requireAxis('rotate point x', step.x);
    const y = requireAxis('rotate point y', step.y);
    const dx = x - cx;
    const dy = y - cy;
    return point(cx + dx * c - dy * s, cy + dx * s + dy * c, step.z);
  });
}

function rad(degrees: number): number {
  return degrees * DEG;
}

function requireAxis(name: string, value: number | null): number {
  if (value === null || !Number.isFinite(value)) throw new Error(`${name} must be finite`);
  return value;
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

function nullableRound(value: number | null): number | null {
  return value === null ? null : round(value);
}

function round(value: number): number {
  return Math.round(value * 1e6) / 1e6;
}
