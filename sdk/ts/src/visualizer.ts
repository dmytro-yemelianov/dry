// 3D coordinate frame triad axes visualizer helper (Option D).

export interface Point3D {
  x: number;
  y: number;
  z: number;
}

export interface AxisLine {
  axis: 'X' | 'Y' | 'Z';
  color: string;
  start: Point3D;
  end: Point3D;
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
