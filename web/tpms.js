// TPMS contour code generator for the browser gallery. It evaluates an implicit surface,
// slices each Z layer with marching squares, stitches contours, and emits Dry L1 ops.
const TAU = Math.PI * 2;
const EPS = 1e-9;
const DEFAULT_LAYER_HEIGHT = 0.28;
const DEFAULT_ADAPTIVE_MIN_LAYER_HEIGHT = 0.14;
const DEFAULT_ADAPTIVE_MAX_LAYER_HEIGHT = 0.32;
const DEFAULT_ADAPTIVE_MAX_LENGTH_DELTA = 0.35;
const DEFAULT_ADAPTIVE_MAX_POINT_DELTA = 0.45;
const DEFAULT_ADAPTIVE_MAX_DEPTH = 4;
const DEFAULT_MAX_FIELD_SAMPLES = 6_000_000;
const TPMS_PATH_MODES = {
  LINEAR: 'linear',
  SAFE_ARCS: 'safe-arcs',
};
const DEFAULT_PATH_MODE = TPMS_PATH_MODES.SAFE_ARCS;
const DEFAULT_ARC_FIT_TOLERANCE = 0.035;
const DEFAULT_ARC_FIT_MIN_POINTS = 4;
const DEFAULT_ARC_FIT_MAX_POINTS = 24;
const DEFAULT_ARC_FIT_MAX_SWEEP = Math.PI * 0.9;

