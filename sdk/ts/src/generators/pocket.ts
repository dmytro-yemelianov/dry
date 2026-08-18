// CNC contour-parallel pocket and profile generator.
// Delegates to the Rust engine (via wasm) for exact geometry generation.
import { Design } from '../design';
import type { Op } from '../ops';
import { pocketOps as enginePocketOps } from '../engine';

export type CutMode = 'pocket' | 'profile';

export interface PocketOptions {
  shape: 'rect' | 'circle';
  // Rectangular shape fields
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  // Circular shape fields
  cx?: number;
  cy?: number;
  radius?: number;

  mode?: CutMode;
  toolDiameter: number;
  stepover?: number;
  depth: number;
  depthPerPass?: number;
  zTop?: number;
  safeZ?: number;
  cutFeed?: number;
  plungeFeed?: number;
}

/** Generate an L1 `Op[]` list for a CNC pocket or profile cut. */
export function pocketOps(options: PocketOptions): Op[] {
  const json = enginePocketOps(JSON.stringify(options));
  return JSON.parse(json) as Op[];
}

/** Generate a fluent `Design` wrapping the generated CNC pocket ops. */
export function pocket(options: PocketOptions): Design {
  return Design.fromOps(pocketOps(options));
}
