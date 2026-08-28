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

/**
 * Convert Dry Toolpath IR into a 3D Wavefront .obj string representation.
 */
export function toolpathToObj(toolpath: Toolpath, includeTravel = false): string {
  const vertices: [number, number, number][] = [];
  const lines: [number, number][] = [];
  let lastIdx: number | null = null;

  for (const seg of toolpath.segments) {
    if (seg.travel && !includeTravel) {
      lastIdx = null;
      continue;
    }
    const end = seg.end;
    if (end && end.length === 3) {
      const x = end[0] ?? 0.0;
      const y = end[1] ?? 0.0;
      const z = end[2] ?? 0.0;
      vertices.push([x, y, z]);
      const currentIdx = vertices.length;
      if (lastIdx !== null) {
        lines.push([lastIdx, currentIdx]);
      }
      lastIdx = currentIdx;
    }
  }

  const objLines: string[] = [
    '# Dry 3D Toolpath Mesh (Wavefront OBJ)',
    `# Vertices: ${vertices.length}, Lines: ${lines.length}`,
    'o DryToolpath',
  ];

  for (const v of vertices) {
    objLines.push(`v ${v[0].toFixed(4)} ${v[1].toFixed(4)} ${v[2].toFixed(4)}`);
  }
  for (const l of lines) {
    objLines.push(`l ${l[0]} ${l[1]}`);
  }

  return objLines.join('\n') + '\n';
}

/**
 * Render a 2D (XY) vector SVG projection of the toolpath.
 */
export function toolpathToSvg(
  toolpath: Toolpath,
  width = 800,
  height = 800,
  padding = 40.0
): string {
  const coords: [number, number, boolean][] = [];
  let minX = Infinity,
    maxX = -Infinity;
  let minY = Infinity,
    maxY = -Infinity;

  for (const seg of toolpath.segments) {
    const end = seg.end;
    if (end && end.length >= 2 && end[0] !== null && end[1] !== null) {
      const x = end[0];
      const y = end[1];
      const isTravel = Boolean(seg.travel);
      coords.push([x, y, isTravel]);
      minX = Math.min(minX, x);
      maxX = Math.max(maxX, x);
      minY = Math.min(minY, y);
      maxY = Math.max(maxY, y);
    }
  }

  if (coords.length === 0) {
    minX = 0;
    maxX = 100;
    minY = 0;
    maxY = 100;
  }

  const spanX = Math.max(maxX - minX, 1.0);
  const spanY = Math.max(maxY - minY, 1.0);
  const scale = Math.min((width - 2 * padding) / spanX, (height - 2 * padding) / spanY);

  const tx = (x: number) => padding + (x - minX) * scale;
  const ty = (y: number) => height - (padding + (y - minY) * scale);

  const svgElements: string[] = [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}">`,
    '  <rect width="100%" height="100%" fill="#0f172a"/>',
    '  <g id="toolpath" stroke-linecap="round" stroke-linejoin="round">',
  ];

  let lastPt: [number, number] | null = null;
  for (const [x, y, isTravel] of coords) {
    const sx = tx(x);
    const sy = ty(y);
    if (lastPt !== null) {
      const color = isTravel ? '#475569' : '#38bdf8';
      const strokeWidth = isTravel ? '1' : '2';
      const dash = isTravel ? ' stroke-dasharray="3,3"' : '';
      svgElements.push(
        `    <line x1="${lastPt[0].toFixed(2)}" y1="${lastPt[1].toFixed(2)}" x2="${sx.toFixed(2)}" y2="${sy.toFixed(2)}" stroke="${color}" stroke-width="${strokeWidth}"${dash}/>`
      );
    }
    lastPt = [sx, sy];
  }

  svgElements.push('  </g>');
  svgElements.push('</svg>');
  return svgElements.join('\n') + '\n';
}

/**
 * Generate a standalone interactive 3D WebGL HTML viewer string.
 */
export function toolpathToInteractiveHtml(
  toolpath: Toolpath,
  title = 'Dry 3D Toolpath Viewer',
  bounds?: [number, number, number, number, number, number]
): string {
  const pointsData = [];
  for (const seg of toolpath.segments) {
    const end = seg.end;
    if (end && end.length === 3 && end[0] !== null && end[1] !== null && end[2] !== null) {
      pointsData.push({
        x: end[0],
        y: end[1],
        z: end[2],
        t: Boolean(seg.travel),
        s: seg.speed ?? 0.0,
      });
    }
  }

  const payload = JSON.stringify(pointsData);
  const boundsJson = JSON.stringify(bounds ?? [0, 250, 0, 250, 0, 250]);

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>${title}</title>
  <style>
    body { margin: 0; overflow: hidden; background: #0b0f19; font-family: sans-serif; color: #f8fafc; }
    #hud { position: absolute; top: 16px; left: 16px; background: rgba(15, 23, 42, 0.85); padding: 16px; border-radius: 8px; border: 1px solid #334155; font-size: 13px; z-index: 10; }
    h1 { margin: 0 0 8px 0; font-size: 16px; color: #38bdf8; }
  </style>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
</head>
<body>
  <div id="hud">
    <h1>${title}</h1>
    <div>Segments: <strong>${pointsData.length}</strong></div>
    <div>Controls: Left-Click = Rotate, Right = Pan</div>
  </div>
  <div id="canvas-container"></div>
  <script>
    const data = ${payload};
    const bounds = ${boundsJson};
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0b0f19);
    const camera = new THREE.PerspectiveCamera(45, window.innerWidth / window.innerHeight, 0.1, 2000);
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(window.innerWidth, window.innerHeight);
    document.getElementById('canvas-container').appendChild(renderer.domElement);
    const controls = new THREE.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;

    const grid = new THREE.GridHelper(bounds[1], 20, 0x334155, 0x1e293b);
    grid.position.set(bounds[1]/2, 0, bounds[3]/2);
    scene.add(grid);
    scene.add(new THREE.AxesHelper(30));

    if (data.length > 1) {
      const points = [];
      const colors = [];
      let minZ = Infinity, maxZ = -Infinity;
      data.forEach(p => { minZ = Math.min(minZ, p.z); maxZ = Math.max(maxZ, p.z); });
      const spanZ = Math.max(maxZ - minZ, 1.0);

      for (let i = 1; i < data.length; i++) {
        const p0 = data[i-1];
        const p1 = data[i];
        points.push(new THREE.Vector3(p0.x, p0.z, p0.y));
        points.push(new THREE.Vector3(p1.x, p1.z, p1.y));
        const t = (p1.z - minZ) / spanZ;
        const col = new THREE.Color();
        if (p1.t) col.setHex(0x475569);
        else col.setHSL(0.55 + t * 0.4, 0.9, 0.55);
        colors.push(col.r, col.g, col.b, col.r, col.g, col.b);
      }
      const geom = new THREE.BufferGeometry().setFromPoints(points);
      geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
      scene.add(new THREE.LineSegments(geom, new THREE.LineBasicMaterial({ vertexColors: true })));

      camera.position.set(bounds[1]/2 + 150, 180, bounds[3]/2 + 150);
      controls.target.set(bounds[1]/2, maxZ/2, bounds[3]/2);
      controls.update();
    }
    function animate() { requestAnimationFrame(animate); controls.update(); renderer.render(scene, camera); }
    animate();
  </script>
</body>
</html>`;
}

