// Browser copy of the SDK's star-polygon lattice generator. It follows the
// public FullControl Colab print-path recipe for M1..M4 and emits Dry L1 ops.
const DEG = Math.PI / 180;
const EPS = 1e-9;

const STAR_POLYGON_FAMILIES = {
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

function normalizeStarPolygonAlpha(family, alphaDeg) {
  const spec = STAR_POLYGON_FAMILIES[family];
  if (!spec) throw new Error(`unknown star-polygon lattice family '${family}'`);
  finite('alphaDeg', alphaDeg);
  const max = spec.alphaUlDeg * 2;
  if (alphaDeg < 0 || alphaDeg > max) throw new Error(`${family} alphaDeg must be in 0..${max} degrees`);
  const mirrored = alphaDeg > spec.alphaUlDeg;
  const effectiveDeg = mirrored ? max - alphaDeg : alphaDeg;
  const regime =
    Math.abs(effectiveDeg - spec.alphaUlDeg) <= EPS ? 'uniqueness-limit'
      : Math.abs(effectiveDeg - spec.alphaSplDeg) <= EPS ? 'star-limit'
        : effectiveDeg < spec.alphaSplDeg ? 'star'
          : 'convex';
  return { inputDeg: alphaDeg, effectiveDeg, mirrored, regime };
}

function starPolygonDentRadiusRatio(starSides, alphaDeg) {
  const n = integer('starSides', starSides, 3);
  finite('alphaDeg', alphaDeg);
  const phi = Math.PI / n;
  const t = Math.tan((alphaDeg * DEG) / 2);
  if (Math.abs(t) <= EPS) return 0;
  return t / (Math.sin(phi) + t * Math.cos(phi));
}

function starPolygonLatticeOps(options = {}) {
  const family = options.family ?? 'M1';
  if (!STAR_POLYGON_FAMILIES[family]) throw new Error(`unknown star-polygon lattice family '${family}'`);

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

  const ops = [
    { op: 'geometry', width: beadWidth, height: beadHeight },
    { op: 'temperature', nozzle: nozzleTemp },
    { op: 'speed', print: printSpeed },
  ];
  if (Math.abs(flow - 1) > EPS) ops.push({ op: 'flow', ratio: flow });

  appendStepOps(ops, shifted);
  return ops;
}

function buildColabLattice(family, alphaDeg, segLength, cols, rows, completeWidth) {
  if (family === 'M1') return buildM1(alphaDeg, segLength, cols, rows);
  if (family === 'M2') return buildM2(alphaDeg, segLength, cols, rows);
  if (family === 'M3') return buildM3(alphaDeg, segLength, cols, rows);
  return buildM4(alphaDeg, segLength, cols, rows, completeWidth);
}

function buildM1(alphaDeg, segLength, cols, rows) {
  const unit = [point(0, 0, 0)];
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-alphaDeg / 2)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(-60 + alphaDeg / 2)));
  unit.push(polarToPoint(pointAt(unit, -1), segLength, rad(120 - alphaDeg / 2)));

  const repeatOffsetX = requireAxis('repeatOffsetX', pointAt(unit, 3).x);
  const rowOffsetX = -(requireAxis('unit[2].x', pointAt(unit, 2).x) - requireAxis('unit[1].x', pointAt(unit, 1).x));
  const rowOffsetY = -(requireAxis('unit[1].y', pointAt(unit, 1).y) + requireAxis('unit[2].y', pointAt(unit, 2).y));

  const row1 = copyMoveSteps(unit, { x: repeatOffsetX }, cols);
  const row2 = reflectXReverse(row1);
  const rowPath = [...row1, ...row2];
  const lattice = [];
  const rowCount = rows * 2 - 1;
  for (let i = 0; i < rowCount; i++) {
    lattice.push(...moveSteps(rowPath, { x: i % 2 === 1 ? rowOffsetX : 0, y: rowOffsetY * i }));
  }
  return { steps: lattice, repeatOffsetX };
}

