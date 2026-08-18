import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { useStudioStore } from '../store/useStudioStore';

export const ThreeViewport: React.FC = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const envelopeMeshRef = useRef<THREE.LineSegments | null>(null);
  const toolpathMeshRef = useRef<THREE.LineSegments | null>(null);
  const toolheadMeshRef = useRef<THREE.Mesh | null>(null);

  const activeMachine = useStudioStore((state) => state.activeMachine);
  const toolpath = useStudioStore((state) => state.toolpath);
  const colorMode = useStudioStore((state) => state.colorMode);
  const isPlaying = useStudioStore((state) => state.isPlaying);
  const currentTime = useStudioStore((state) => state.currentTime);
  const maxTime = useStudioStore((state) => state.maxTime);
  const playSpeed = useStudioStore((state) => state.playSpeed);
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
    rendererRef.current = renderer;
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.target.set(100, 100, 40);
    controlsRef.current = controls;

    // Lighting
    const hemiLight = new THREE.HemisphereLight(0xddeeff, 0x111827, 0.85);
    scene.add(hemiLight);

    const dirLight = new THREE.DirectionalLight(0xffffff, 1.2);
    dirLight.position.set(200, -200, 400);
    scene.add(dirLight);

    // Ground Grid
    const gridHelper = new THREE.GridHelper(500, 50, 0x30363d, 0x161b22);
    gridHelper.position.set(128, 128, 0);
    gridHelper.rotation.x = Math.PI / 2;
    scene.add(gridHelper);

    // Toolhead Cone Mesh
    const coneGeom = new THREE.ConeGeometry(3, 8, 16);
    coneGeom.rotateX(-Math.PI / 2);
    coneGeom.translate(0, 0, 4);
    const coneMat = new THREE.MeshStandardMaterial({ color: 0xff3366, emissive: 0x660022, roughness: 0.3 });
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
      new THREE.LineBasicMaterial({ color: 0x58a6ff, transparent: true, opacity: 0.3 })
    );
    envelope.position.set(bv.x[0] + xSpan / 2, bv.y[0] + ySpan / 2, bv.z[0] + zSpan / 2);
    scene.add(envelope);
    envelopeMeshRef.current = envelope;
  }, [activeMachine]);

  // Update Toolpath Geometry & Colors
  useEffect(() => {
    const scene = sceneRef.current;
    if (!scene) return;

    if (toolpathMeshRef.current) {
      scene.remove(toolpathMeshRef.current);
      toolpathMeshRef.current = null;
    }

    if (!toolpath || !toolpath.segments || !toolpath.segments.length) return;

    const positions: number[] = [];
    const colors: number[] = [];

    let maxZ = 1, minZ = 0;
    for (const seg of toolpath.segments) {
      const z = seg.end ? seg.end[2] : 0;
      if (z > maxZ) maxZ = z;
    }

    let cursor = [0, 0, 0];
    for (const seg of toolpath.segments) {
      const start = seg.start || cursor;
      const end = seg.end || start;
      cursor = end;

      positions.push(start[0], start[1], start[2]);
      positions.push(end[0], end[1], end[2]);

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

      colors.push(color.r, color.g, color.b);
      colors.push(color.r, color.g, color.b);
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));

    const material = new THREE.LineBasicMaterial({ vertexColors: true, linewidth: 2 });
    const mesh = new THREE.LineSegments(geometry, material);
    scene.add(mesh);
    toolpathMeshRef.current = mesh;
  }, [toolpath, colorMode]);

  // Update Toolhead Position based on Playback / Focused Line
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

  // Camera Presets
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

  // Drag & Drop Handler
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
          className={`color-mode-btn ${colorMode === 'type' ? 'active' : ''}`}
          onClick={() => useStudioStore.getState().setColorMode('type')}
        >
          Pass Type
        </button>
        <button
          className={`color-mode-btn ${colorMode === 'height' ? 'active' : ''}`}
          onClick={() => useStudioStore.getState().setColorMode('height')}
        >
          Z-Height
        </button>
        <button
          className={`color-mode-btn ${colorMode === 'speed' ? 'active' : ''}`}
          onClick={() => useStudioStore.getState().setColorMode('speed')}
        >
          Speed Heatmap
        </button>
      </div>

      <div className="engine-status-tag">
        <span className="status-pulse"></span>
        WASM Engine Active (0ms)
      </div>

      <div ref={containerRef} id="viewport3d" style={{ width: '100%', height: '100%' }} />
    </div>
  );
};
