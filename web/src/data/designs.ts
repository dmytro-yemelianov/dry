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
  op: 'arc',
  cx,
  cy,
  x,
  y,
  z,
  clockwise,
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

// Builder Parameter Helpers
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

const starLatticeParams = (alphaDeg: number): ParameterDef[] => [
  range('alphaDeg', 'Alpha Angle', alphaDeg, 0, 150, 1, 'deg'),
  range('segLength', 'Strut Length', 4.33, 1, 50, 0.01, 'mm'),
  range('cols', 'Columns', 10, 1, 15, 1, '1'),
  range('rows', 'Rows', 3, 1, 12, 1, '1'),
  range('layers', 'Layers', 2, 1, 24, 1, '1'),
  range('layerHeight', 'Layer Height', 0.2, 0.05, 2, 0.001, 'mm'),
];

const tpmsParams = (): ParameterDef[] => [
  range('cellSize', 'Cell Size', 22, 4, 80, 0.5, 'mm'),
  range('samplesPerCell', 'Samples/Cell', 16, 4, 64, 1, '1/cell'),
  range('cellsX', 'Cells X', 1, 1, 8, 1, '1'),
  range('cellsY', 'Cells Y', 1, 1, 8, 1, '1'),
  range('cellsZ', 'Cells Z', 1, 1, 8, 1, '1'),
  range('layerHeight', 'Layer Height', 0.28, 0.08, 1.4, 0.01, 'mm'),
  range('isoLevel', 'Iso Level', 0, -4, 4, 0.05, '1'),
];

