import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  renderFrameAxes,
  renderMachineEnvelope,
  renderPassColorSegments,
  Design,
  mm,
} from '../src/index.js';

describe('3D Visualizer Helpers Suite', () => {
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

  it('generates 12 wireframe lines for machine build envelope', () => {
    const box = renderMachineEnvelope([0, 250, 0, 210, 0, 220]);
    assert.strictEqual(box.lines.length, 12);
    // First line should be bottom edge from (0,0,0) to (250,0,0)
    assert.deepStrictEqual(box.lines[0].start, { x: 0, y: 0, z: 0 });
    assert.deepStrictEqual(box.lines[0].end, { x: 250, y: 0, z: 0 });
  });

  it('groups toolpath moves into color-coded pass layers', () => {
    const design = new Design()
      .point(mm(0), mm(0), mm(0))
      .extruder(true)
      .point(mm(50), mm(0), mm(0))
      .extruder(false)
      .point(mm(0), mm(0), mm(10));

    const toolpath = design.ir();
    const groups = renderPassColorSegments(toolpath);
    assert(groups.length >= 2); // Travel + Cutting groups
    assert(groups.some((g) => g.role === 'Travel' && g.color === '#ef4444'));
    assert(groups.some((g) => g.role === 'Cutting / Extrusion' && g.color === '#2563eb'));
  });
});
