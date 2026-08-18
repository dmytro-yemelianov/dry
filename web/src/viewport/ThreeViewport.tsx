import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { useStudioStore } from '../store/useStudioStore';
import { buildStadiumBeadsGeometry } from './beadGeometry';
import type { Segment, PlasticMaterial } from '../types/domain';

const PLASTIC_MATERIALS: Record<
  PlasticMaterial,
  {
    color: number;
    roughness: number;
    metalness: number;
    clearcoat?: number;
    clearcoatRoughness?: number;
    transmission?: number;
    transparent?: boolean;
    opacity?: number;
  }
> = {
  cyan: {
    color: 0x00d2ff,
    roughness: 0.18,
    metalness: 0.05,
    clearcoat: 0.85,
    clearcoatRoughness: 0.1,
  },
  obsidian: {
    color: 0x1c2128,
    roughness: 0.55,
    metalness: 0.04,
    clearcoat: 0.08,
  },
  gold: {
    color: 0xdfb035,
    roughness: 0.22,
    metalness: 0.65,
    clearcoat: 0.6,
  },
  orange: {
    color: 0xff6600,
    roughness: 0.32,
    metalness: 0.05,
    clearcoat: 0.45,
  },
  white: {
    color: 0xf5f7fa,
    roughness: 0.38,
    metalness: 0.02,
    clearcoat: 0.25,
  },
  resin: {
    color: 0x77ccee,
    roughness: 0.08,
    metalness: 0.0,
    transmission: 0.85,
    transparent: true,
    opacity: 0.9,
  },
};

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
  const segmentSections = useStudioStore((state) => state.segmentSections);
  const effectiveGroupingKind = useStudioStore((state) => state.effectiveGroupingKind);
  const colorMode = useStudioStore((state) => state.colorMode);
  const renderStyle = useStudioStore((state) => state.renderStyle);
  const setRenderStyle = useStudioStore((state) => state.setRenderStyle);
  const plasticMaterial = useStudioStore((state) => state.plasticMaterial);
  const setPlasticMaterial = useStudioStore((state) => state.setPlasticMaterial);
  const slicingFilterMode = useStudioStore((state) => state.slicingFilterMode);
  const setSlicingFilterMode = useStudioStore((state) => state.setSlicingFilterMode);
  const targetSectionIndex = useStudioStore((state) => state.targetSectionIndex);
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
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.toneMappingExposure = 1.15;
    rendererRef.current = renderer;
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.target.set(100, 100, 40);
    controlsRef.current = controls;

    // Studio 3-Point PBR Lighting Rig
    const hemiLight = new THREE.HemisphereLight(0xddeeff, 0x161b22, 1.0);
    scene.add(hemiLight);

    const keyLight = new THREE.DirectionalLight(0xffffff, 1.6);
    keyLight.position.set(240, -180, 380);
    scene.add(keyLight);

    const fillLight = new THREE.DirectionalLight(0x90b0e0, 0.8);
    fillLight.position.set(-220, -200, 200);
    scene.add(fillLight);

    const rimLight = new THREE.DirectionalLight(0x58a6ff, 1.0);
    rimLight.position.set(0, 320, 240);
    scene.add(rimLight);

    // Ground Grid
    const gridHelper = new THREE.GridHelper(500, 50, 0x30363d, 0x161b22);
    gridHelper.position.set(128, 128, 0);
    gridHelper.rotation.x = Math.PI / 2;
    scene.add(gridHelper);

    // Toolhead Cone Mesh
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

  // Update Toolpath Geometry & Physical Shaders
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
      // 1. Solid Extruded Plastic Stadium Beads
      const solidGeom = buildStadiumBeadsGeometry(segments, 8, {
        mode: slicingFilterMode,
        targetSection: targetSectionIndex,
        segmentSections,
      });

      const pDef = PLASTIC_MATERIALS[plasticMaterial] || PLASTIC_MATERIALS.cyan;
      const solidMat = new THREE.MeshPhysicalMaterial({
        color: pDef.color,
        roughness: pDef.roughness,
        metalness: pDef.metalness,
        clearcoat: pDef.clearcoat ?? 0.3,
        clearcoatRoughness: pDef.clearcoatRoughness ?? 0.1,
        transmission: pDef.transmission ?? 0.0,
        transparent: pDef.transparent ?? false,
        opacity: pDef.opacity ?? 1.0,
        side: THREE.DoubleSide,
      });

      const solidMesh = new THREE.Mesh(solidGeom, solidMat);
      scene.add(solidMesh);
      toolpathMeshRef.current = solidMesh;

      // 2. Rapid Travel Lines
      const travelPositions: number[] = [];
      let cursor = [0, 0, 0];
      for (let i = 0; i < segments.length; i++) {
        const seg = segments[i];
        const rawStart = seg.start;
        const p0 = rawStart && rawStart[0] !== null ? (rawStart as number[]) : cursor;
        const rawEnd = seg.end;
        const p1 = rawEnd && rawEnd[0] !== null ? (rawEnd as number[]) : p0;
        cursor = p1;

        const isTravel = seg.travel === true || seg.kind === 'travel';
        if (isTravel) {
          if (slicingFilterMode === 'upToSection' && segmentSections[i] > targetSectionIndex) continue;
          if (slicingFilterMode === 'singleSection' && segmentSections[i] !== targetSectionIndex) continue;
          travelPositions.push(p0[0], p0[1], p0[2], p1[0], p1[1], p1[2]);
        }
      }

      if (travelPositions.length > 0) {
        const tGeom = new THREE.BufferGeometry();
        tGeom.setAttribute('position', new THREE.Float32BufferAttribute(travelPositions, 3));
        const tMat = new THREE.LineBasicMaterial({ color: 0x38424d, transparent: true, opacity: 0.6 });
        const tMesh = new THREE.LineSegments(tGeom, tMat);
        scene.add(tMesh);
        travelMeshRef.current = tMesh;
      }
    } else {
      // Wireframe Lines Mode
      const positions: number[] = [];
      const colors: number[] = [];

      let maxZ = 1, minZ = 0;
      for (const seg of segments) {
        const z = seg.end && seg.end[2] !== null ? (seg.end[2] as number) : 0;
        if (z > maxZ) maxZ = z;
      }

      let cursor = [0, 0, 0];
      for (let i = 0; i < segments.length; i++) {
        const seg = segments[i];
        const rawStart = seg.start;
        const p0 = rawStart && rawStart[0] !== null ? (rawStart as number[]) : cursor;
        const rawEnd = seg.end;
        const p1 = rawEnd && rawEnd[0] !== null ? (rawEnd as number[]) : p0;
        cursor = p1;

        if (slicingFilterMode === 'upToSection' && segmentSections[i] > targetSectionIndex) continue;
        if (slicingFilterMode === 'singleSection' && segmentSections[i] !== targetSectionIndex) continue;

        positions.push(p0[0], p0[1], p0[2], p1[0], p1[1], p1[2]);

        const isTravel = seg.travel === true || seg.kind === 'travel';
        let color: THREE.Color;

        if (colorMode === 'height') {
          const z = p1[2] || 0;
          const frac = (z - minZ) / (maxZ - minZ + 0.001);
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
  }, [toolpath, renderStyle, plasticMaterial, colorMode, slicingFilterMode, targetSectionIndex, segmentSections]);

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
    const rawPt = seg.end || seg.start || [0, 0, 0];
    const pt = [rawPt[0] || 0, rawPt[1] || 0, rawPt[2] || 0];
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

  const filterUnitName =
    effectiveGroupingKind === 'revolution'
      ? 'Turn'
      : effectiveGroupingKind === 'figure'
      ? 'Figure'
      : 'Layer';

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
            <option value="cyan">Cyber Cyan PETG</option>
            <option value="obsidian">Obsidian Matte PLA</option>
            <option value="gold">Silk Gold PLA</option>
            <option value="orange">Sunset Orange PLA</option>
            <option value="white">Ceramic White PLA</option>
            <option value="resin">Translucent SLA Resin</option>
          </select>
        )}

        <div style={{ marginLeft: '6px', display: 'flex', gap: '2px' }}>
          <button
            className={`color-mode-btn ${slicingFilterMode === 'all' ? 'active' : ''}`}
            onClick={() => setSlicingFilterMode('all')}
            title="Show complete toolpath"
          >
            All
          </button>
          <button
            className={`color-mode-btn ${slicingFilterMode === 'upToSection' ? 'active' : ''}`}
            onClick={() => setSlicingFilterMode('upToSection')}
            title={`Inspect up to active ${filterUnitName}`}
          >
            Up to {filterUnitName}
          </button>
          <button
            className={`color-mode-btn ${slicingFilterMode === 'singleSection' ? 'active' : ''}`}
            onClick={() => setSlicingFilterMode('singleSection')}
            title={`Isolate only active ${filterUnitName}`}
          >
            Isolate {filterUnitName}
          </button>
        </div>
      </div>

      <div className="engine-status-tag">
        <span className="status-pulse"></span>
        WASM PBR Engine Active
      </div>

      <div ref={containerRef} id="viewport3d" style={{ width: '100%', height: '100%' }} />
    </div>
  );
};