const TPMS_SURFACES = {
  gyroid: {
    label: 'Gyroid',
    equation: 'sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)',
    field: (x, y, z) => Math.sin(x) * Math.cos(y) + Math.sin(y) * Math.cos(z) + Math.sin(z) * Math.cos(x),
  },
  'schwarz-p': {
    label: 'Schwarz P',
    equation: 'cos(x) + cos(y) + cos(z)',
    field: (x, y, z) => Math.cos(x) + Math.cos(y) + Math.cos(z),
  },
  'schwarz-d': {
    label: 'Schwarz D / Diamond',
    equation: 'sin(x)sin(y)sin(z) + sin(x)cos(y)cos(z) + cos(x)sin(y)cos(z) + cos(x)cos(y)sin(z)',
    field: (x, y, z) =>
      Math.sin(x) * Math.sin(y) * Math.sin(z) +
      Math.sin(x) * Math.cos(y) * Math.cos(z) +
      Math.cos(x) * Math.sin(y) * Math.cos(z) +
      Math.cos(x) * Math.cos(y) * Math.sin(z),
  },
  iwp: {
    label: 'Schoen I-WP',
    equation: '2(cos(x)cos(y)+cos(y)cos(z)+cos(z)cos(x)) - (cos(2x)+cos(2y)+cos(2z))',
    field: (x, y, z) =>
      2 * (Math.cos(x) * Math.cos(y) + Math.cos(y) * Math.cos(z) + Math.cos(z) * Math.cos(x)) -
      (Math.cos(2 * x) + Math.cos(2 * y) + Math.cos(2 * z)),
  },
  neovius: {
    label: 'Neovius',
    equation: '3(cos(x)+cos(y)+cos(z)) + 4cos(x)cos(y)cos(z)',
    field: (x, y, z) => 3 * (Math.cos(x) + Math.cos(y) + Math.cos(z)) + 4 * Math.cos(x) * Math.cos(y) * Math.cos(z),
  },
  'fischer-koch-s': {
    label: 'Fischer-Koch S',
    equation: 'cos(2x)sin(y)cos(z) + cos(2y)sin(z)cos(x) + cos(2z)sin(x)cos(y)',
    field: (x, y, z) =>
      Math.cos(2 * x) * Math.sin(y) * Math.cos(z) +
      Math.cos(2 * y) * Math.sin(z) * Math.cos(x) +
      Math.cos(2 * z) * Math.sin(x) * Math.cos(y),
  },
  'fischer-koch-y': {
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
    label: 'Schoen F-RD',
    equation: '4cos(x)cos(y)cos(z) - (cos(2x)cos(2y)+cos(2y)cos(2z)+cos(2z)cos(2x))',
    field: (x, y, z) =>
      4 * Math.cos(x) * Math.cos(y) * Math.cos(z) -
      (Math.cos(2 * x) * Math.cos(2 * y) +
        Math.cos(2 * y) * Math.cos(2 * z) +
        Math.cos(2 * z) * Math.cos(2 * x)),
  },
  lidinoid: {
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

function tpmsOps(options = {}) {
  const surface = options.surface ?? 'gyroid';
  const spec = TPMS_SURFACES[surface];
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
  const maxFieldSamples = positiveOrInfinity('maxFieldSamples', options.maxFieldSamples ?? DEFAULT_MAX_FIELD_SAMPLES);
  const pathMode = tpmsPathMode(options.pathMode ?? DEFAULT_PATH_MODE);
  const arcFitTolerance = positive('arcFitTolerance', options.arcFitTolerance ?? DEFAULT_ARC_FIT_TOLERANCE);
  const arcFitMinPoints = integer('arcFitMinPoints', options.arcFitMinPoints ?? DEFAULT_ARC_FIT_MIN_POINTS, 4);
  const arcFitMaxPoints = integer('arcFitMaxPoints', options.arcFitMaxPoints ?? DEFAULT_ARC_FIT_MAX_POINTS, arcFitMinPoints);
  const arcFitMaxSweep = positive('arcFitMaxSweep', options.arcFitMaxSweep ?? DEFAULT_ARC_FIT_MAX_SWEEP);
  if (adaptiveMinLayerHeight - adaptiveMaxLayerHeight > EPS) {
    throw new Error('adaptiveMinLayerHeight must be <= adaptiveMaxLayerHeight');
  }
  if (arcFitMinPoints > arcFitMaxPoints) {
    throw new Error('arcFitMinPoints must be <= arcFitMaxPoints');
  }

  const width = cellsX * cellSize;
  const depth = cellsY * cellSize;
  const height = cellsZ * cellSize;
  const nx = cellsX * samplesPerCell;
  const ny = cellsY * samplesPerCell;
  assertTpmsBudget({
    nx,
    ny,
    height,
    sliceHeight: adaptive ? Math.min(layerHeight, adaptiveMinLayerHeight) : layerHeight,
    maxFieldSamples,
  });
  const dx = width / nx;
  const dy = depth / ny;
  const minPathLength = positiveOrZero('minPathLength', options.minPathLength ?? Math.min(dx, dy));
  const perimeterInset = Math.min(
    positiveOrZero('perimeterInset', options.perimeterInset ?? beadWidth),
    Math.max(0, width / 2 - EPS),
    Math.max(0, depth / 2 - EPS)
  );
  const ops = [
    { op: 'geometry', width: beadWidth, height: beadHeight },
    { op: 'temperature', nozzle: nozzleTemp },
    { op: 'speed', print: printSpeed },
  ];
  if (Math.abs(flow - 1) > EPS) ops.push({ op: 'flow', ratio: flow });

  const buildLayer = (zLocal) => {
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

  let previousLocal = null;
  for (const slice of layerSlices) {
    const zLocal = slice.zLocal;
    const z = z0 + zLocal;
    if (perimeter) {
      const rectLocal = rectanglePath(width, depth, perimeterInset);
      const rect = rectLocal.map((p) => ({ x: p.x - width / 2 + centerX, y: p.y - depth / 2 + centerY }));
      appendPath(ops, rect, z, pathMode, { tolerance: arcFitTolerance, minPoints: arcFitMinPoints, maxPoints: arcFitMaxPoints, maxSweep: arcFitMaxSweep });
      previousLocal = rectLocal[rectLocal.length - 1];
    }
    const paths = orderPaths(slice.paths, previousLocal);
    for (const path of paths) {
      const points = path.points.map((p) => ({ x: p.x - width / 2 + centerX, y: p.y - depth / 2 + centerY }));
      appendPath(ops, points, z, pathMode, { tolerance: arcFitTolerance, minPoints: arcFitMinPoints, maxPoints: arcFitMaxPoints, maxSweep: arcFitMaxSweep });
      previousLocal = path.points[path.points.length - 1];
    }
  }
  ops.push({ op: 'extruder', on: false });
  return ops;
}

function buildLayerSlices(height, layerHeight, buildLayer, adaptive) {
  const baseZ = baseLayerZs(height, layerHeight);
  const cache = new Map();
  const sample = (zLocal) => {
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
      const inserted = [];
      refineAdaptiveLayerInterval(a, b, sample, adaptive, inserted, 0);
      slices.push(...inserted);
    }
    slices.push(b);
  }
  return slices;
}

function baseLayerZs(height, layerHeight) {
  const zValues = [0];
  for (let z = layerHeight; z < height - EPS; z += layerHeight) zValues.push(round(z));
  if (height > EPS && Math.abs(zValues[zValues.length - 1] - height) > EPS) zValues.push(round(height));
  return zValues;
}

function refineAdaptiveLayerInterval(a, b, sample, options, out, depth) {
  if (!needsAdaptiveLayer(a, b, options, depth)) return;
  const midZ = round((a.zLocal + b.zLocal) / 2);
  if (midZ - a.zLocal < options.minLayerHeight - EPS || b.zLocal - midZ < options.minLayerHeight - EPS) return;
  const mid = sample(midZ);
  refineAdaptiveLayerInterval(a, mid, sample, options, out, depth + 1);
  out.push(mid);
  refineAdaptiveLayerInterval(mid, b, sample, options, out, depth + 1);
}

function needsAdaptiveLayer(a, b, options, depth) {
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

function layerSlice(zLocal, paths) {
  return {
    zLocal: round(zLocal),
    paths,
    pathCount: paths.length,
    pointCount: paths.reduce((total, path) => total + path.points.length, 0),
    length: paths.reduce((total, path) => total + pathLength(path.points), 0),
  };
}

function assertTpmsBudget({ nx, ny, height, sliceHeight, maxFieldSamples }) {
  if (!Number.isFinite(maxFieldSamples)) return;
  const estimatedLayers = Math.ceil(height / sliceHeight) + 1;
  const estimatedFieldSamples = (nx + 1) * (ny + 1) * estimatedLayers;
  if (estimatedFieldSamples <= maxFieldSamples) return;
  throw new Error(
    `TPMS resolution budget exceeded (${Math.ceil(estimatedFieldSamples)} field samples > ${Math.ceil(maxFieldSamples)}). ` +
      'Reduce samples/cells/cell height or raise the layer height.'
  );
}

function rectanglePath(width, depth, inset) {
  return [
    { x: inset, y: inset },
    { x: width - inset, y: inset },
    { x: width - inset, y: depth - inset },
    { x: inset, y: depth - inset },
    { x: inset, y: inset },
  ];
}

function marchingSquaresLayer(spec, isoLevel, width, depth, cellSize, nx, ny, zLocal, phaseX, phaseY, phaseZ) {
  const dx = width / nx;
  const dy = depth / ny;
  const values = new Array((nx + 1) * (ny + 1));
  const valueAt = (i, j) => values[j * (nx + 1) + i];
  for (let j = 0; j <= ny; j++) {
    for (let i = 0; i <= nx; i++) {
      const x = ((i * dx) / cellSize + phaseX) * TAU;
      const y = ((j * dy) / cellSize + phaseY) * TAU;
      const z = (zLocal / cellSize + phaseZ) * TAU;
      values[j * (nx + 1) + i] = spec.field(x, y, z) - isoLevel;
    }
  }

  const segments = [];
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
      const crossings = [];
      if (crosses(v00, v10)) crossings.push(interpolate(p00, p10, v00, v10));
      if (crosses(v10, v11)) crossings.push(interpolate(p10, p11, v10, v11));
      if (crosses(v11, v01)) crossings.push(interpolate(p11, p01, v11, v01));
      if (crosses(v01, v00)) crossings.push(interpolate(p01, p00, v01, v00));
      if (crossings.length === 2) {
        segments.push({ a: crossings[0], b: crossings[1] });
      } else if (crossings.length === 4) {
        const xc = (((i + 0.5) * dx) / cellSize + phaseX) * TAU;
        const yc = (((j + 0.5) * dy) / cellSize + phaseY) * TAU;
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

function stitchSegments(segments) {
  const unused = new Set(segments.map((_, i) => i));
  const endpoints = new Map();
  for (let i = 0; i < segments.length; i++) {
    addEndpoint(endpoints, pointKey(segments[i].a), i);
    addEndpoint(endpoints, pointKey(segments[i].b), i);
  }

  const paths = [];
  while (unused.size) {
    const first = unused.values().next().value;
    unused.delete(first);
    const path = [segments[first].a, segments[first].b];
    extendPath(path, segments, endpoints, unused, false);
    extendPath(path, segments, endpoints, unused, true);
    paths.push({ points: dedupeConsecutive(path) });
  }
  return paths;
}

function extendPath(path, segments, endpoints, unused, atStart) {
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

function orderPaths(paths, cursor) {
  const remaining = [...paths];
  const ordered = [];
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

function orientPath(path, cursor) {
  if (!cursor || path.points.length < 2) return path;
  const first = path.points[0];
  const last = path.points[path.points.length - 1];
  return distance(cursor, last) < distance(cursor, first) ? { points: [...path.points].reverse() } : path;
}

function appendPath(ops, points, z, pathMode = TPMS_PATH_MODES.LINEAR, arcOptions = {}) {
  const [start, ...rest] = points;
  ops.push({ op: 'extruder', on: false }, move(start, z), { op: 'extruder', on: true });
  if (pathMode === TPMS_PATH_MODES.SAFE_ARCS) {
    appendArcFittedPath(ops, points, z, arcOptions);
    return;
  }
  for (const point of rest) ops.push(move(point, z));
}

function move(point, z) {
  return { op: 'move', x: round(point.x), y: round(point.y), z: round(z) };
}

function arcMove(fit, point, z) {
  return {
    op: 'arc',
    cx: round(fit.cx),
    cy: round(fit.cy),
    x: round(point.x),
    y: round(point.y),
    z: round(z),
    clockwise: fit.clockwise,
  };
}

function appendArcFittedPath(ops, points, z, options) {
  let i = 0;
  while (i < points.length - 1) {
    const fit = bestArcFit(points, i, options);
    if (fit) {
      ops.push(arcMove(fit, points[fit.end], z));
      i = fit.end;
    } else {
      i += 1;
      ops.push(move(points[i], z));
    }
  }
}

function bestArcFit(points, start, options) {
  const minPoints = options.minPoints ?? DEFAULT_ARC_FIT_MIN_POINTS;
  const maxPoints = options.maxPoints ?? DEFAULT_ARC_FIT_MAX_POINTS;
  const last = Math.min(points.length - 1, start + maxPoints - 1);
  let best = null;
  for (let end = start + minPoints - 1; end <= last; end++) {
    const candidate = fitArcCandidate(points, start, end, options);
    if (candidate) best = candidate;
    else if (end > start + minPoints - 1 || best) break;
  }
  return best;
}

function fitArcCandidate(points, start, end, options) {
  const tolerance = options.tolerance ?? DEFAULT_ARC_FIT_TOLERANCE;
  const maxSweep = options.maxSweep ?? DEFAULT_ARC_FIT_MAX_SWEEP;
  if (end - start + 1 < (options.minPoints ?? DEFAULT_ARC_FIT_MIN_POINTS)) return null;

  const first = points[start];
  const mid = points[Math.floor((start + end) / 2)];
  const last = points[end];
  const centre = circumcenter(first, mid, last);
  if (!centre) return null;
  const radius = distance(first, centre);
  if (radius <= tolerance || radius > 10000) return null;

  const startAngle = Math.atan2(first.y - centre.y, first.x - centre.x);
  const endAngle = Math.atan2(last.y - centre.y, last.x - centre.x);
  let sign = 0;
  let maxRadialError = 0;
  let maxChordDeviation = 0;

  for (let i = start + 1; i <= end; i++) {
    const turn = turnSign(centre, points[i - 1], points[i]);
    if (Math.abs(turn) <= 1e-10) return null;
    const currentSign = Math.sign(turn);
    if (sign === 0) sign = currentSign;
    else if (currentSign !== sign) return null;
  }
  if (sign === 0) return null;

  const clockwise = sign < 0;
  const sweep = directedSweep(startAngle, endAngle, clockwise);
  if (sweep <= EPS || sweep > maxSweep) return null;

  for (let i = start; i <= end; i++) {
    const point = points[i];
    const radialError = Math.abs(distance(point, centre) - radius);
    if (radialError > tolerance) return null;
    maxRadialError = Math.max(maxRadialError, radialError);
    const progress = directedSweep(startAngle, Math.atan2(point.y - centre.y, point.x - centre.x), clockwise);
    if (progress > sweep + 1e-6 && progress < TAU - 1e-6) return null;
    maxChordDeviation = Math.max(maxChordDeviation, distanceToLine(point, first, last));
  }

  if (maxChordDeviation < tolerance * 1.5) return null;
  return { end, cx: centre.x, cy: centre.y, clockwise, error: maxRadialError };
}

function circumcenter(a, b, c) {
  const d = 2 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
  if (Math.abs(d) < 1e-10) return null;
  const a2 = a.x * a.x + a.y * a.y;
  const b2 = b.x * b.x + b.y * b.y;
  const c2 = c.x * c.x + c.y * c.y;
  return {
    x: (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
    y: (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
  };
}

function turnSign(center, a, b) {
  return (a.x - center.x) * (b.y - center.y) - (a.y - center.y) * (b.x - center.x);
}

function directedSweep(startAngle, endAngle, clockwise) {
  let sweep = clockwise ? startAngle - endAngle : endAngle - startAngle;
  sweep %= TAU;
  if (sweep <= 0) sweep += TAU;
  return sweep;
}

function distanceToLine(point, a, b) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len = Math.hypot(dx, dy);
  if (len <= EPS) return distance(point, a);
  return Math.abs(dy * point.x - dx * point.y + b.x * a.y - b.y * a.x) / len;
}

function interpolate(a, b, va, vb) {
  const t = va / (va - vb);
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
}

function crosses(a, b) {
  return (a < 0 && b > 0) || (a > 0 && b < 0);
}

function scrubZero(value) {
  if (Math.abs(value) > EPS) return value;
  return value < 0 ? -EPS : EPS;
}

function addEndpoint(map, key, index) {
  const list = map.get(key);
  if (list) list.push(index);
  else map.set(key, [index]);
}

function dedupeConsecutive(points) {
  const out = [];
  for (const point of points) {
    const prev = out[out.length - 1];
    if (!prev || distance(prev, point) > 1e-7) out.push(point);
  }
  return out;
}

function pathLength(points) {
  let total = 0;
  for (let i = 1; i < points.length; i++) total += distance(points[i - 1], points[i]);
  return total;
}

function pointKey(point) {
  return `${Math.round(point.x * 1e6)},${Math.round(point.y * 1e6)}`;
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

function positiveOrInfinity(name, value) {
  if (value === Infinity) return value;
  return positive(name, value);
}

function integer(name, value, min) {
  finite(name, value);
  if (!Number.isInteger(value) || value < min) throw new Error(`${name} must be an integer >= ${min}`);
  return value;
}

function tpmsPathMode(value) {
  if (Object.values(TPMS_PATH_MODES).includes(value)) return value;
  throw new Error(`unknown TPMS path mode '${value}'`);
}

function round(value) {
  return Math.round(value * 1e6) / 1e6;
}

export { TPMS_PATH_MODES, TPMS_SURFACES, tpmsOps };
