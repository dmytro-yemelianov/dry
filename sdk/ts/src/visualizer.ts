// 3D coordinate frame triad axes & machine envelope visualizer helper.

import { Segment, Toolpath } from './ops.js';

export interface Point3D {
  x: number;
  y: number;
  z: number;
}

export interface AxisLine {
  axis: 'X' | 'Y' | 'Z' | 'Envelope';
  color: string;
  start: Point3D;
  end: Point3D;
}

export interface WireframeBox {
  lines: AxisLine[];
}

export interface PassSegmentGroup {
  role: string;
  color: string;
  segments: Segment[];
}

/**
 * Generate standard RGB 3D coordinate triad axes for visualization (Red=X, Green=Y, Blue=Z).
 */
export function renderFrameAxes(
  origin: Point3D = { x: 0, y: 0, z: 0 },
  length = 10.0
): AxisLine[] {
  return [
    {
      axis: 'X',
      color: '#ff0000',
      start: { ...origin },
      end: { x: origin.x + length, y: origin.y, z: origin.z },
    },
    {
      axis: 'Y',
      color: '#00ff00',
      start: { ...origin },
      end: { x: origin.x, y: origin.y + length, z: origin.z },
    },
    {
      axis: 'Z',
      color: '#0000ff',
      start: { ...origin },
      end: { x: origin.x, y: origin.y, z: origin.z + length },
    },
  ];
}

/**
 * Generate 12 3D wireframe bounding box edges representing a machine's physical build envelope.
 */
export function renderMachineEnvelope(
  bounds: [number, number, number, number, number, number],
  color = '#64748b'
): WireframeBox {
  const [minX, maxX, minY, maxY, minZ, maxZ] = bounds;

  const lines: AxisLine[] = [
    // Bottom rectangle (Z = minZ)
    { axis: 'Envelope', color, start: { x: minX, y: minY, z: minZ }, end: { x: maxX, y: minY, z: minZ } },
    { axis: 'Envelope', color, start: { x: maxX, y: minY, z: minZ }, end: { x: maxX, y: maxY, z: minZ } },
    { axis: 'Envelope', color, start: { x: maxX, y: maxY, z: minZ }, end: { x: minX, y: maxY, z: minZ } },
    { axis: 'Envelope', color, start: { x: minX, y: maxY, z: minZ }, end: { x: minX, y: minY, z: minZ } },

    // Top rectangle (Z = maxZ)
    { axis: 'Envelope', color, start: { x: minX, y: minY, z: maxZ }, end: { x: maxX, y: minY, z: maxZ } },
    { axis: 'Envelope', color, start: { x: maxX, y: minY, z: maxZ }, end: { x: maxX, y: maxY, z: maxZ } },
    { axis: 'Envelope', color, start: { x: maxX, y: maxY, z: maxZ }, end: { x: minX, y: maxY, z: maxZ } },
    { axis: 'Envelope', color, start: { x: minX, y: maxY, z: maxZ }, end: { x: minX, y: minY, z: maxZ } },

    // 4 Vertical pillars
    { axis: 'Envelope', color, start: { x: minX, y: minY, z: minZ }, end: { x: minX, y: minY, z: maxZ } },
    { axis: 'Envelope', color, start: { x: maxX, y: minY, z: minZ }, end: { x: maxX, y: minY, z: maxZ } },
    { axis: 'Envelope', color, start: { x: maxX, y: maxY, z: minZ }, end: { x: maxX, y: maxY, z: maxZ } },
    { axis: 'Envelope', color, start: { x: minX, y: maxY, z: minZ }, end: { x: minX, y: maxY, z: maxZ } },
  ];

  return { lines };
}

/**
 * Group toolpath segments by pass role with standard UI color palette.
 */
export function renderPassColorSegments(toolpath: Toolpath): PassSegmentGroup[] {
  const groups: Record<string, { role: string; color: string; segments: Segment[] }> = {
    travel: { role: 'Travel', color: '#ef4444', segments: [] },
    cutting: { role: 'Cutting / Extrusion', color: '#2563eb', segments: [] },
  };

  for (const seg of toolpath.segments) {
    if (seg.travel) {
      groups.travel.segments.push(seg);
    } else {
      groups.cutting.segments.push(seg);
    }
  }

  return Object.values(groups).filter((g) => g.segments.length > 0);
}