export const DESIGN_DEFS: Record<string, DesignDef> = {
  // ---- Basics ----
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

  // ---- Curves & Splines ----
  arcs_mix: {
    key: 'arcs_mix',
    label: 'Arcs (Native G2/G3 Circular)',
    group: 'Curves & Geometries',
    tags: ['arc', 'curve'],
    params: [
      range('radius', 'Radius', 10, 2, 40, 0.5, 'mm'),
      range('gap', 'Gap', 10, 2, 50, 0.5, 'mm'),
      range('speed', 'Print Speed', 1800, 60, 12000, 10, 'mm/min'),
      zParam('z', 0.4),
    ],
    build: ({ radius = 10, gap = 10, speed = 1800, z = 0.4 }) => [
      G(0.6, 0.2), ON, M(radius * 2, 5, z), SPEED(speed),
      ARC(radius, 5, 0, 5, null, true),
      M(0, 5 + gap, z),
      ARC(radius, 5 + gap, radius * 2, 5 + gap, null, true),
    ],
  },
  rounded_rect: {
    key: 'rounded_rect',
    label: 'Rounded Rectangle (4 G3 Fillets)',
    group: 'Curves & Geometries',
    tags: ['arc', 'line', 'fillet'],
    params: [
      range('w', 'Width', 26, 4, 80, 0.5, 'mm'),
      range('h', 'Height', 18, 4, 80, 0.5, 'mm'),
      range('r', 'Corner Radius', 5, 0.5, 30, 0.5, 'mm'),
      ...centerParams(),
      zParam('z', 0.4),
    ],
    build: ({ w = 26, h = 18, r = 5, cx = 50, cy = 50, z = 0.4 }) => {
      const radius = Math.min(r, w / 2, h / 2);
      const ops: Op[] = [G(0.6, 0.2), ON];
      const x0 = cx - w / 2, x1 = cx + w / 2, y0 = cy - h / 2, y1 = cy + h / 2;
      ops.push(M(x0 + radius, y0, z), M(x1 - radius, y0, z));
      ops.push(ARC(x1 - radius, y0 + radius, x1, y0 + radius, null, false));
      ops.push(M(x1, y1 - radius, z));
      ops.push(ARC(x1 - radius, y1 - radius, x1 - radius, y1, null, false));
      ops.push(M(x0 + radius, y1, z));
      ops.push(ARC(x0 + radius, y1 - radius, x0, y1 - radius, null, false));
      ops.push(M(x0, y0 + radius, z));
      ops.push(ARC(x0 + radius, y0 + radius, x0 + radius, y0, null, false));
      return ops;
    },
  },
  spline_s_curve: {
    key: 'spline_s_curve',
    label: 'S-Curve (Catmull-Rom Spline)',
    group: 'Curves & Geometries',
    tags: ['spline', 'curve'],
    params: [
      range('length', 'Length', 64, 8, 100, 1, 'mm'),
      range('amp', 'Amplitude', 16, 1, 40, 0.5, 'mm'),
      range('points', 'Control Points', 6, 3, 24, 1, '1'),
      ...centerParams(),
      zParam(),
    ],
    build: ({ length = 64, amp = 16, points = 6, cx = 50, cy = 50, z = 0.2 }) => {
      const x0 = cx - length / 2;
      const ctrl: [number, number, number][] = [];
      for (let i = 0; i <= points; i++) {
        const f = i / points;
        ctrl.push([x0 + f * length, cy + amp * Math.sin(f * TAU), z]);
      }
      return [G(0.6, 0.2), OFF, M(ctrl[0][0], ctrl[0][1], z), ON, SPLINE(ctrl.slice(1))];
    },
  },
  hilbert: {
    key: 'hilbert',
    label: 'Hilbert Curve (Fractal Space-Filling)',
    group: 'Curves & Geometries',
    tags: ['fractal', 'parametric'],
    params: [
      range('order', 'Fractal Order', 4, 1, 7, 1, '1'),
      range('size', 'Grid Size', 40, 4, 90, 1, 'mm'),
      ...centerParams(),
      zParam(),
    ],
    build: ({ order = 4, size = 40, cx = 50, cy = 50, z = 0.2 }) => {
      const n = 1 << Math.round(order);
      const d2xy = (d: number): [number, number] => {
        let t = d, x = 0, y = 0;
        for (let s = 1; s < n; s *= 2) {
          const rx = 1 & ((t / 2) | 0), ry = 1 & (t ^ rx);
          if (ry === 0) {
            if (rx === 1) { x = s - 1 - x; y = s - 1 - y; }
            const tmp = x; x = y; y = tmp;
          }
          x += s * rx; y += s * ry; t = (t / 4) | 0;
        }
        return [x, y];
      };
      const ops: Op[] = [G(0.5, 0.2), TEMP(205), ON];
      for (let d = 0; d < n * n; d++) {
        const [gx, gy] = d2xy(d);
        ops.push(M(cx - size / 2 + (gx / (n - 1)) * size, cy - size / 2 + (gy / (n - 1)) * size, z));
      }
      return ops;
    },
  },
  rose: {
    key: 'rose',
    label: 'Rose Curve (Rhodonea Petals)',
    group: 'Curves & Geometries',
    tags: ['parametric', 'curve'],
    params: [
      range('k', 'Petal Factor k', 5, 1, 16, 1, '1'),
      range('a', 'Radius a', 18, 2, 45, 0.5, 'mm'),
      ...centerParams(),
      zParam(),
      sampleParam(360, 1600),
    ],
    build: ({ k = 5, a = 18, cx = 50, cy = 50, z = 0.2, samples = 360 }) => {
      const ops: Op[] = [G(0.5, 0.2), TEMP(205), ON];
      const maxTh = Math.round(k) % 2 === 0 ? TAU : Math.PI;
      for (let i = 0; i <= samples; i++) {
        const th = (i / samples) * maxTh, r = a * Math.cos(k * th);
        ops.push(M(cx + r * Math.cos(th), cy + r * Math.sin(th), z));
      }
      return ops;
    },
  },
  spirograph: {
    key: 'spirograph',
    label: 'Spirograph (Hypotrochoid)',
    group: 'Curves & Geometries',
    tags: ['parametric', 'curve'],
    params: [
      range('R', 'Outer Radius R', 22, 3, 50, 1, 'mm'),
      range('r', 'Inner Radius r', 7, 1, 30, 1, 'mm'),
      range('d', 'Pen Offset d', 11, 1, 40, 0.5, 'mm'),
      ...centerParams(),
      zParam(),
      sampleParam(720, 2400),
    ],
    build: ({ R = 22, r = 7, d = 11, cx = 50, cy = 50, z = 0.2, samples = 720 }) => {
      const ops: Op[] = [G(0.5, 0.2), TEMP(205), ON];
      const turns = r / gcd(R, r);
      for (let i = 0; i <= samples; i++) {
        const th = (i / samples) * TAU * turns;
        const x = (R - r) * Math.cos(th) + d * Math.cos(((R - r) / r) * th);
        const y = (R - r) * Math.sin(th) - d * Math.sin(((R - r) / r) * th);
        ops.push(M(cx + x, cy + y, z));
      }
      return ops;
    },
  },

  // ---- Infill & Multi-Layer ----
  infill_panel: {
    key: 'infill_panel',
    label: 'Infill Panel (Perimeter + Zig-Zag)',
    group: 'Infill & Multi-Layer',
    tags: ['infill', 'travel'],
    params: [
      range('w', 'Width', 26, 4, 80, 0.5, 'mm'),
      range('h', 'Height', 18, 4, 80, 0.5, 'mm'),
      range('gap', 'Hatch Spacing', 2, 0.5, 12, 0.1, 'mm'),
      ...centerParams(),
      zParam(),
    ],
    build: ({ w = 26, h = 18, gap = 2, cx = 50, cy = 50, z = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), ON];
      const x0 = cx - w / 2, x1 = cx + w / 2, y0 = cy - h / 2, y1 = cy + h / 2;
      ops.push(M(x0, y0, z), M(x1, y0, z), M(x1, y1, z), M(x0, y1, z), M(x0, y0, z));
      const bot = y0 + gap, top = y1 - gap;
      ops.push(OFF, M(x0 + gap, bot, z), ON);
      const xs: number[] = [];
      for (let x = x0 + gap; x <= x1 - gap + 1e-9; x += gap) xs.push(x);
      let y = bot;
      for (let i = 0; i < xs.length; i++) {
        const ny = y === bot ? top : bot;
        ops.push(M(xs[i], ny, z));
        y = ny;
        if (i < xs.length - 1) ops.push(M(xs[i + 1], y, z));
      }
      return ops;
    },
  },
  collinear_comb: {
    key: 'collinear_comb',
    label: 'Comb (Collinear Merge Demonstration)',
    group: 'Infill & Multi-Layer',
    tags: ['travel', 'optimize'],
    params: [
      range('rungs', 'Rungs', 6, 1, 24, 1, '1'),
      range('len', 'Length', 30, 2, 90, 0.5, 'mm'),
      range('pitch', 'Pitch', 4, 0.5, 20, 0.5, 'mm'),
      range('subdiv', 'Subdivisions / Rung', 5, 1, 24, 1, '1'),
      range('x0', 'Start X', 10, 0, 100, 0.5, 'mm'),
      range('y0', 'Start Y', 10, 0, 100, 0.5, 'mm'),
      zParam(),
    ],
    build: ({ rungs = 6, len = 30, pitch = 4, subdiv = 5, x0 = 10, y0 = 10, z = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), ON];
      let x = x0;
      ops.push(M(x, y0, z));
      for (let r = 0; r < rungs; r++) {
        const y = y0 + r * pitch;
        const dir = r % 2 === 0 ? 1 : -1;
        const xEnd = x + dir * len;
        for (let k = 1; k <= subdiv; k++) {
          ops.push(M(x + dir * len * (k / subdiv), y, z));
        }
        x = xEnd;
        if (r < rungs - 1) ops.push(M(x, y0 + (r + 1) * pitch, z));
      }
      return ops;
    },
  },
  honeycomb: {
    key: 'honeycomb',
    label: 'Honeycomb (Hexagonal Lattice Tiling)',
    group: 'Infill & Multi-Layer',
    tags: ['infill', 'travel'],
    params: [
      range('cols', 'Columns', 5, 1, 16, 1, '1'),
      range('rows', 'Rows', 4, 1, 16, 1, '1'),
      range('s', 'Cell Side', 4.5, 1, 15, 0.25, 'mm'),
      ...centerParams(),
      zParam(),
    ],
    build: ({ cols = 5, rows = 4, s = 4.5, cx = 50, cy = 50, z = 0.2 }) => {
      const ops: Op[] = [G(0.5, 0.2), TEMP(205), ON];
      const hex: [number, number][] = [];
      for (let i = 0; i < 6; i++) {
        const a = Math.PI / 6 + (i * TAU) / 6;
        hex.push([s * Math.cos(a), s * Math.sin(a)]);
      }
      const dx = s * Math.sqrt(3), dy = s * 1.5;
      const ox = cx - ((cols - 1) * dx) / 2, oy = cy - ((rows - 1) * dy) / 2;
      for (let row = 0; row < rows; row++) {
        for (let col = 0; col < cols; col++) {
          const hxc = ox + col * dx + (row % 2 ? dx / 2 : 0), hyc = oy + row * dy;
          ops.push(OFF, M(hxc + hex[0][0], hyc + hex[0][1], z), ON);
          for (let i = 1; i <= 6; i++) ops.push(M(hxc + hex[i % 6][0], hyc + hex[i % 6][1], z));
        }
      }
      return ops;
    },
  },
  corrugated_wall: {
    key: 'corrugated_wall',
    label: 'Corrugated Wall (Boustrophedon Multi-Layer)',
    group: 'Infill & Multi-Layer',
    tags: ['multi-layer', 'parametric'],
    params: [
      range('length', 'Wall Length', 44, 4, 100, 1, 'mm'),
      range('amp', 'Sine Amplitude', 4, 0.5, 20, 0.5, 'mm'),
      range('waves', 'Wave Count', 5, 1, 20, 1, '1'),
      range('layers', 'Layers', 10, 1, 80, 1, '1'),
      range('layerH', 'Layer Height', 0.3, 0.05, 2, 0.01, 'mm'),
      sampleParam(72, 500),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ length = 44, amp = 4, waves = 5, layers = 10, layerH = 0.3, samples = 72, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), TEMP(210), FAN(0.4), ON];
      const x0 = cx - length / 2;
      for (let L = 0; L < layers; L++) {
        const z = z0 + L * layerH;
        for (let i = 0; i <= samples; i++) {
          const f = L % 2 === 0 ? i / samples : 1 - i / samples;
          ops.push(M(x0 + f * length, cy + amp * Math.sin(f * TAU * waves), z));
        }
      }
      return ops;
    },
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
    label: 'Retraction Tower (Retract / Prime Cycles)',
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
  lattice: {
    key: 'lattice',
    label: 'Lattice Cube (Cross-Hatch Alternating 3D)',
    group: 'Infill & Multi-Layer',
    tags: ['infill', 'multi-layer', '3D'],
    params: [
      range('size', 'Cube Size', 28, 4, 90, 0.5, 'mm'),
      range('gap', 'Strut Gap', 4, 0.5, 20, 0.5, 'mm'),
      range('layers', 'Layers', 8, 1, 80, 1, '1'),
      range('layerH', 'Layer Height', 0.3, 0.05, 2, 0.01, 'mm'),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ size = 28, gap = 4, layers = 8, layerH = 0.3, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), TEMP(210), ON];
      const x0 = cx - size / 2, y0 = cy - size / 2, x1 = cx + size / 2, y1 = cy + size / 2;
      for (let L = 0; L < layers; L++) {
        const z = z0 + L * layerH;
        const lines: [[number, number], [number, number]][] = [];
        if (L % 2 === 0) for (let y = y0; y <= y1 + 1e-9; y += gap) lines.push([[x0, y], [x1, y]]);
        else for (let x = x0; x <= x1 + 1e-9; x += gap) lines.push([[x, y0], [x, y1]]);
        let flip = false;
        for (const [p, q] of lines) {
          const a = flip ? q : p, b = flip ? p : q;
          ops.push(OFF, M(a[0], a[1], z), ON, M(b[0], b[1], z));
          flip = !flip;
        }
      }
      return ops;
    },
  },

  // ---- Vases & Non-Planar ----
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
    label: 'Cone Vase (Non-Planar Tapering)',
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
    label: 'Twisted Vase (Fluted & Non-Planar)',
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
  star_tower: {
    key: 'star_tower',
    label: 'Star Tower (Stacked with Twist)',
    group: 'Vases & Non-Planar',
    tags: ['multi-layer', 'travel', '3D'],
    params: [
      range('points', 'Points', 5, 3, 16, 1, '1'),
      range('outer', 'Outer Radius', 16, 2, 45, 0.5, 'mm'),
      range('inner', 'Inner Radius', 7, 1, 35, 0.5, 'mm'),
      range('layers', 'Layers', 9, 1, 80, 1, '1'),
      range('layerH', 'Layer Height', 0.4, 0.05, 2, 0.01, 'mm'),
      ...centerParams(),
      zParam('z0', 0.2),
    ],
    build: ({ points = 5, outer = 16, inner = 7, layers = 9, layerH = 0.4, cx = 50, cy = 50, z0 = 0.2 }) => {
      const ops: Op[] = [G(0.6, 0.2), TEMP(210), ON];
      const m = Math.round(points) * 2;
      for (let L = 0; L < layers; L++) {
        const z = z0 + L * layerH, rot = L * 0.12, v: [number, number][] = [];
        for (let i = 0; i < m; i++) {
          const r = i % 2 === 0 ? outer : inner, a = (i / m) * TAU - Math.PI / 2 + rot;
          v.push([cx + r * Math.cos(a), cy + r * Math.sin(a)]);
        }
        ops.push(OFF, M(v[0][0], v[0][1], z), ON);
        for (let i = 1; i <= m; i++) ops.push(M(v[i % m][0], v[i % m][1], z));
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
    label: 'Lissajous Ribbon (3D Space)',
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

  // ---- Research Lattices (Auxetic Star-Polygons M1..M4) ----
  star_lattice_m1: {
    key: 'star_lattice_m1',
    label: 'M1 Auxetic Star-Polygon (30°)',
    group: 'Research Lattices',
    tags: ['research', 'lattice', 'M1', 'auxetic'],
    params: starLatticeParams(30),
    build: (params) => starPolygonLatticeOps({ family: 'M1', ...params }),
  },
  star_lattice_m2: {
    key: 'star_lattice_m2',
    label: 'M2 Auxetic Star-Polygon (60°)',
    group: 'Research Lattices',
    tags: ['research', 'lattice', 'M2', 'auxetic'],
    params: starLatticeParams(60),
    build: (params) => starPolygonLatticeOps({ family: 'M2', ...params }),
  },
  star_lattice_m3: {
    key: 'star_lattice_m3',
    label: 'M3 Auxetic Star-Polygon (30°)',
    group: 'Research Lattices',
    tags: ['research', 'lattice', 'M3', 'auxetic'],
    params: starLatticeParams(30),
    build: (params) => starPolygonLatticeOps({ family: 'M3', ...params }),
  },
  star_lattice_m4: {
    key: 'star_lattice_m4',
    label: 'M4 Auxetic Star-Polygon (45°)',
    group: 'Research Lattices',
    tags: ['research', 'lattice', 'M4', 'auxetic'],
    params: starLatticeParams(45),
    build: (params) => starPolygonLatticeOps({ family: 'M4', ...params }),
  },

  // ---- TPMS Minimal Surfaces ----
  tpms_gyroid: {
    key: 'tpms_gyroid',
    label: 'TPMS Gyroid Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'gyroid', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'gyroid', ...params }),
  },
  tpms_schwarz_p: {
    key: 'tpms_schwarz_p',
    label: 'TPMS Schwarz P Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'Schwarz P', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'schwarz-p', ...params }),
  },
  tpms_schwarz_d: {
    key: 'tpms_schwarz_d',
    label: 'TPMS Schwarz D Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'Schwarz D', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'schwarz-d', ...params }),
  },
  tpms_iwp: {
    key: 'tpms_iwp',
    label: 'TPMS Schoen I-WP Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'I-WP', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'iwp', ...params }),
  },
  tpms_neovius: {
    key: 'tpms_neovius',
    label: 'TPMS Neovius Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'Neovius', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'neovius', ...params }),
  },
  tpms_fks: {
    key: 'tpms_fks',
    label: 'TPMS Fischer-Koch S Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'FKS', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'fischer-koch-s', ...params }),
  },
  tpms_frd: {
    key: 'tpms_frd',
    label: 'TPMS Schoen F-RD Contours',
    group: 'TPMS Minimal Surfaces',
    tags: ['TPMS', 'F-RD', 'implicit'],
    params: tpmsParams(),
    build: (params) => generateTpmsOps({ surface: 'frd', ...params }),
  },
};

// FullControl gallery items converted to DesignDefs
type FullControlItem = {
  label?: string;
  name?: string;
  ops: Op[];
  tags?: string[];
  description?: string;
  links?: Array<[string, string]>;
};

// These are reconstructions of published FullControl notebooks. The generated data carries the
// provenance links that credit them, so it has to survive the conversion — dropping it here is how
// the gallery lost its attribution.
export const FULLCONTROL_GALLERY: Record<string, DesignDef> = Object.fromEntries(
  Object.entries(FC_DATA as Record<string, FullControlItem>).map(([key, item]) => {
    const title = item.label || item.name || key;
    return [
      `fc_${key}`,
      {
        key: `fc_${key}`,
        label: `FC: ${title}`,
        title,
        group: 'FullControl Gallery',
        tags: item.tags || ['fullcontrol', 'paper'],
        params: [],
        ops: item.ops,
        description: item.description,
        source: 'fullcontrol',
        sourceKey: key,
        links: item.links,
      },
    ];
  })
);
