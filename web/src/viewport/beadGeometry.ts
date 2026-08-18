import * as THREE from 'three';
import type { Segment, SlicingFilterMode } from '../types/domain';

type Vec3 = [number, number, number];

const TAU = Math.PI * 2;
const UP: Vec3 = [0, 0, 1];

const vsub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const vlen = (a: Vec3): number => Math.hypot(a[0], a[1], a[2]);
const vnorm = (a: Vec3): Vec3 => {
  const l = vlen(a);
  return l > 1e-9 ? [a[0] / l, a[1] / l, a[2] / l] : [0, 0, 1];
};
const vcross = (a: Vec3, b: Vec3): Vec3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

export interface BeadFilterOptions {
  mode: SlicingFilterMode;
  targetSection: number;
  segmentSections: number[];
  activeFilterLayers?: number[];
  activeFilterFigures?: number[];
  activeFilterTurns?: number[];
}

export function buildStadiumBeadsGeometry(
  segments: Segment[],
  radialSegments = 8,
  filter?: BeadFilterOptions
): THREE.BufferGeometry {
  const pos: number[] = [];
  const nrm: number[] = [];

  const push = (p: Vec3, n: Vec3) => {
    pos.push(p[0], p[1], p[2]);
    nrm.push(n[0], n[1], n[2]);
  };

  const tri = (a: Vec3, b: Vec3, c: Vec3, na: Vec3, nb: Vec3, nc: Vec3) => {
    push(a, na);
    push(b, nb);
    push(c, nc);
  };

  const rings = Math.max(6, radialSegments);
  let cursor: Vec3 = [0, 0, 0];

  for (let idx = 0; idx < segments.length; idx++) {
    const seg = segments[idx];

    // Determine start and end points
    const rawStart = seg.start;
    const p0: Vec3 =
      rawStart && rawStart[0] !== null && !isNaN(rawStart[0] as number)
        ? (rawStart as Vec3)
        : cursor;

    const rawEnd = seg.end;
    const p1: Vec3 =
      rawEnd && rawEnd[0] !== null && !isNaN(rawEnd[0] as number)
        ? (rawEnd as Vec3)
        : p0;

    cursor = p1;

    // Check if extruding move
    const isTravel = seg.travel === true || seg.kind === 'travel';
    if (isTravel) continue;

    // Multi-modal slicing & multi-tag filter check
    if (filter) {
      if (filter.mode === 'upToSection' && filter.segmentSections) {
        const segSec = filter.segmentSections[idx];
        if (segSec > filter.targetSection) continue;
      } else if (filter.mode === 'singleSection' && filter.segmentSections) {
        const segSec = filter.segmentSections[idx];
        if (segSec !== filter.targetSection) continue;
      } else if (filter.mode === 'multiFilter') {
        const tags = seg.tags;
        if (filter.activeFilterLayers && filter.activeFilterLayers.length > 0) {
          if (!tags || tags.layer === undefined || !filter.activeFilterLayers.includes(tags.layer)) continue;
        }
        if (filter.activeFilterFigures && filter.activeFilterFigures.length > 0) {
          if (!tags || tags.figure === undefined || !filter.activeFilterFigures.includes(tags.figure)) continue;
        }
        if (filter.activeFilterTurns && filter.activeFilterTurns.length > 0) {
          if (!tags || tags.turn === undefined || !filter.activeFilterTurns.includes(tags.turn)) continue;
        }
      }
    }

    const d = vsub(p1, p0);
    const len = vlen(d);
    if (len < 1e-6) continue;

    const dir: Vec3 = [d[0] / len, d[1] / len, d[2] / len];
    let side = vcross(dir, UP);
    if (vlen(side) < 1e-5) side = vcross(dir, [1, 0, 0]);
    side = vnorm(side);
    const vn = vnorm(vcross(side, dir));

    const hw = (seg.width || 0.45) / 2;
    const hh = (seg.height || 0.2) / 2;

    const ring0: Vec3[] = [];
    const ring1: Vec3[] = [];
    const normals: Vec3[] = [];

    for (let i = 0; i < rings; i++) {
      const a = (i / rings) * TAU;
      const sx = Math.cos(a);
      const uz = Math.sin(a);
      const normal = vnorm([
        side[0] * sx + vn[0] * uz,
        side[1] * sx + vn[1] * uz,
        side[2] * sx + vn[2] * uz,
      ]);
      normals.push(normal);

      const offset: Vec3 = [
        side[0] * hw * sx + vn[0] * hh * uz,
        side[1] * hw * sx + vn[1] * hh * uz,
        side[2] * hw * sx + vn[2] * hh * uz,
      ];
      ring0.push([p0[0] + offset[0], p0[1] + offset[1], p0[2] + offset[2]]);
      ring1.push([p1[0] + offset[0], p1[1] + offset[1], p1[2] + offset[2]]);
    }

    for (let i = 0; i < rings; i++) {
      const j = (i + 1) % rings;
      // Cylinder wall quad (2 triangles)
      tri(ring0[i], ring1[i], ring1[j], normals[i], normals[i], normals[j]);
      tri(ring0[i], ring1[j], ring0[j], normals[i], normals[j], normals[j]);

      // Start cap
      const negDir: Vec3 = [-dir[0], -dir[1], -dir[2]];
      tri(p0, ring0[j], ring0[i], negDir, negDir, negDir);

      // End cap
      tri(p1, ring1[i], ring1[j], dir, dir, dir);
    }
  }

  const geom = new THREE.BufferGeometry();
  geom.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  geom.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return geom;
}
