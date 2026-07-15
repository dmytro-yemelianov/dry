import { Design } from '@sdk/design';
import { PRINTERS } from '@sdk/ops';
import {
  resolveGcode,
  resolveMetrics,
  resolveMetricsIr,
  resolveIr,
  resolveBinary,
  resolveOptimizedIr,
  resolveBalancedIr,
  resolveVerify,
} from '@sdk/engine';
import { initDryWeb } from '@sdk/engine.web';
import { tpms } from '@sdk/generators/tpms';
import { starPolygonLattice } from '@sdk/generators/starPolygonLattice';

export interface Dry {
  Design: typeof Design;
  PRINTERS: typeof PRINTERS;
  resolveGcode: typeof resolveGcode;
  resolveMetrics: typeof resolveMetrics;
  resolveMetricsIr: typeof resolveMetricsIr;
  resolveIr: typeof resolveIr;
  resolveBinary: typeof resolveBinary;
  resolveOptimizedIr: typeof resolveOptimizedIr;
  resolveBalancedIr: typeof resolveBalancedIr;
  resolveVerify: typeof resolveVerify;
  tpms: typeof tpms;
  starPolygonLattice: typeof starPolygonLattice;
}

const dry: Dry = {
  Design,
  PRINTERS,
  resolveGcode,
  resolveMetrics,
  resolveMetricsIr,
  resolveIr,
  resolveBinary,
  resolveOptimizedIr,
  resolveBalancedIr,
  resolveVerify,
  tpms,
  starPolygonLattice,
};

export function getDry(): Dry {
  return dry;
}

let ready: Promise<void> | undefined;

/** Initialise the wasm engine exactly once. Safe to call from every component mount. */
export function initDryEngine(): Promise<void> {
  if (!ready) {
    const meta = import.meta as ImportMeta & { env?: { BASE_URL?: string } };
    const base = meta.env?.BASE_URL ?? '/';
    const attempt = initDryWeb(`${base}pkg/dry_wasm.js`);
    ready = attempt.catch((error: unknown) => {
      ready = undefined;
      throw error;
    });
  }
  return ready;
}
