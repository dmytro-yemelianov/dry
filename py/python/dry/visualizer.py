"""3D Toolpath Visualizer and Model Exporter for Dry."""

import json
from typing import Any, Dict, List, Optional, Tuple, Sequence


def toolpath_to_obj(ir: Dict[str, Any], include_travel: bool = False) -> str:
    """Convert Dry Toolpath IR into a 3D Wavefront .obj string."""
    segments = ir.get("segments", [])
    vertices: List[Tuple[float, float, float]] = []
    lines: List[Tuple[int, int]] = []

    last_idx: Optional[int] = None

    for seg in segments:
        is_travel = seg.get("travel", False)
        if is_travel and not include_travel:
            last_idx = None
            continue

        end = seg.get("end")
        if end and len(end) == 3:
            x, y, z = end[0] or 0.0, end[1] or 0.0, end[2] or 0.0
            vertices.append((x, y, z))
            current_idx = len(vertices)

            if last_idx is not None:
                lines.append((last_idx, current_idx))
            last_idx = current_idx

    obj_lines = [
        "# Dry 3D Toolpath Mesh (Wavefront OBJ)",
        f"# Vertices: {len(vertices)}, Line Segments: {len(lines)}",
        "o DryToolpath",
    ]

    for v in vertices:
        obj_lines.append(f"v {v[0]:.4f} {v[1]:.4f} {v[2]:.4f}")

    for l in lines:
        obj_lines.append(f"l {l[0]} {l[1]}")

    return "\n".join(obj_lines) + "\n"


def toolpath_to_svg(
    ir: Dict[str, Any],
    width: int = 800,
    height: int = 800,
    padding: float = 40.0,
) -> str:
    """Render a 2D (XY) vector SVG projection of the toolpath."""
    segments = ir.get("segments", [])
    coords: List[Tuple[float, float, bool]] = []

    min_x, max_x = float("inf"), float("-inf")
    min_y, max_y = float("inf"), float("-inf")

    for seg in segments:
        end = seg.get("end")
        if end and len(end) >= 2 and end[0] is not None and end[1] is not None:
            x, y = float(end[0]), float(end[1])
            is_travel = bool(seg.get("travel", False))
            coords.append((x, y, is_travel))
            min_x, max_x = min(min_x, x), max(max_x, x)
            min_y, max_y = min(min_y, y), max(max_y, y)

    if not coords:
        min_x, max_x, min_y, max_y = 0.0, 100.0, 0.0, 100.0

    span_x = max(max_x - min_x, 1.0)
    span_y = max(max_y - min_y, 1.0)
    scale = min((width - 2 * padding) / span_x, (height - 2 * padding) / span_y)

    def tx(x: float) -> float:
        return padding + (x - min_x) * scale

    def ty(y: float) -> float:
        return height - (padding + (y - min_y) * scale)

    svg_elements: List[str] = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}">',
        '  <rect width="100%" height="100%" fill="#0f172a"/>',
        f'  <g id="toolpath" stroke-linecap="round" stroke-linejoin="round">',
    ]

    last_pt: Optional[Tuple[float, float]] = None
    for x, y, is_travel in coords:
        sx, sy = tx(x), ty(y)
        if last_pt is not None:
            color = "#475569" if is_travel else "#38bdf8"
            stroke_width = "1" if is_travel else "2"
            dash = ' stroke-dasharray="3,3"' if is_travel else ''
            svg_elements.append(
                f'    <line x1="{last_pt[0]:.2f}" y1="{last_pt[1]:.2f}" x2="{sx:.2f}" y2="{sy:.2f}" stroke="{color}" stroke-width="{stroke_width}"{dash}/>'
            )
        last_pt = (sx, sy)

    svg_elements.append('  </g>')
    svg_elements.append('</svg>')
    return "\n".join(svg_elements) + "\n"


