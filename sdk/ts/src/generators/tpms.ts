// TPMS infill generator. The Op-producing path delegates to the Rust engine (via wasm): the SDK
// serialises the camelCase `TpmsOptions` wire form, calls the engine's `tpmsOps`, and parses the L1
// `Op[]` back. The engine slices the selected triply-periodic minimal surface with `libm` math, so the
// TS SDK is byte-identical to the native CLI and the Python SDK (cross-SDK identity, P4) — the previous
// JS `Math`-based marching-squares generator drifted sub-micron and is gone.
//
// The surface metadata (`TPMS_SURFACES`, `tpmsSurfaceSpec`) and the pure field evaluator (`tpmsField`)
// stay in TypeScript: they describe / sample the implicit field for callers and previews, and never
// feed toolpath geometry, so there is no byte-identity contract on them.
import { Design } from '../design';
import type { Op } from '../ops';
import { tpmsOps as engineTpmsOps } from '../engine';

/** Supported triply periodic minimal surface names for TPMS generation. */
export type TpmsSurface =
  | 'gyroid'
  | 'schwarz-p'
  | 'schwarz-d'
  | 'iwp'
  | 'neovius'
  | 'fischer-koch-s'
  | 'fischer-koch-y'
  | 'frd'
  | 'lidinoid'
  | 'split-p';

/** Display metadata for a TPMS surface. */
export interface TpmsSurfaceSpec {
  /** Stable surface identifier used in options. */
  surface: TpmsSurface;
  /** Human-readable surface label. */
  label: string;
  /** Mathematical field equation shown in reference docs/UI. */
  equation: string;
}

interface InternalTpmsSurfaceSpec extends TpmsSurfaceSpec {
  field: (x: number, y: number, z: number) => number;
}

/** Options for slicing a TPMS scalar field into Dry authoring operations. */
export interface TpmsOptions {
  /** Surface family. Defaults to `gyroid`. */
  surface?: TpmsSurface;
  /** Isosurface value f(x,y,z)=isoLevel. */
  isoLevel?: number;
  /** Cubic unit-cell size in mm. */
  cellSize?: number;
  /** Unit cells along X. */
  cellsX?: number;
  /** Unit cells along Y. */
  cellsY?: number;
  /** Unit cells along Z. */
  cellsZ?: number;
  /** XY marching-squares resolution per unit cell. */
  samplesPerCell?: number;
  /** Layer height in mm. */
  layerHeight?: number;
  /** First layer Z in mm. */
  z0?: number;
  /** X coordinate for the generated field origin/center. */
  centerX?: number;
  /** Y coordinate for the generated field origin/center. */
  centerY?: number;
  /** Extrusion bead width in mm. */
  beadWidth?: number;
  /** Extrusion bead height in mm. */
  beadHeight?: number;
  /** Nozzle temperature in degrees Celsius. */
  nozzleTemp?: number;
  /** Print feedrate in mm/min. */
  printSpeed?: number;
  /** Flow multiplier. */
  flow?: number;
  /** Phase offsets in unit-cell periods, useful for moving seams/features. */
  phaseX?: number;
  phaseY?: number;
  phaseZ?: number;
  /** Add a single-wall rectangular perimeter around every sliced layer for infill-style previews. */
  perimeter?: boolean;
  /** Inset for the generated perimeter in mm. */
  perimeterInset?: number;
  /** Drop very short stitched contours. Defaults to one grid cell. */
  minPathLength?: number;
  /** Insert extra Z slices in intervals that are too tall or change contour topology sharply. */
  adaptive?: boolean;
  /** Minimum adaptive layer height in mm. */
  adaptiveMinLayerHeight?: number;
  /** Maximum adaptive layer height in mm. */
  adaptiveMaxLayerHeight?: number;
  /** Maximum tolerated path-length delta before adaptive subdivision. */
  adaptiveMaxLengthDelta?: number;
  /** Maximum tolerated contour point-count delta before adaptive subdivision. */
  adaptiveMaxPointDelta?: number;
  /** Maximum recursive adaptive subdivision depth. */
  adaptiveMaxDepth?: number;
  /** Guardrail for browser/interactive use. Set to Infinity for trusted offline generation. */
  maxFieldSamples?: number;
}

