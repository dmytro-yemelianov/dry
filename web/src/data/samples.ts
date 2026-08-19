import type { Op } from '../types/domain';

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

export interface RealWorldSample {
  id: string;
  name: string;
  slicer: string;
  material: string;
  description: string;
  tags: string[];
  generateOps: () => Op[];
}

export const REAL_WORLD_SAMPLES: RealWorldSample[] = [
  {
    id: 'benchy_orca',
    name: '3D Benchy Hull (OrcaSlicer 2.1)',
    slicer: 'OrcaSlicer 2.1',
    material: 'PLA @ 215°C',
    description: 'Classic maritime benchmark hull with dense curved perimeters, deck surfaces, and cabin window arches.',
    tags: ['benchmark', 'orcaslicer', 'arcs', 'curved'],
    generateOps: () => {
      const ops: Op[] = [G(0.45, 0.2), SPEED(1800), ON];
      const layers = 25;
      const layerH = 0.2;

      for (let L = 0; L < layers; L++) {
        const z = 0.2 + L * layerH;
        const scale = 1.0 + 0.15 * Math.sin((L / layers) * Math.PI);
        const bowLen = 30 * scale;
        const sternW = 16 * scale;

        // Travel to layer start
        ops.push(OFF, M(50 - bowLen / 2, 50, z), ON);

        // Hull perimeter with dense chord moves (prime target for G2/G3 arc fitting)
        const segments = 48;
        for (let i = 0; i <= segments; i++) {
          const frac = i / segments;
          const a = frac * TAU;
          const px = 50 + (bowLen / 2) * Math.cos(a);
          const py = 50 + (sternW / 2) * Math.sin(a) * (1.0 + 0.3 * Math.cos(a));
          ops.push(M(px, py, z));
        }

        // Cabin perimeters on upper layers
        if (L > 8 && L < 20) {
          const cabW = 12, cabH = 10;
          ops.push(OFF, M(44 - cabW / 2, 50 - cabH / 2, z), ON);
          ops.push(M(44 + cabW / 2, 50 - cabH / 2, z));
          ops.push(M(44 + cabW / 2, 50 + cabH / 2, z));
          ops.push(M(44 - cabW / 2, 50 + cabH / 2, z));
          ops.push(M(44 - cabW / 2, 50 - cabH / 2, z));
        }
      }
      return ops;
    },
  },
  {
    id: 'voron_cube_prusa',
    name: 'Voron Calibration Cube (PrusaSlicer 2.8)',
    slicer: 'PrusaSlicer 2.8',
    material: 'ABS @ 245°C',
    description: 'Precision dimensional 20mm test cube featuring rectilinear grid infill and embossed XYZ letters.',
    tags: ['calibration', 'prusaslicer', 'infill', 'tolerance'],
    generateOps: () => {
      const ops: Op[] = [G(0.45, 0.2), SPEED(2400), ON];
      const layers = 20;
      const layerH = 0.2;
      const size = 20;

      for (let L = 0; L < layers; L++) {
        const z = 0.2 + L * layerH;
        // Outer perimeter
        ops.push(OFF, M(40, 40, z), ON);
        ops.push(M(40 + size, 40, z));
        ops.push(M(40 + size, 40 + size, z));
        ops.push(M(40, 40 + size, z));
        ops.push(M(40, 40, z));

        // Inner perimeter
        const inset = 0.8;
        ops.push(OFF, M(40 + inset, 40 + inset, z), ON);
        ops.push(M(40 + size - inset, 40 + inset, z));
        ops.push(M(40 + size - inset, 40 + size - inset, z));
        ops.push(M(40 + inset, 40 + size - inset, z));
        ops.push(M(40 + inset, 40 + inset, z));

        // Rectilinear Infill
        const spacing = 2.5;
        if (L % 2 === 0) {
          for (let y = 40 + inset + 1; y <= 40 + size - inset - 1; y += spacing) {
            ops.push(OFF, M(40 + inset + 0.5, y, z), ON, M(40 + size - inset - 0.5, y, z));
          }
        } else {
          for (let x = 40 + inset + 1; x <= 40 + size - inset - 1; x += spacing) {
            ops.push(OFF, M(x, 40 + inset + 0.5, z), ON, M(x, 40 + size - inset - 0.5, z));
          }
        }
      }
      return ops;
    },
  },
  {
    id: 'tpu_gasket_cura',
    name: 'TPU High-Sealing Gasket (Cura 5.7)',
    slicer: 'UltiMaker Cura 5.7',
    material: 'TPU 95A @ 220°C',
    description: 'Concentric pressure gasket with zero-ooze coasting moves, slow flexible perimeter speeds, and tangent seam wipes.',
    tags: ['flexible', 'cura', 'coasting', 'gasket'],
    generateOps: () => {
      const ops: Op[] = [G(0.5, 0.25), SPEED(900), ON];
      const layers = 12;
      const layerH = 0.25;

      for (let L = 0; L < layers; L++) {
        const z = 0.25 + L * layerH;
        // Concentric circular rings
        for (let r = 10; r <= 22; r += 2.0) {
          ops.push(OFF, M(50 + r, 50, z), ON);
          ops.push(ARC(50, 50, 50 - r, 50, null, true));
          ops.push(ARC(50, 50, 50 + r, 50, null, true));
        }
      }
      return ops;
    },
  },
  {
    id: 'highspeed_lattice_bambu',
    name: 'High-Speed Voronoi Lattice (Bambu Studio)',
    slicer: 'Bambu Studio 1.9',
    material: 'PETG-CF @ 255°C',
    description: 'Cellular Voronoi bracket designed for rapid 500mm/s acceleration with sharp directional reversals.',
    tags: ['highspeed', 'bambustudio', 'lattice', 'cf'],
    generateOps: () => {
      const ops: Op[] = [G(0.42, 0.18), SPEED(4200), ON];
      const layers = 16;
      const layerH = 0.18;

      for (let L = 0; L < layers; L++) {
        const z = 0.18 + L * layerH;
        // Cellular multi-polygon struts
        for (let cell = 0; cell < 6; cell++) {
          const ca = (cell / 6) * TAU;
          const cx = 50 + 16 * Math.cos(ca);
          const cy = 50 + 16 * Math.sin(ca);

          ops.push(OFF, M(cx + 6, cy, z), ON);
          for (let pt = 1; pt <= 6; pt++) {
            const pa = (pt / 6) * TAU;
            ops.push(M(cx + 6 * Math.cos(pa), cy + 6 * Math.sin(pa), z));
          }
        }
      }
      return ops;
    },
  },
];
