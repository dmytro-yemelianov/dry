// @dry/sdk — author algorithmic machine toolpaths in TypeScript. A thin front-end onto the Dry engine
// (Rust, compiled to wasm). The same engine the native CLI and the Python SDK use; output is
// byte-identical across all three.
//
//   import { Design } from '@dry/sdk';
//   const d = new Design().geometry(0.6, 0.2).extruder(true)
//     .point(0, 0, 0.2).point(10, 0, 0.2).point(10, 10, 0.2).point(0, 10, 0.2).point(0, 0, 0.2);
//   console.log(d.gcode().join('\n'));   // motion g-code
//   console.log(d.simulate());           // metrics

export { Design } from './design';
export { resolveGcode, resolveMetrics, resolveIr, resolveVerify } from './engine';
export { PRINTERS, RESOLVE_PARAMS } from './ops';
export type { Op, ResolveParams, Metrics, Segment, Toolpath } from './ops';
