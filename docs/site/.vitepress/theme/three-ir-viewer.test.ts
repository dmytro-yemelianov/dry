import { expect, test, vi } from 'vitest';
import * as THREE from 'three';
import { ThreeIrViewer } from './three-ir-viewer';
import type { Toolpath } from '@sdk/ops';

test('clear removes the current toolpath and repaints the empty scene', () => {
  const viewer = Object.create(ThreeIrViewer.prototype) as ThreeIrViewer;
  const internals = viewer as unknown as {
    currentIr: unknown;
    currentOptions: Record<string, unknown>;
    clearPath: () => void;
    paint: () => void;
  };
  internals.currentIr = { version: 1, segments: [] };
  internals.currentOptions = { maxSegments: 1 };
  internals.clearPath = vi.fn();
  internals.paint = vi.fn();

  viewer.clear();

  expect(internals.currentIr).toBeUndefined();
  expect(internals.currentOptions).toEqual({});
  expect(internals.clearPath).toHaveBeenCalledOnce();
  expect(internals.paint).toHaveBeenCalledOnce();
});

test('fit updates controls after changing their target', () => {
  const viewer = Object.create(ThreeIrViewer.prototype) as ThreeIrViewer;
  const update = vi.fn();
  const internals = viewer as unknown as {
    controls: { target: { copy: (value: THREE.Vector3) => void }; update: () => void };
    currentBox: () => THREE.Box3;
    refreshHelpers: () => void;
    resize: () => void;
    paint: () => void;
  };
  internals.controls = { target: { copy: vi.fn() }, update };
  internals.currentBox = () => new THREE.Box3(new THREE.Vector3(0, 0, 0), new THREE.Vector3(10, 20, 5));
  internals.refreshHelpers = vi.fn();
  internals.resize = vi.fn();
  internals.paint = vi.fn();

  viewer.fit();

  expect(update).toHaveBeenCalledOnce();
});

test('playback-only renders reuse existing path geometry', () => {
  const viewer = Object.create(ThreeIrViewer.prototype) as ThreeIrViewer;
  const rebuildPath = vi.fn();
  const internals = viewer as unknown as {
    currentIr: Toolpath | undefined;
    rebuildPath: () => void;
    updatePathAppearance: () => void;
    paint: () => void;
  };
  internals.currentIr = undefined;
  internals.rebuildPath = rebuildPath;
  internals.updatePathAppearance = vi.fn();
  internals.paint = vi.fn();
  const ir = { version: 1, segments: [] } as Toolpath;

  viewer.render(ir);
  viewer.render(ir, { maxSegments: 0 });

  expect(rebuildPath).toHaveBeenCalledOnce();
  expect(internals.updatePathAppearance).toHaveBeenCalledTimes(2);
});

test('helper refresh disposes previous helper resources', () => {
  const viewer = Object.create(ThreeIrViewer.prototype) as ThreeIrViewer;
  const gridGeometryDispose = vi.fn();
  const gridMaterialDispose = vi.fn();
  const axesGeometryDispose = vi.fn();
  const axesMaterialDispose = vi.fn();
  const remove = vi.fn();
  const internals = viewer as unknown as {
    scene: { remove: (object: unknown) => void };
    grid: { geometry: { dispose: () => void }; material: { dispose: () => void } } | undefined;
    axes: { geometry: { dispose: () => void }; material: { dispose: () => void } } | undefined;
    clearHelpers: () => void;
  };
  internals.scene = { remove };
  internals.grid = { geometry: { dispose: gridGeometryDispose }, material: { dispose: gridMaterialDispose } };
  internals.axes = { geometry: { dispose: axesGeometryDispose }, material: { dispose: axesMaterialDispose } };

  internals.clearHelpers();

  expect(remove).toHaveBeenCalledTimes(2);
  expect(gridGeometryDispose).toHaveBeenCalledOnce();
  expect(gridMaterialDispose).toHaveBeenCalledOnce();
  expect(axesGeometryDispose).toHaveBeenCalledOnce();
  expect(axesMaterialDispose).toHaveBeenCalledOnce();
});
