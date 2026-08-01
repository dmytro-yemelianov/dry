// TPMS infill generator for the browser gallery. Op generation delegates to the Rust engine over
// wasm (`tpms_ops_json`) — the same idiom `sdk/ts/src/generators/tpms.ts` uses — so the browser
// demo is byte-identical to the native CLI, the Python SDK, and the TS SDK (the engine slices with
// `libm`, not JS `Math`). This replaces the former 638-line `tpms.js` reimplementation, which
// drifted sub-micron from the engine and carried its own now-fixed defects (see docs/04-tasks.md
// H1.7 / H1.4).
//
// `pathMode` and `arcFit*` are not supported here: the engine's TPMS generator only ever emits
// `move` ops (no `arc`), so there is no G2/G3 arc-fitting equivalent to delegate to.
import { tpms_ops_json } from './pkg/dry_wasm.js';

/** Display metadata for a TPMS surface (label + field equation, for reference/UI only). */
const TPMS_SURFACES = {
  gyroid: {
    label: 'Gyroid',
    equation: 'sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)',
  },
  'schwarz-p': {
    label: 'Schwarz P',
    equation: 'cos(x) + cos(y) + cos(z)',
  },
  'schwarz-d': {
    label: 'Schwarz D / Diamond',
    equation: 'sin(x)sin(y)sin(z) + sin(x)cos(y)cos(z) + cos(x)sin(y)cos(z) + cos(x)cos(y)sin(z)',
  },
  iwp: {
    label: 'Schoen I-WP',
    equation: '2(cos(x)cos(y)+cos(y)cos(z)+cos(z)cos(x)) - (cos(2x)+cos(2y)+cos(2z))',
  },
  neovius: {
    label: 'Neovius',
    equation: '3(cos(x)+cos(y)+cos(z)) + 4cos(x)cos(y)cos(z)',
  },
  'fischer-koch-s': {
    label: 'Fischer-Koch S',
    equation: 'cos(2x)sin(y)cos(z) + cos(2y)sin(z)cos(x) + cos(2z)sin(x)cos(y)',
  },
  'fischer-koch-y': {
    label: 'Fischer-Koch Y',
    equation: 'cos(x)cos(y)cos(z)+sin(x)sin(y)sin(z)+sin(2x)sin(y)+sin(2y)sin(z)+sin(x)sin(2z)+sin(2x)cos(z)+cos(x)sin(2y)+cos(y)sin(2z)',
  },
  frd: {
    label: 'Schoen F-RD',
    equation: '4cos(x)cos(y)cos(z) - (cos(2x)cos(2y)+cos(2y)cos(2z)+cos(2z)cos(2x))',
  },
  lidinoid: {
    label: 'Lidinoid',
    equation: 'sin(2x)cos(y)sin(z)+sin(2y)cos(z)sin(x)+sin(2z)cos(x)sin(y)-cos(2x)cos(2y)-cos(2y)cos(2z)-cos(2z)cos(2x)+0.3',
  },
  'split-p': {
    label: 'Split P',
    equation: '1.1(sum sin(2a)sin(c)cos(b)) - 0.2(sum cos(2a)cos(2b)) - 0.4(sum cos(2a))',
  },
};

/**
 * Serialise options to the camelCase `TpmsOptions` wire form the Rust engine deserialises. Keys
 * already match the engine 1:1 (mirrors `sdk/ts/src/generators/tpms.ts`) — this is mostly
 * `JSON.stringify`, but JSON has no Infinity literal and the engine reads a non-positive
 * `maxFieldSamples` as "no budget limit", so an explicit Infinity maps onto 0.
 */
function serializeTpmsOptions(options) {
  const wire = { ...options };
  if (wire.maxFieldSamples === Infinity) wire.maxFieldSamples = 0;
  return JSON.stringify(wire);
}

/**
 * Build the selected TPMS infill as an L1 op list. Delegates generation to the Rust engine so the
 * output is byte-identical to the native CLI / Python SDK / TS SDK. Invalid options (unknown
 * surface, budget overrun, …) throw the engine's structured error.
 */
function tpmsOps(options = {}) {
  return JSON.parse(tpms_ops_json(serializeTpmsOptions(options)));
}

export { TPMS_SURFACES, tpmsOps };
