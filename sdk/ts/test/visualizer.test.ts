import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { renderFrameAxes } from '../src/index';

describe('3D Frame Visualizer Triad Axes (Option D)', () => {
  it('generates correct RGB axis segments from origin', () => {
    const axes = renderFrameAxes({ x: 10, y: 20, z: 30 }, 15.0);
    assert.strictEqual(axes.length, 3);

    const xAxis = axes.find((a) => a.axis === 'X')!;
    assert.strictEqual(xAxis.color, '#ff0000');
    assert.deepStrictEqual(xAxis.start, { x: 10, y: 20, z: 30 });
    assert.deepStrictEqual(xAxis.end, { x: 25, y: 20, z: 30 });

    const yAxis = axes.find((a) => a.axis === 'Y')!;
    assert.strictEqual(yAxis.color, '#00ff00');
    assert.deepStrictEqual(yAxis.start, { x: 10, y: 20, z: 30 });
    assert.deepStrictEqual(yAxis.end, { x: 10, y: 35, z: 30 });

    const zAxis = axes.find((a) => a.axis === 'Z')!;
    assert.strictEqual(zAxis.color, '#0000ff');
    assert.deepStrictEqual(zAxis.start, { x: 10, y: 20, z: 30 });
    assert.deepStrictEqual(zAxis.end, { x: 10, y: 20, z: 45 });
  });
});
