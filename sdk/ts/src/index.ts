// @dry/sdk — author algorithmic machine toolpaths in TypeScript. A thin front-end onto the Dry engine
// (Rust, compiled to wasm). The same engine the native CLI and the Python SDK use; output is
// byte-identical across all three.
//
//   import { Design } from '@dry/sdk';
//   const d = new Design().geometry(0.6, 0.2).extruder(true)
//     .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2);
//   console.log(d.gcode().join('\n'));   // motion g-code
//   console.log(d.simulate());           // metrics

import './engine.node'; // side effect: install the Node wasm binding on import (Node entry only)

export { Design } from './design';
export { FeatureProgram, feature, group, repeat } from './features';
export type { FeatureNode, FeaturePose, FeatureProgramDocument } from './features';
export {
  expandFeatures,
  resolveGcode,
  resolveMetrics,
  resolveMetricsIr,
  resolveIr,
  resolveBinary,
  resolveOptimizedIr,
  resolveBalancedIr,
  resolveVerify,
} from './engine';
export type { MachineKinematics } from './engine';
export { PRINTERS, RESOLVE_PARAMS } from './ops';
export type {
  Finding,
  Metrics,
  Op,
  Report,
  ResolveParams,
  Segment,
  SegmentKind,
  Severity,
  Toolpath,
  ToolpathMeta,
} from './ops';
export {
  STAR_POLYGON_FAMILIES,
  normalizeStarPolygonAlpha,
  starPolygonDentRadiusRatio,
  starPolygonFamilySpec,
  starPolygonLattice,
  starPolygonLatticeOps,
} from './generators/starPolygonLattice';
export type {
  NormalizedStarPolygonAlpha,
  StarPolygonBasis,
  StarPolygonFamily,
  StarPolygonFamilySpec,
  StarPolygonLatticeOptions,
  StarPolygonRegime,
} from './generators/starPolygonLattice';
export { TPMS_SURFACES, tpms, tpmsField, tpmsOps, tpmsSurfaceSpec } from './generators/tpms';
export type { TpmsOptions, TpmsSurface, TpmsSurfaceSpec } from './generators/tpms';
