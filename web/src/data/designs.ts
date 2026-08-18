import type { Op, DesignDef, ParameterDef, ResolveParams } from '../types/domain';
import { generateTpmsOps } from '../wasm/engine';
import { starPolygonLatticeOps } from '../../lattice-research.js';
import { FULLCONTROL_DESIGNS as FC_DATA } from '../../fullcontrol-gallery.generated.js';

export const RESOLVE_PARAMS: ResolveParams = {
  print_speed: 1000,
  travel_speed: 8000,
  dia: 1.75,
};

const TAU = Math.PI * 2;
const G = (w: number, h: number): Op => ({ op: 'geometry', width: w, height: h });
const ON: Op = { op: 'extruder', on: true };
const OFF: Op = { op: 'extruder', on: false };
const SPEED = (v: number): Op => ({ op: 'speed', print: v });
const M = (x?: number | null, y?: number | null, z?: number | null): Op => ({ op: 'move', x, y, z });
const ARC = (cx: number, cy: number, x: number | null, y: number | null, z: number | null, clockwise: boolean): Op => ({
  op: 'arc', cx, cy, x, y, z, clockwise
});
const TEMP = (c: number): Op => ({ op: 'temperature', nozzle: c });
const FAN = (v: number): Op => ({ op: 'fan', speed: v });
const RETRACT = (distance: number, speed: number): Op => ({ op: 'retract', distance, speed });
const UNRETRACT = (distance: number, speed: number): Op => ({ op: 'unretract', distance, speed });
const SPLINE = (points: [number, number, number][]): Op => ({ op: 'spline', points });

const gcd = (a: number, b: number): number => {
  let x = Math.abs(a);
  let y = Math.abs(b);
  while (y) {
    const t = y;
    y = x % y;
    x = t;
  }
  return x;
};

// Builder Helpers
const range = (
  id: string,
  label: string,
  defaultValue: number,
  min: number,
  max: number,
  step: number,
  unit = '1',
  title = ''
): ParameterDef => ({
  id,
  label,
  defaultValue,
  min,
  max,
  step,
  unit,
  title,
});

const centerParams = (): ParameterDef[] => [
  range('cx', 'Center X', 50, 0, 100, 0.5, 'mm'),
  range('cy', 'Center Y', 50, 0, 100, 0.5, 'mm'),
];
const zParam = (id = 'z', value = 0.2): ParameterDef => range(id, 'Z Height', value, 0.05, 20, 0.01, 'mm');
const sampleParam = (value = 360, max = 1200): ParameterDef => range('samples', 'Resolution', value, 12, max, 1, '1');

