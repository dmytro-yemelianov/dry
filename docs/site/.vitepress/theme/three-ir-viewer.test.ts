import { expect, test, vi } from 'vitest';
import { ThreeIrViewer } from './three-ir-viewer';

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