def toolpath_to_interactive_html(
    ir: Dict[str, Any],
    title: str = "Dry 3D Toolpath Viewer",
    bounds: Optional[Sequence[Sequence[float]]] = None,
) -> str:
    """Generate a self-contained, standalone interactive 3D WebGL HTML viewer with Three.js."""
    segments = ir.get("segments", [])
    points_data = []

    for seg in segments:
        end = seg.get("end")
        if end and len(end) == 3 and end[0] is not None and end[1] is not None and end[2] is not None:
            points_data.append({
                "x": float(end[0]),
                "y": float(end[1]),
                "z": float(end[2]),
                "t": bool(seg.get("travel", False)),
                "s": float(seg.get("speed", 0.0)),
                "e": float(seg.get("extrusion", 0.0) or 0.0),
            })

    json_payload = json.dumps(points_data)
    bounds_json = json.dumps(bounds if bounds else [[0, 250], [0, 250], [0, 250]])

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{title}</title>
  <style>
    body {{ margin: 0; overflow: hidden; background: #0b0f19; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; color: #f8fafc; }}
    #canvas-container {{ width: 100vw; height: 100vh; }}
    #hud {{
      position: absolute; top: 16px; left: 16px; background: rgba(15, 23, 42, 0.85);
      backdrop-filter: blur(8px); padding: 16px; border-radius: 8px; border: 1px solid #334155;
      font-size: 13px; max-width: 320px; box-shadow: 0 4px 20px rgba(0,0,0,0.5);
    }}
    h1 {{ margin: 0 0 8px 0; font-size: 16px; color: #38bdf8; }}
    .badge {{ display: inline-block; padding: 2px 6px; border-radius: 4px; background: #1e293b; font-size: 11px; margin-right: 4px; }}
    .metric {{ margin: 4px 0; display: flex; justify-content: space-between; }}
    .metric-val {{ font-weight: 600; color: #94a3b8; }}
  </style>
  <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
</head>
<body>
  <div id="hud">
    <h1>{title}</h1>
    <div style="margin-bottom: 8px;">
      <span class="badge" style="background:#0284c7;color:#fff;">Dry Engine</span>
      <span class="badge" style="background:#059669;color:#fff;">WebGL 3D</span>
    </div>
    <div class="metric"><span>Segments:</span><span class="metric-val" id="seg-count">{len(points_data)}</span></div>
    <div class="metric"><span>Controls:</span><span class="metric-val">Left-Click = Rotate, Right = Pan</span></div>
  </div>
  <div id="canvas-container"></div>

  <script>
    const data = {json_payload};
    const bounds = {bounds_json};

    const container = document.getElementById('canvas-container');
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0b0f19);

    const camera = new THREE.PerspectiveCamera(45, window.innerWidth / window.innerHeight, 0.1, 2000);
    const renderer = new THREE.WebGLRenderer({{ antialias: true }});
    renderer.setSize(window.innerWidth, window.innerHeight);
    renderer.setPixelRatio(window.devicePixelRatio);
    container.appendChild(renderer.domElement);

    const controls = new THREE.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;

    // Grid & Envelope
    const grid = new THREE.GridHelper(bounds[0][1], 20, 0x334155, 0x1e293b);
    grid.position.set(bounds[0][1]/2, 0, bounds[1][1]/2);
    scene.add(grid);

    // Coordinate Axes Triad
    const axes = new THREE.AxesHelper(30);
    scene.add(axes);

    // Build Toolpath Geometry
    if (data.length > 1) {{
      const points = [];
      const colors = [];
      let minZ = Infinity, maxZ = -Infinity;
      data.forEach(p => {{ minZ = Math.min(minZ, p.z); maxZ = Math.max(maxZ, p.z); }});
      const spanZ = Math.max(maxZ - minZ, 1.0);

      for (let i = 1; i < data.length; i++) {{
        const p0 = data[i-1];
        const p1 = data[i];
        
        points.push(new THREE.Vector3(p0.x, p0.z, p0.y));
        points.push(new THREE.Vector3(p1.x, p1.z, p1.y));

        const t = (p1.z - minZ) / spanZ;
        const col = new THREE.Color();
        if (p1.t) {{
          col.setHex(0x475569); // travel
        }} else {{
          col.setHSL(0.55 + t * 0.4, 0.9, 0.55); // layer rainbow
        }}
        colors.push(col.r, col.g, col.b);
        colors.push(col.r, col.g, col.b);
      }}

      const geom = new THREE.BufferGeometry().setFromPoints(points);
      geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
      const mat = new THREE.LineBasicMaterial({{ vertexColors: true, linewidth: 2 }});
      const line = new THREE.LineSegments(geom, mat);
      scene.add(line);

      // Focus camera
      const midX = bounds[0][1] / 2;
      const midY = bounds[1][1] / 2;
      camera.position.set(midX + 150, 180, midY + 150);
      controls.target.set(midX, maxZ / 2, midY);
      controls.update();
    }}

    window.addEventListener('resize', () => {{
      camera.aspect = window.innerWidth / window.innerHeight;
      camera.updateProjectionMatrix();
      renderer.setSize(window.innerWidth, window.innerHeight);
    }});

    function animate() {{
      requestAnimationFrame(animate);
      controls.update();
      renderer.render(scene, camera);
    }}
    animate();
  </script>
</body>
</html>
"""