export const DESIGN_DEFS: Record<string, DesignDef> = {
  square: {
    key: 'square',
    label: 'Square (Line Moves)',
    group: 'Basics',
    tags: ['line', 'perimeter'],
    params: [range('side', 'Side Length', 10, 1, 80, 0.5, 'mm'), zParam()],
    build: ({ side = 10, z = 0.2 }) => [
      G(0.6, 0.2), ON, M(0, 0, z), M(side, 0, z), M(side, side, z), M(0, side, z), M(0, 0, z)
    ],
  },
  star: {
    key: 'star',
    label: 'Star (Continuous Stroke)',
    group: 'Basics',
    tags: ['line', 'parametric'],
    params: [
      range('points', 'Points', 5, 3, 16, 1, '1'),
      range('outer', 'Outer Radius', 20, 2, 45, 0.5, 'mm'),
      range('inner', 'Inner Radius', 8, 1, 35, 0.5, 'mm'),
      ...centerParams(),
      zParam(),
    ],
    build: ({ points = 5, outer = 20, inner = 8, cx = 50, cy = 50, z = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), ON];
      const verts: [number, number][] = [];
      for (let i = 0; i < points * 2; i++) {
        const r = i % 2 === 0 ? outer : inner;
        const a = (i / (points * 2)) * TAU - Math.PI / 2;
        verts.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
      }
      ops.push(M(verts[0][0], verts[0][1], z));
      for (let i = 1; i < verts.length; i++) ops.push(M(verts[i][0], verts[i][1], z));
      ops.push(M(verts[0][0], verts[0][1], z));
      return ops;
    },
  },
  spiral_vase: {
    key: 'spiral_vase',
    label: 'Spiral Vase (Continuous Helix)',
    group: 'Vases & Non-Planar',
    tags: ['non-planar', '3D', 'vase'],
    params: [
      range('radius', 'Radius', 15, 2, 45, 0.5, 'mm'),
      range('height', 'Height', 1.5, 0.2, 80, 0.1, 'mm'),
      range('layerH', 'Layer Height', 0.3, 0.05, 2, 0.01, 'mm'),
      range('perLayer', 'Samples / Layer', 24, 4, 96, 1, '1'),
      ...centerParams(),
    ],
    build: ({ radius = 15, height = 1.5, layerH = 0.3, perLayer = 24, cx = 50, cy = 50 }) => {
      const ops: Op[] = [G(0.6, 0.2), ON];
      const n = Math.round((height / layerH) * perLayer);
      for (let i = 0; i <= n; i++) {
        const frac = i / perLayer;
        const a = frac * TAU;
        ops.push(M(cx + radius * Math.cos(a), cy + radius * Math.sin(a), 0.2 + frac * layerH));
      }
      return ops;
    },
  },
  cone_vase: {
    key: 'cone_vase',
    label: 'Cone Vase (Non-Planar)',
    group: 'Vases & Non-Planar',
    tags: ['non-planar', '3D'],
    params: [
      range('r0', 'Base Radius', 18, 2, 45, 0.5, 'mm'),
      range('r1', 'Top Radius', 4, 1, 45, 0.5, 'mm'),
      range('height', 'Height', 12, 0.5, 100, 0.5, 'mm'),
      range('layerH', 'Layer Height', 0.4, 0.05, 2, 0.01, 'mm'),
      range('perLayer', 'Samples / Layer', 32, 4, 120, 1, '1'),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ r0 = 18, r1 = 4, height = 12, layerH = 0.4, perLayer = 32, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), ON];
      const turns = height / layerH;
      const n = Math.round(turns * perLayer);
      for (let i = 0; i <= n; i++) {
        const f = i / n, a = f * turns * TAU, r = r0 + (r1 - r0) * f;
        ops.push(M(cx + r * Math.cos(a), cy + r * Math.sin(a), z0 + f * height));
      }
      return ops;
    },
  },
  twisted_vase: {
    key: 'twisted_vase',
    label: 'Twisted Vase (Fluted)',
    group: 'Vases & Non-Planar',
    tags: ['non-planar', '3D', 'parametric'],
    params: [
      range('sides', 'Sides', 5, 3, 16, 1, '1'),
      range('radius', 'Radius', 14, 2, 45, 0.5, 'mm'),
      range('height', 'Height', 16, 0.5, 100, 0.5, 'mm'),
      range('layerH', 'Layer Height', 0.4, 0.05, 2, 0.01, 'mm'),
      range('twistDeg', 'Twist Angle', 360, -1080, 1080, 15, 'deg'),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ sides = 5, radius = 14, height = 16, layerH = 0.4, twistDeg = 360, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), TEMP(210), ON];
      const twist = (twistDeg / 360) * TAU;
      const layers = Math.round(height / layerH), n = layers * sides;
      for (let i = 0; i <= n; i++) {
        const f = i / n, ang = (i / sides) * TAU + twist * f, r = radius * (0.88 + 0.12 * Math.cos(sides * ang));
        ops.push(M(cx + r * Math.cos(ang), cy + r * Math.sin(ang), z0 + f * height));
      }
      return ops;
    },
  },
  torus_knot: {
    key: 'torus_knot',
    label: 'Torus Knot (3D Non-Planar)',
    group: 'Vases & Non-Planar',
    tags: ['non-planar', '3D', 'parametric'],
    params: [
      range('p', 'p-turns', 3, 1, 12, 1, '1'),
      range('q', 'q-turns', 2, 1, 12, 1, '1'),
      range('R', 'Major Radius', 15, 2, 45, 0.5, 'mm'),
      range('r', 'Minor Radius', 5, 0.5, 20, 0.5, 'mm'),
      sampleParam(480, 2400),
      ...centerParams(),
      range('zc', 'Center Z', 10, 0, 80, 0.5, 'mm'),
    ],
    build: ({ p = 3, q = 2, R = 15, r = 5, samples = 480, cx = 50, cy = 50, zc = 10 }) => {
      const ops: Op[] = [G(0.6, 0.2), TEMP(210), ON];
      for (let i = 0; i <= samples; i++) {
        const t = (i / samples) * TAU, rad = R + r * Math.cos(q * t);
        ops.push(M(cx + rad * Math.cos(p * t), cy + rad * Math.sin(p * t), zc + r * Math.sin(q * t)));
      }
      return ops;
    },
  },
  lissajous: {
    key: 'lissajous',
    label: 'Lissajous Ribbon (3D)',
    group: 'Vases & Non-Planar',
    tags: ['non-planar', '3D', 'parametric'],
    params: [
      range('a', 'Frequency A', 3, 1, 12, 1, '1'),
      range('b', 'Frequency B', 2, 1, 12, 1, '1'),
      range('deltaDeg', 'Phase Angle', 90, -360, 360, 5, 'deg'),
      range('A', 'Amplitude X', 18, 1, 45, 0.5, 'mm'),
      range('B', 'Amplitude Y', 18, 1, 45, 0.5, 'mm'),
      sampleParam(500, 2400),
      ...centerParams(),
      zParam('z0', 0.2),
      range('zRange', 'Z Span', 9, 0, 80, 0.5, 'mm'),
    ],
    build: ({ a = 3, b = 2, deltaDeg = 90, A = 18, B = 18, samples = 500, cx = 50, cy = 50, z0 = 0.2, zRange = 9 }) => {
      const ops: Op[] = [G(0.5, 0.2), TEMP(205), ON];
      const delta = (deltaDeg / 360) * TAU;
      for (let i = 0; i <= samples; i++) {
        const t = (i / samples) * TAU;
        ops.push(M(cx + A * Math.sin(a * t + delta), cy + B * Math.sin(b * t), z0 + (i / samples) * zRange));
      }
      return ops;
    },
  },
  tpms_gyroid: {
    key: 'tpms_gyroid',
    label: 'TPMS Gyroid Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'gyroid', 'implicit'],
    params: [
      range('cellSize', 'Cell Size', 22, 4, 80, 0.5, 'mm'),
      range('cellsX', 'Cells X', 1, 1, 8, 1, '1'),
      range('cellsY', 'Cells Y', 1, 1, 8, 1, '1'),
      range('cellsZ', 'Cells Z', 1, 1, 8, 1, '1'),
      range('layerHeight', 'Layer Height', 0.28, 0.08, 1.4, 0.01, 'mm'),
      range('isoLevel', 'Iso Level', 0, -4, 4, 0.05, '1'),
    ],
    build: (params) => generateTpmsOps({ surface: 'gyroid', ...params }),
  },
  tpms_schwarz_d: {
    key: 'tpms_schwarz_d',
    label: 'TPMS Schwarz D Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'Schwarz D', 'implicit'],
    params: [
      range('cellSize', 'Cell Size', 22, 4, 80, 0.5, 'mm'),
      range('cellsX', 'Cells X', 1, 1, 8, 1, '1'),
      range('cellsY', 'Cells Y', 1, 1, 8, 1, '1'),
      range('cellsZ', 'Cells Z', 1, 1, 8, 1, '1'),
      range('layerHeight', 'Layer Height', 0.28, 0.08, 1.4, 0.01, 'mm'),
      range('isoLevel', 'Iso Level', 0, -4, 4, 0.05, '1'),
    ],
    build: (params) => generateTpmsOps({ surface: 'schwarz-d', ...params }),
  },
  star_lattice_m1: {
    key: 'star_lattice_m1',
    label: 'M1 Auxetic Star-Polygon (30°)',
    group: 'Research Lattices',
    tags: ['research', 'lattice', 'M1', 'auxetic'],
    params: [
      range('alphaDeg', 'Alpha Angle', 30, 0, 150, 1, 'deg'),
      range('segLength', 'Strut Length', 4.33, 1, 50, 0.01, 'mm'),
      range('cols', 'Columns', 8, 1, 15, 1, '1'),
      range('rows', 'Rows', 3, 1, 12, 1, '1'),
      range('layers', 'Layers', 2, 1, 24, 1, '1'),
      range('layerHeight', 'Layer Height', 0.2, 0.05, 2, 0.001, 'mm'),
    ],
    build: (params) => starPolygonLatticeOps({ family: 'M1', ...params }),
  },
  layered_tower: {
    key: 'layered_tower',
    label: 'Layered Tower (10 Layers + Travel)',
    group: 'Infill & Multi-Layer',
    tags: ['multi-layer', 'travel'],
    params: [
      range('side', 'Side Length', 20, 2, 80, 0.5, 'mm'),
      range('layers', 'Layers', 10, 1, 80, 1, '1'),
      range('layerH', 'Layer Height', 0.3, 0.05, 2, 0.01, 'mm'),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ side = 20, layers = 10, layerH = 0.3, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), SPEED(1200)];
      const h = side / 2;
      const corner = [[cx - h, cy - h], [cx + h, cy - h], [cx + h, cy + h], [cx - h, cy + h]];
      for (let L = 0; L < layers; L++) {
        const z = z0 + L * layerH;
        ops.push(OFF, M(corner[0][0], corner[0][1], z), ON);
        for (let i = 1; i <= 4; i++) ops.push(M(corner[i % 4][0], corner[i % 4][1], z));
      }
      return ops;
    },
  },
  retraction_tower: {
    key: 'retraction_tower',
    label: 'Retraction Tower (Retract / Prime)',
    group: 'Infill & Multi-Layer',
    tags: ['multi-layer', 'travel', 'retract'],
    params: [
      range('side', 'Side Length', 16, 2, 80, 0.5, 'mm'),
      range('layers', 'Layers', 6, 1, 60, 1, '1'),
      range('layerH', 'Layer Height', 0.4, 0.05, 2, 0.01, 'mm'),
      range('retractDist', 'Retract Distance', 1.2, 0, 8, 0.1, 'mm'),
      range('retractSpeed', 'Retract Speed', 2400, 60, 9000, 60, 'mm/min'),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ side = 16, layers = 6, layerH = 0.4, retractDist = 1.2, retractSpeed = 2400, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), SPEED(1200)];
      const h = side / 2;
      const corner = [[cx - h, cy - h], [cx + h, cy - h], [cx + h, cy + h], [cx - h, cy + h]];
      for (let L = 0; L < layers; L++) {
        const z = z0 + L * layerH;
        ops.push(OFF, M(corner[0][0], corner[0][1], z));
        if (L > 0) ops.push(UNRETRACT(retractDist, retractSpeed));
        ops.push(ON);
        for (let i = 1; i <= 4; i++) ops.push(M(corner[i % 4][0], corner[i % 4][1], z));
        if (L < layers - 1) ops.push(RETRACT(retractDist, retractSpeed));
      }
      return ops;
    },
  },
};

// FullControl paper gallery items converted to DesignDefs
export const FULLCONTROL_GALLERY: Record<string, DesignDef> = Object.fromEntries(
  Object.entries(FC_DATA as Record<string, { label?: string; name?: string; ops: Op[]; tags?: string[] }>).map(([key, item]) => [
    `fc_${key}`,
    {
      key: `fc_${key}`,
      label: `FC: ${item.label || item.name || key}`,
      group: 'FullControl Gallery',
      tags: item.tags || ['fullcontrol', 'paper'],
      params: [],
      ops: item.ops,
    }
  ])
);