function buildM2(alphaDeg, segLength, cols, rows) {
  const devAngle = Math.atan((1 - Math.cos(rad(alphaDeg))) / (Math.sqrt(3) + Math.sin(rad(alphaDeg)))) / DEG;

  const unit = [point(0, 0, 0)];
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
  const back = [point(backStartX, backStartY)];
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

function buildM3(alphaDeg, segLength, cols, rows) {
  let unit = [point(0, 0, 0)];
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
  const back = [
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

function buildM4(alphaInputDeg, segLength, cols, rows, completeWidth) {
  const alphaDeg = Math.abs(alphaInputDeg - 150) <= EPS ? 120 : alphaInputDeg;
  const devAngle =
    Math.abs(
      Math.acos(
        -Math.sqrt(1 + Math.sin(2 * rad(alphaDeg))) /
          Math.sqrt(3 - 2 * Math.cos(rad(alphaDeg)) + 2 * Math.sin(rad(alphaDeg)))
      )
    ) / DEG;
  finite('M4 devAngle', devAngle);

  const unit = [point(0, 0, 0)];
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

  const back = [point(backStartX, backStartY)];
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

function appendStepOps(ops, steps) {
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

function returnLines(repeatOffsetX) {
  return [point(-repeatOffsetX, null, null), point(null, 0, null), point(0, null, null)];
}

function point(x = null, y = null, z = null) {
  return { kind: 'point', x, y, z };
}

function extruder(on) {
  return { kind: 'extruder', on };
}

function pointAt(steps, index) {
  const resolved = index < 0 ? steps.length + index : index;
  const step = steps[resolved];
  if (!step || step.kind !== 'point') throw new Error(`expected point at step index ${index}`);
  return step;
}

function cloneStep(step) {
  return step.kind === 'point' ? clonePoint(step) : extruder(step.on);
}

function clonePoint(p) {
  return point(p.x, p.y, p.z);
}

function polarToPoint(centre, radius, angleRad) {
  return point(
    requireAxis('centre.x', centre.x) + radius * Math.cos(angleRad),
    requireAxis('centre.y', centre.y) + radius * Math.sin(angleRad),
    centre.z
  );
}

function moveSteps(steps, vector) {
  return steps.map((step) => moveStep(step, vector));
}

function copyMoveSteps(steps, vector, quantity) {
  const out = [];
  for (let i = 0; i < quantity; i++) out.push(...steps.map((step) => moveStep(step, scaleVector(vector, i))));
  return out;
}

function moveStep(step, vector) {
  return step.kind === 'point' ? movePoint(step, vector) : cloneStep(step);
}

function movePoint(p, vector) {
  return point(moveAxis(p.x, vector.x), moveAxis(p.y, vector.y), moveAxis(p.z, vector.z));
}

function scaleVector(vector, scale) {
  return {
    x: vector.x === undefined ? undefined : vector.x * scale,
    y: vector.y === undefined ? undefined : vector.y * scale,
    z: vector.z === undefined ? undefined : vector.z * scale,
  };
}

function moveAxis(value, delta) {
  if (value === null || delta === undefined) return value;
  return value + delta;
}

function reflectXReverse(steps) {
  return [...steps].reverse().map((step) => {
    if (step.kind === 'extruder') return cloneStep(step);
    return point(step.x, step.y === null ? null : -step.y, step.z);
  });
}

function rotateSteps(steps, centre, angleRad) {
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

function rad(degrees) {
  return degrees * DEG;
}

function requireAxis(name, value) {
  if (value === null || !Number.isFinite(value)) throw new Error(`${name} must be finite`);
  return value;
}

function finite(name, value) {
  if (!Number.isFinite(value)) throw new Error(`${name} must be finite`);
  return value;
}

function positive(name, value) {
  finite(name, value);
  if (value <= 0) throw new Error(`${name} must be > 0`);
  return value;
}

function positiveOrZero(name, value) {
  finite(name, value);
  if (value < 0) throw new Error(`${name} must be >= 0`);
  return value;
}

function integer(name, value, min) {
  finite(name, value);
  if (!Number.isInteger(value) || value < min) throw new Error(`${name} must be an integer >= ${min}`);
  return value;
}

function nullableRound(value) {
  return value === null ? null : round(value);
}

function round(value) {
  return Math.round(value * 1e6) / 1e6;
}

export {
  STAR_POLYGON_FAMILIES,
  normalizeStarPolygonAlpha,
  starPolygonDentRadiusRatio,
  starPolygonLatticeOps,
};
