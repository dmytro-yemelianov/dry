import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { useStudioStore } from '../store/useStudioStore';
import type { Segment } from '../types/domain';

const PLASTIC_PALETTES: Record<string, { color: number; roughness: number; metalness: number }> = {
  cyan: { color: 0x00d2ff, roughness: 0.28, metalness: 0.08 },
  obsidian: { color: 0x22272e, roughness: 0.35, metalness: 0.1 },
  gold: { color: 0xe6b800, roughness: 0.22, metalness: 0.4 },
  orange: { color: 0xff6600, roughness: 0.32, metalness: 0.05 },
  white: { color: 0xf0f4f8, roughness: 0.38, metalness: 0.02 },
};

// 3D Vector Helpers for Volumetric Bead Geometry
type Vec3 = [number, number, number];
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
const vmad = (p: Vec3, d: Vec3, s: number): Vec3 => [
  p[0] + d[0] * s,
  p[1] + d[1] * s,
  p[2] + d[2] * s,
];

function buildVolumetricBeadsGeometry(segments: Segment[]): THREE.BufferGeometry {
  const pos: number[] = [];
  const nrm: number[] = [];
  const UP: Vec3 = [0, 0, 1];

  const push = (p: Vec3, n: Vec3) => {
    pos.push(p[0], p[1], p[2]);
    nrm.push(n[0], n[1], n[2]);
  };

  const quad = (a: Vec3, b: Vec3, c: Vec3, d: Vec3, n: Vec3) => {
    push(a, n);
    push(b, n);
    push(c, n);
    push(a, n);
    push(c, n);
    push(d, n);
  };

  let cursor: Vec3 = [0, 0, 0];
  for (const seg of segments) {
    const p0: Vec3 = seg.start || cursor;
    const p1: Vec3 = seg.end || p0;
    cursor = p1;

    // Only generate solid geometry for extruding moves
    if (seg.kind === 'travel' || !seg.extruder_on) continue;

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

    const C = (e: Vec3, ss: number, uu: number): Vec3 => vmad(vmad(e, side, hw * ss), vn, hh * uu);

    const a = { mm: C(p0, -1, -1), pm: C(p0, 1, -1), pp: C(p0, 1, 1), mp: C(p0, -1, 1) };
    const b = { mm: C(p1, -1, -1), pm: C(p1, 1, -1), pp: C(p1, 1, 1), mp: C(p1, -1, 1) };
    const neg = (v: Vec3): Vec3 => [-v[0], -v[1], -v[2]];

    quad(a.pm, b.pm, b.pp, a.pp, side); // +side
    quad(a.mm, a.mp, b.mp, b.mm, neg(side)); // -side
    quad(a.mp, a.pp, b.pp, b.mp, vn); // top
    quad(a.mm, b.mm, b.pm, a.pm, neg(vn)); // bottom
    quad(a.mm, a.pm, a.pp, a.mp, neg(dir)); // start cap
    quad(b.mm, b.mp, b.pp, b.pm, dir); // end cap
  }

  const geom = new THREE.BufferGeometry();
  geom.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  geom.setAttribute('normal', new THREE.Float32BufferAttribute(nrm, 3));
  return geom;
}

