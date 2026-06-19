// The fluent authoring API. A `Design` is a chain of L1 ops; builders return `this`. The engine calls
// (`gcode`/`simulate`/`ir`) resolve those ops in wasm — the SDK itself holds no toolpath logic.
import type { Metrics, Op, Toolpath } from './ops';
import { PRINTERS } from './ops';
import { resolveGcode, resolveIr, resolveMetrics } from './engine';

function params(printer: string) {
  const p = PRINTERS[printer];
  if (!p) throw new Error(`unknown printer '${printer}'; known: ${Object.keys(PRINTERS).sort().join(', ')}`);
  return p;
}

export class Design {
  readonly ops: Op[] = [];

  /** Set the extrusion bead cross-section (mm). */
  geometry(width: number, height: number): this {
    this.ops.push({ op: 'geometry', width, height });
    return this;
  }

  /** Turn the extruder on/off (off => subsequent moves are travels). */
  extruder(on: boolean): this {
    this.ops.push({ op: 'extruder', on });
    return this;
  }

  /** Set the print feedrate (mm/min). */
  speed(printSpeed: number): this {
    this.ops.push({ op: 'speed', print: printSpeed });
    return this;
  }

  /** Move to a point; an omitted axis is inherited from the running position. */
  point(x: number | null = null, y: number | null = null, z: number | null = null): this {
    this.ops.push({ op: 'move', x, y, z });
    return this;
  }

  /** A circular arc about (cx, cy) to an end point; clockwise => G2, else G3. */
  arc(a: { cx: number; cy: number; x?: number | null; y?: number | null; z?: number | null; clockwise?: boolean }): this {
    this.ops.push({
      op: 'arc',
      cx: a.cx,
      cy: a.cy,
      x: a.x ?? null,
      y: a.y ?? null,
      z: a.z ?? null,
      clockwise: a.clockwise ?? false,
    });
    return this;
  }

  // ---- process channels (§3) ----

  /** Set the nozzle temperature channel (°C). */
  temperature(nozzle: number): this {
    this.ops.push({ op: 'temperature', nozzle });
    return this;
  }

  /** Set the part-cooling fan channel (0..1). */
  fan(speed: number): this {
    this.ops.push({ op: 'fan', speed });
    return this;
  }

  /** Set the flow multiplier channel (scales deposited volume; default 1.0). */
  flow(ratio: number): this {
    this.ops.push({ op: 'flow', ratio });
    return this;
  }

  /** Set the active tool channel. */
  tool(index: number): this {
    this.ops.push({ op: 'tool', index });
    return this;
  }

  /** Set the toolframe orientation: the tool-direction vector (i, j, k). Identity is +Z. */
  orient(i: number, j: number, k: number): this {
    this.ops.push({ op: 'orient', i, j, k });
    return this;
  }

  /** Pause in place for `seconds` (emits a `G4` dwell). */
  dwell(seconds: number): this {
    this.ops.push({ op: 'dwell', seconds });
    return this;
  }

  // ---- engine calls ----

  /** Resolve + emit motion g-code (an array of lines). */
  gcode(printer = 'generic', relativeE = true): string[] {
    return resolveGcode(this.ops, params(printer), relativeE);
  }

  /** Resolve + simulate; returns metrics (time, distances, material, peak flow). */
  simulate(printer = 'generic'): Metrics {
    return resolveMetrics(this.ops, params(printer));
  }

  /** Resolve to the L2 Dry IR ({ version, segments }). */
  ir(printer = 'generic'): Toolpath {
    return resolveIr(this.ops, params(printer));
  }
}
