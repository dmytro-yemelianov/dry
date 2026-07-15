import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { Toolpath } from '@sdk/ops';
import { segmentPoints } from './segment-points';

export type CadViewPreset = 'top' | 'front' | 'right' | 'iso';

export interface ThreeIrRenderOptions {
  maxSegments?: number;
  activeSegment?: number;
}

const PRINT = new THREE.LineBasicMaterial({ color: 0x38a0ff });
const TRAVEL = new THREE.LineDashedMaterial({ color: 0x8ca0b8, dashSize: 3, gapSize: 3, transparent: true, opacity: 0.55 });
const ACTIVE = new THREE.LineBasicMaterial({ color: 0xffd166, linewidth: 3 });

export class ThreeIrViewer {
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 100000);
  private readonly renderer: THREE.WebGLRenderer;
  private readonly controls: OrbitControls;
  private readonly pathGroup = new THREE.Group();
  private grid: THREE.GridHelper | undefined;
  private axes: THREE.AxesHelper | undefined;
  private observer: ResizeObserver | undefined;
  private viewSize = 100;
  private currentIr: Toolpath | undefined;
  private currentOptions: ThreeIrRenderOptions = {};

  constructor(private readonly host: HTMLElement) {
    this.scene.background = new THREE.Color(0x0b0f17);
    this.scene.add(this.pathGroup);
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    this.host.appendChild(this.renderer.domElement);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = false;
    this.controls.screenSpacePanning = true;
    this.controls.mouseButtons = {
      LEFT: THREE.MOUSE.ROTATE,
      MIDDLE: THREE.MOUSE.DOLLY,
      RIGHT: THREE.MOUSE.PAN,
    };
    this.controls.touches = {
      ONE: THREE.TOUCH.ROTATE,
      TWO: THREE.TOUCH.DOLLY_PAN,
    };
    this.controls.addEventListener('change', () => this.paint());

    this.observer = new ResizeObserver(() => this.resize());
    this.observer.observe(this.host);
    this.setView('iso');
    this.resize();
  }

  render(ir: Toolpath, options: ThreeIrRenderOptions = {}): void {
    const geometryChanged = this.currentIr !== ir;
    this.currentIr = ir;
    this.currentOptions = options;
    if (geometryChanged) this.rebuildPath();
    this.updatePathAppearance();
    this.paint();
  }

  clear(): void {
    this.currentIr = undefined;
    this.currentOptions = {};
    this.clearPath();
    this.paint();
  }

  setView(preset: CadViewPreset): void {
    const box = this.currentBox();
    const center = box.getCenter(new THREE.Vector3());
    const size = Math.max(this.viewSize, 1);
    const dist = size * 2.5;

    if (preset === 'top') {
      this.camera.up.set(0, 1, 0);
      this.camera.position.set(center.x, center.y, center.z + dist);
    } else if (preset === 'front') {
      this.camera.up.set(0, 0, 1);
      this.camera.position.set(center.x, center.y - dist, center.z);
    } else if (preset === 'right') {
      this.camera.up.set(0, 0, 1);
      this.camera.position.set(center.x + dist, center.y, center.z);
    } else {
      this.camera.up.set(0, 0, 1);
      this.camera.position.set(center.x + dist, center.y - dist, center.z + dist * 0.75);
    }

    this.controls.target.copy(center);
    this.camera.lookAt(center);
    this.controls.update();
    this.paint();
  }

  fit(): void {
    const box = this.currentBox();
    const center = box.getCenter(new THREE.Vector3());
    const size = box.getSize(new THREE.Vector3());
    this.viewSize = Math.max(size.x, size.y, size.z, 20) * 1.35;
    this.controls.target.copy(center);
    this.controls.update();
    this.refreshHelpers(Math.max(size.x, size.y, 20));
    this.resize();
    this.paint();
  }

  dispose(): void {
    this.observer?.disconnect();
    this.controls.dispose();
    this.clearPath();
    this.clearHelpers();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }

  private rebuildPath(): void {
    this.clearPath();
    if (!this.currentIr) return;
    const segments = this.currentIr.segments;
    for (let index = 0; index < segments.length; index++) {
      const pts = segmentPoints(segments[index]).map(vector);
      if (pts.length < 2) continue;
      const geometry = new THREE.BufferGeometry().setFromPoints(pts);
      const material = segments[index].travel ? TRAVEL : PRINT;
      const line = new THREE.Line(geometry, material);
      if (material instanceof THREE.LineDashedMaterial) line.computeLineDistances();
      this.pathGroup.add(line);
    }
    this.fit();
  }

  private updatePathAppearance(): void {
    const segments = this.currentIr?.segments ?? [];
    const limit = Math.min(segments.length, Math.max(0, this.currentOptions.maxSegments ?? segments.length));
    for (let index = 0; index < this.pathGroup.children.length; index++) {
      const line = this.pathGroup.children[index] as THREE.Line;
      line.visible = index < limit;
      line.material = index === this.currentOptions.activeSegment ? ACTIVE : segments[index]?.travel ? TRAVEL : PRINT;
    }
  }

  private clearPath(): void {
    for (const child of [...this.pathGroup.children]) {
      this.pathGroup.remove(child);
      if ('geometry' in child) (child.geometry as THREE.BufferGeometry).dispose();
    }
  }

  private currentBox(): THREE.Box3 {
    const box = new THREE.Box3().setFromObject(this.pathGroup);
    if (box.isEmpty()) box.setFromCenterAndSize(new THREE.Vector3(0, 0, 0), new THREE.Vector3(100, 100, 30));
    return box;
  }

  private refreshHelpers(size: number): void {
    this.clearHelpers();
    const gridSize = Math.max(20, Math.ceil(size / 10) * 10);
    this.grid = new THREE.GridHelper(gridSize, 20, 0x263241, 0x1a2432);
    this.grid.rotation.x = Math.PI / 2;
    this.scene.add(this.grid);
    this.axes = new THREE.AxesHelper(gridSize * 0.18);
    this.scene.add(this.axes);
  }

  private clearHelpers(): void {
    for (const helper of [this.grid, this.axes]) {
      if (!helper) continue;
      this.scene.remove(helper);
      helper.geometry.dispose();
      const materials = Array.isArray(helper.material) ? helper.material : [helper.material];
      for (const material of materials) material.dispose();
    }
    this.grid = undefined;
    this.axes = undefined;
  }

  private resize(): void {
    const width = Math.max(1, this.host.clientWidth);
    const height = Math.max(1, this.host.clientHeight);
    const aspect = width / height;
    this.camera.left = -this.viewSize * aspect / 2;
    this.camera.right = this.viewSize * aspect / 2;
    this.camera.top = this.viewSize / 2;
    this.camera.bottom = -this.viewSize / 2;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(width, height, false);
    this.paint();
  }

  private paint(): void {
    this.renderer.render(this.scene, this.camera);
  }
}

function vector(p: readonly (number | null)[]): THREE.Vector3 {
  return new THREE.Vector3(p[0] ?? 0, p[1] ?? 0, p[2] ?? 0);
}