export const ThreeViewport: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const envelopeMeshRef = useRef<THREE.LineSegments | null>(null);
  const toolpathMeshRef = useRef<THREE.Object3D | null>(null);
  const travelMeshRef = useRef<THREE.LineSegments | null>(null);
  const toolheadMeshRef = useRef<THREE.Mesh | null>(null);

  const activeMachine = useStudioStore((state) => state.activeMachine);
  const toolpath = useStudioStore((state) => state.toolpath);
  const colorMode = useStudioStore((state) => state.colorMode);
  const renderStyle = useStudioStore((state) => state.renderStyle);
  const setRenderStyle = useStudioStore((state) => state.setRenderStyle);
  const plasticMaterial = useStudioStore((state) => state.plasticMaterial);
  const setPlasticMaterial = useStudioStore((state) => state.setPlasticMaterial);
  const currentTime = useStudioStore((state) => state.currentTime);
  const maxTime = useStudioStore((state) => state.maxTime);
  const focusedLineIndex = useStudioStore((state) => state.focusedLineIndex);
  const seekTime = useStudioStore((state) => state.seekTime);
  const importCustomGcode = useStudioStore((state) => state.importCustomGcode);

  // Initialize Scene
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const width = container.clientWidth;
    const height = container.clientHeight;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x090d13);
    sceneRef.current = scene;

    const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 5000);
    camera.position.set(180, -260, 220);
    camera.up.set(0, 0, 1);
    cameraRef.current = camera;

    const renderer = new THREE.WebGLRenderer({ antialias: true, powerPreference: 'high-performance' });
    renderer.setSize(width, height);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.shadowMap.enabled = true;
    rendererRef.current = renderer;
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.target.set(100, 100, 40);
    controlsRef.current = controls;

    // Lighting (Studio Key + Rim + Ambient)
    const hemiLight = new THREE.HemisphereLight(0xddeeff, 0x161b22, 0.9);
    scene.add(hemiLight);

    const keyLight = new THREE.DirectionalLight(0xffffff, 1.4);
    keyLight.position.set(220, -180, 350);
    scene.add(keyLight);

    const rimLight = new THREE.DirectionalLight(0x58a6ff, 0.6);
    rimLight.position.set(-200, 300, 200);
    scene.add(rimLight);

    // Ground Grid
    const gridHelper = new THREE.GridHelper(500, 50, 0x30363d, 0x161b22);
    gridHelper.position.set(128, 128, 0);
    gridHelper.rotation.x = Math.PI / 2;
    scene.add(gridHelper);

    // Toolhead Cone Mesh (Spindle/Nozzle Indicator)
    const coneGeom = new THREE.ConeGeometry(3.5, 9, 24);
    coneGeom.rotateX(-Math.PI / 2);
    coneGeom.translate(0, 0, 4.5);
    const coneMat = new THREE.MeshStandardMaterial({
      color: 0xff3366,
      emissive: 0x550011,
      roughness: 0.25,
      metalness: 0.2,
    });
    const toolhead = new THREE.Mesh(coneGeom, coneMat);
    toolhead.visible = false;
    scene.add(toolhead);
    toolheadMeshRef.current = toolhead;

    // Animation Loop
    let animId: number;
    const animate = () => {
      animId = requestAnimationFrame(animate);
      controls.update();

      const store = useStudioStore.getState();
      if (store.isPlaying) {
        let nextTime = store.currentTime + 0.016 * store.playSpeed;
        if (nextTime > store.maxTime) nextTime = 0;
        seekTime(nextTime);
      }

      renderer.render(scene, camera);
    };
    animate();

    const handleResize = () => {
      if (!container) return;
      const w = container.clientWidth;
      const h = container.clientHeight;
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);
    };
    window.addEventListener('resize', handleResize);

    return () => {
      cancelAnimationFrame(animId);
      window.removeEventListener('resize', handleResize);
      renderer.dispose();
      container.innerHTML = '';
    };
  }, []);

  // Update Machine Build Envelope
  useEffect(() => {
    const scene = sceneRef.current;
    if (!scene || !activeMachine) return;

    if (envelopeMeshRef.current) {
      scene.remove(envelopeMeshRef.current);
    }

    const bv = activeMachine.build_volume;
    const xSpan = bv.x[1] - bv.x[0];
    const ySpan = bv.y[1] - bv.y[0];
    const zSpan = bv.z[1] - bv.z[0];

    const geom = new THREE.BoxGeometry(xSpan, ySpan, zSpan);
    const edges = new THREE.EdgesGeometry(geom);
    const envelope = new THREE.LineSegments(
      edges,
      new THREE.LineBasicMaterial({ color: 0x58a6ff, transparent: true, opacity: 0.35 })
    );
    envelope.position.set(bv.x[0] + xSpan / 2, bv.y[0] + ySpan / 2, bv.z[0] + zSpan / 2);
    scene.add(envelope);
    envelopeMeshRef.current = envelope;
  }, [activeMachine]);

  // Update Toolpath Rendering (Solid Volumetric Beads vs Wireframe)
  useEffect(() => {
    const scene = sceneRef.current;
    if (!scene) return;

    if (toolpathMeshRef.current) {
      scene.remove(toolpathMeshRef.current);
      toolpathMeshRef.current = null;
    }
    if (travelMeshRef.current) {
      scene.remove(travelMeshRef.current);
      travelMeshRef.current = null;
    }

    if (!toolpath || !toolpath.segments || !toolpath.segments.length) return;

    const segments = toolpath.segments;

    if (renderStyle === 'beads') {
      // 1. Realistic Volumetric 3D Plastic Beads
      const solidGeom = buildVolumetricBeadsGeometry(segments);
      const palette = PLASTIC_PALETTES[plasticMaterial] || PLASTIC_PALETTES.cyan;

      const solidMat = new THREE.MeshStandardMaterial({
        color: palette.color,
        roughness: palette.roughness,
        metalness: palette.metalness,
      });

      const solidMesh = new THREE.Mesh(solidGeom, solidMat);
      scene.add(solidMesh);
      toolpathMeshRef.current = solidMesh;

      // 2. Rapid Travel Lines
      const travelPositions: number[] = [];
      let cursor: Vec3 = [0, 0, 0];
      for (const seg of segments) {
        const start: Vec3 = seg.start || cursor;
        const end: Vec3 = seg.end || start;
        cursor = end;
        if (seg.kind === 'travel' || !seg.extruder_on) {
          travelPositions.push(start[0], start[1], start[2], end[0], end[1], end[2]);
        }
      }
      if (travelPositions.length > 0) {
        const tGeom = new THREE.BufferGeometry();
        tGeom.setAttribute('position', new THREE.Float32BufferAttribute(travelPositions, 3));
        const tMat = new THREE.LineBasicMaterial({ color: 0x30363d, transparent: true, opacity: 0.6 });
        const tMesh = new THREE.LineSegments(tGeom, tMat);
        scene.add(tMesh);
        travelMeshRef.current = tMesh;
      }
    } else {
      // Line Segments (Wireframe Mode)
      const positions: number[] = [];
      const colors: number[] = [];

      let maxZ = 1, minZ = 0;
      for (const seg of segments) {
        const z = seg.end ? seg.end[2] : 0;
        if (z > maxZ) maxZ = z;
      }

      let cursor: Vec3 = [0, 0, 0];
      for (const seg of segments) {
        const start: Vec3 = seg.start || cursor;
        const end: Vec3 = seg.end || start;
        cursor = end;

        positions.push(start[0], start[1], start[2], end[0], end[1], end[2]);

        const isTravel = seg.kind === 'travel' || !seg.extruder_on;
        let color: THREE.Color;

        if (colorMode === 'height') {
          const frac = (end[2] - minZ) / (maxZ - minZ + 0.001);
          color = new THREE.Color().setHSL(0.6 - frac * 0.5, 1.0, 0.5);
        } else if (colorMode === 'speed') {
          const speed = seg.speed || 1000;
          const frac = Math.min(1.0, speed / 6000);
          color = new THREE.Color().setHSL(0.66 * (1.0 - frac), 1.0, 0.5);
        } else {
          color = isTravel ? new THREE.Color(0x30363d) : new THREE.Color(0x58a6ff);
        }

        colors.push(color.r, color.g, color.b, color.r, color.g, color.b);
      }

      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
      geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));

      const material = new THREE.LineBasicMaterial({ vertexColors: true, linewidth: 2 });
      const lineMesh = new THREE.LineSegments(geometry, material);
      scene.add(lineMesh);
      toolpathMeshRef.current = lineMesh;
    }
  }, [toolpath, renderStyle, plasticMaterial, colorMode]);

  // Update Toolhead Position
  useEffect(() => {
    const toolhead = toolheadMeshRef.current;
    if (!toolhead || !toolpath || !toolpath.segments || !toolpath.segments.length) {
      if (toolhead) toolhead.visible = false;
      return;
    }

    const segs = toolpath.segments;
    let targetIdx = 0;

    if (focusedLineIndex !== null && focusedLineIndex >= 0 && focusedLineIndex < segs.length) {
      targetIdx = focusedLineIndex;
    } else {
      const frac = Math.min(1.0, Math.max(0.0, currentTime / maxTime));
      targetIdx = Math.floor(frac * (segs.length - 1));
    }

    const seg = segs[targetIdx];
    const pt = seg.end || seg.start || [0, 0, 0];
    toolhead.position.set(pt[0], pt[1], pt[2]);
    toolhead.visible = true;
  }, [currentTime, maxTime, focusedLineIndex, toolpath]);

  const setCameraView = (mode: 'iso' | 'top' | 'front' | 'side') => {
    const camera = cameraRef.current;
    const controls = controlsRef.current;
    if (!camera || !controls) return;

    if (mode === 'iso') camera.position.set(180, -260, 220);
    if (mode === 'top') camera.position.set(128, 128, 450);
    if (mode === 'front') camera.position.set(128, -380, 100);
    if (mode === 'side') camera.position.set(480, 128, 100);

    controls.target.set(128, 128, 40);
    controls.update();
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    if (!e.dataTransfer.files.length) return;
    const file = e.dataTransfer.files[0];
    const reader = new FileReader();
    reader.onload = (evt) => {
      if (typeof evt.target?.result === 'string') {
        importCustomGcode(evt.target.result, file.name);
      }
    };
    reader.readAsText(file);
  };

  return (
    <div
      className="viewport-center"
      onDragOver={(e) => e.preventDefault()}
      onDrop={handleDrop}
    >
      <div className="viewport-overlay-tools">
        <button className="view-btn" onClick={() => setCameraView('iso')}>Iso</button>
        <button className="view-btn" onClick={() => setCameraView('top')}>Top</button>
        <button className="view-btn" onClick={() => setCameraView('front')}>Front</button>
        <button className="view-btn" onClick={() => setCameraView('side')}>Side</button>
      </div>

      <div className="color-mode-tools">
        <button
          className={`color-mode-btn ${renderStyle === 'beads' ? 'active' : ''}`}
          onClick={() => setRenderStyle('beads')}
        >
          Solid Plastic Beads
        </button>
        <button
          className={`color-mode-btn ${renderStyle === 'wireframe' ? 'active' : ''}`}
          onClick={() => setRenderStyle('wireframe')}
        >
          Wireframe
        </button>

        {renderStyle === 'beads' && (
          <select
            className="plastic-material-select"
            value={plasticMaterial}
            onChange={(e) => setPlasticMaterial(e.target.value as any)}
          >
            <option value="cyan">Cyber Cyan PLA</option>
            <option value="obsidian">Obsidian Matte PLA</option>
            <option value="gold">Silk Gold PLA</option>
            <option value="orange">Sunset Orange PLA</option>
            <option value="white">Ceramic White PLA</option>
          </select>
        )}
      </div>

      <div className="engine-status-tag">
        <span className="status-pulse"></span>
        WASM Engine Active (0ms)
      </div>

      <div ref={containerRef} id="viewport3d" style={{ width: '100%', height: '100%' }} />
    </div>
  );
};