// Pure field evaluators — the nodal value of each surface at (x, y, z), with world coordinates already
// scaled by 2π / cellSize. Used only by `tpmsField` (sampling/preview); toolpath geometry comes from
// the Rust engine. The `label`/`equation` strings feed `TPMS_SURFACES` / `tpmsSurfaceSpec`.
const TPMS_INTERNAL: Record<TpmsSurface, InternalTpmsSurfaceSpec> = {
  gyroid: {
    surface: 'gyroid',
    label: 'Gyroid',
    equation: 'sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)',
    field: (x, y, z) => Math.sin(x) * Math.cos(y) + Math.sin(y) * Math.cos(z) + Math.sin(z) * Math.cos(x),
  },
  'schwarz-p': {
    surface: 'schwarz-p',
    label: 'Schwarz P',
    equation: 'cos(x) + cos(y) + cos(z)',
    field: (x, y, z) => Math.cos(x) + Math.cos(y) + Math.cos(z),
  },
  'schwarz-d': {
    surface: 'schwarz-d',
    label: 'Schwarz D / Diamond',
    equation: 'sin(x)sin(y)sin(z) + sin(x)cos(y)cos(z) + cos(x)sin(y)cos(z) + cos(x)cos(y)sin(z)',
    field: (x, y, z) =>
      Math.sin(x) * Math.sin(y) * Math.sin(z) +
      Math.sin(x) * Math.cos(y) * Math.cos(z) +
      Math.cos(x) * Math.sin(y) * Math.cos(z) +
      Math.cos(x) * Math.cos(y) * Math.sin(z),
  },
  iwp: {
    surface: 'iwp',
    label: 'Schoen I-WP',
    equation: '2(cos(x)cos(y)+cos(y)cos(z)+cos(z)cos(x)) - (cos(2x)+cos(2y)+cos(2z))',
    field: (x, y, z) =>
      2 * (Math.cos(x) * Math.cos(y) + Math.cos(y) * Math.cos(z) + Math.cos(z) * Math.cos(x)) -
      (Math.cos(2 * x) + Math.cos(2 * y) + Math.cos(2 * z)),
  },
  neovius: {
    surface: 'neovius',
    label: 'Neovius',
    equation: '3(cos(x)+cos(y)+cos(z)) + 4cos(x)cos(y)cos(z)',
    field: (x, y, z) => 3 * (Math.cos(x) + Math.cos(y) + Math.cos(z)) + 4 * Math.cos(x) * Math.cos(y) * Math.cos(z),
  },
  'fischer-koch-s': {
    surface: 'fischer-koch-s',
    label: 'Fischer-Koch S',
    equation: 'cos(2x)sin(y)cos(z) + cos(2y)sin(z)cos(x) + cos(2z)sin(x)cos(y)',
    field: (x, y, z) =>
      Math.cos(2 * x) * Math.sin(y) * Math.cos(z) +
      Math.cos(2 * y) * Math.sin(z) * Math.cos(x) +
      Math.cos(2 * z) * Math.sin(x) * Math.cos(y),
  },
  'fischer-koch-y': {
    surface: 'fischer-koch-y',
    label: 'Fischer-Koch Y',
    equation: 'cos(x)cos(y)cos(z)+sin(x)sin(y)sin(z)+sin(2x)sin(y)+sin(2y)sin(z)+sin(x)sin(2z)+sin(2x)cos(z)+cos(x)sin(2y)+cos(y)sin(2z)',
    field: (x, y, z) =>
      Math.cos(x) * Math.cos(y) * Math.cos(z) +
      Math.sin(x) * Math.sin(y) * Math.sin(z) +
      Math.sin(2 * x) * Math.sin(y) +
      Math.sin(2 * y) * Math.sin(z) +
      Math.sin(x) * Math.sin(2 * z) +
      Math.sin(2 * x) * Math.cos(z) +
      Math.cos(x) * Math.sin(2 * y) +
      Math.cos(y) * Math.sin(2 * z),
  },
  frd: {
    surface: 'frd',
    label: 'Schoen F-RD',
    equation: '4cos(x)cos(y)cos(z) - (cos(2x)cos(2y)+cos(2y)cos(2z)+cos(2z)cos(2x))',
    field: (x, y, z) =>
      4 * Math.cos(x) * Math.cos(y) * Math.cos(z) -
      (Math.cos(2 * x) * Math.cos(2 * y) +
        Math.cos(2 * y) * Math.cos(2 * z) +
        Math.cos(2 * z) * Math.cos(2 * x)),
  },
  lidinoid: {
    surface: 'lidinoid',
    label: 'Lidinoid',
    equation: 'sin(2x)cos(y)sin(z)+sin(2y)cos(z)sin(x)+sin(2z)cos(x)sin(y)-cos(2x)cos(2y)-cos(2y)cos(2z)-cos(2z)cos(2x)+0.3',
    field: (x, y, z) =>
      Math.sin(2 * x) * Math.cos(y) * Math.sin(z) +
      Math.sin(2 * y) * Math.cos(z) * Math.sin(x) +
      Math.sin(2 * z) * Math.cos(x) * Math.sin(y) -
      Math.cos(2 * x) * Math.cos(2 * y) -
      Math.cos(2 * y) * Math.cos(2 * z) -
      Math.cos(2 * z) * Math.cos(2 * x) +
      0.3,
  },
  'split-p': {
    surface: 'split-p',
    label: 'Split P',
    equation: '1.1(sum sin(2a)sin(c)cos(b)) - 0.2(sum cos(2a)cos(2b)) - 0.4(sum cos(2a))',
    field: (x, y, z) =>
      1.1 *
        (Math.sin(2 * x) * Math.sin(z) * Math.cos(y) +
          Math.sin(2 * y) * Math.sin(x) * Math.cos(z) +
          Math.sin(2 * z) * Math.sin(y) * Math.cos(x)) -
      0.2 *
        (Math.cos(2 * x) * Math.cos(2 * y) +
          Math.cos(2 * y) * Math.cos(2 * z) +
          Math.cos(2 * z) * Math.cos(2 * x)) -
      0.4 * (Math.cos(2 * x) + Math.cos(2 * y) + Math.cos(2 * z)),
  },
};

