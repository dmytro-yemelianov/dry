// Clean-room implementation of the star-polygon lattice families from Soyarslan et al.
// It emits Dry L1 ops only; the wasm engine still owns resolve/simulate/emit.
const TAU = Math.PI * 2;
const DEG = Math.PI / 180;
const EPS = 1e-9;

const STAR_POLYGON_FAMILIES = {
  M1: {
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

function normalizeStarPolygonAlpha(family, alphaDeg) {
  const spec = STAR_POLYGON_FAMILIES[family];
  if (!spec) throw new Error(`unknown star-polygon lattice family '${family}'`);
  finite('alphaDeg', alphaDeg);
  const max = spec.alphaUlDeg * 2;
  if (alphaDeg < 0 || alphaDeg > max) throw new Error(`${family} alphaDeg must be in 0..${max} degrees`);
  const mirrored = alphaDeg > spec.alphaUlDeg;
  return { inputDeg: alphaDeg, effectiveDeg: mirrored ? max - alphaDeg : alphaDeg, mirrored };
}

function starPolygonDentRadiusRatio(starSides, alphaDeg) {
  const phi = Math.PI / starSides;
  const t = Math.tan((alphaDeg * DEG) / 2);
  if (Math.abs(t) <= EPS) return 0;
  return t / (Math.sin(phi) + t * Math.cos(phi));
}

function starPolygonLatticeOps(options = {}) {
  const family = options.family ?? 'M1';
  const spec = STAR_POLYGON_FAMILIES[family];
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
  const ops = [
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

function buildPaths(spec, alpha, cols, rows, unit, outerRadiusRatio, includeConnectors) {
  const [a1, a2] = basisVectors(spec.basis, unit);
  const outerRadius = unit * outerRadiusRatio;
  const cells = [];
  const byKey = new Map();
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

  const paths = cells.map((cell) => ({ points: cell.loop, closed: true }));
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

function starPolygonPoints(sides, outerRadius, alphaDeg, center, rotation, handedness) {
  const dentRadius = outerRadius * starPolygonDentRadiusRatio(sides, alphaDeg);
  const points = [];
  for (let i = 0; i < sides * 2; i++) {
    const radius = i % 2 === 0 ? outerRadius : dentRadius;
    const angle = rotation + handedness * ((i * Math.PI) / sides);
    points.push({ x: center.x + radius * Math.cos(angle), y: center.y + radius * Math.sin(angle) });
  }
  return points;
}

function basisVectors(basis, unit) {
  if (basis === 'square') return [{ x: unit, y: 0 }, { x: 0, y: unit }];
  return [
    { x: Math.cos(Math.PI / 3) * unit, y: Math.sin(Math.PI / 3) * unit },
    { x: -Math.cos(Math.PI / 3) * unit, y: Math.sin(Math.PI / 3) * unit },
  ];
}

function appendLayerOps(ops, paths, z) {
  for (const path of paths) {
    if (path.points.length < 2) continue;
    const [start, ...rest] = path.points;
    ops.push({ op: 'extruder', on: false }, move(start, z), { op: 'extruder', on: true });
    for (const point of rest) ops.push(move(point, z));
  }
}

function move(point, z) {
  return { op: 'move', x: round(point.x), y: round(point.y), z: round(z) };
}

function centerPaths(paths, centerX, centerY) {
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

function orderPaths(paths) {
  const remaining = [...paths];
  const ordered = [];
  let cursor = null;
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

function preparePath(path, cursor) {
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

function reversePath(path) {
  if (!path.closed) return { closed: false, points: [...path.points].reverse() };
  const ring = path.points.slice(0, -1).reverse();
  return { closed: true, points: [...ring, ring[0]] };
}

function closestPair(a, b) {
  let best = [a[0], b[0]];
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

function cellKey(col, row) {
  return `${col},${row}`;
}

function add(a, b) {
  return { x: a.x + b.x, y: a.y + b.y };
}

function scale(a, k) {
  return { x: a.x * k, y: a.y * k };
}

function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y);
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

function round(value) {
  return Math.round(value * 1e6) / 1e6;
}

export {
  STAR_POLYGON_FAMILIES,
  normalizeStarPolygonAlpha,
  starPolygonDentRadiusRatio,
  starPolygonLatticeOps,
};