/** Metadata catalog for all supported TPMS surfaces. */
export const TPMS_SURFACES: Record<TpmsSurface, TpmsSurfaceSpec> = Object.fromEntries(
  Object.entries(TPMS_INTERNAL).map(([key, spec]) => [
    key,
    { surface: spec.surface, label: spec.label, equation: spec.equation },
  ])
) as Record<TpmsSurface, TpmsSurfaceSpec>;

/** Return display metadata for a TPMS surface. */
export function tpmsSurfaceSpec(surface: TpmsSurface): TpmsSurfaceSpec {
  const spec = TPMS_SURFACES[surface];
  if (!spec) throw new Error(`unknown TPMS surface '${surface}'`);
  return spec;
}

/** Sample a surface's nodal field at (x, y, z) — world coordinates already scaled by 2π / cellSize. */
export function tpmsField(surface: TpmsSurface, x: number, y: number, z: number): number {
  const spec = TPMS_INTERNAL[surface];
  if (!spec) throw new Error(`unknown TPMS surface '${surface}'`);
  return spec.field(x, y, z);
}

/**
 * Serialise options to the camelCase `TpmsOptions` wire form the Rust engine deserialises. The keys
 * already match the engine 1:1, so this is mostly `JSON.stringify` — but JSON has no Infinity literal
 * and the engine reads a non-positive `maxFieldSamples` as "no budget limit", so an explicit Infinity
 * is mapped onto 0 to carry the unbounded sentinel across the boundary.
 */
function serializeTpmsOptions(options: TpmsOptions): string {
  const wire: TpmsOptions = { ...options };
  if (wire.maxFieldSamples === Infinity) wire.maxFieldSamples = 0;
  return JSON.stringify(wire);
}

/**
 * Build the selected TPMS infill as an L1 op list. Delegates generation to the Rust engine so the
 * output is byte-identical to the native CLI / Python SDK (the engine uses `libm`, not JS `Math`).
 * Invalid options (unknown surface, budget overrun, …) throw the engine's structured error.
 */
export function tpmsOps(options: TpmsOptions = {}): Op[] {
  return JSON.parse(engineTpmsOps(serializeTpmsOptions(options))) as Op[];
}

/** Generate a fluent `Design` containing a TPMS toolpath. */
export function tpms(options: TpmsOptions = {}): Design {
  const design = new Design();
  design.ops.push(...tpmsOps(options));
  return design;
}
