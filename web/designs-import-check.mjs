// Node-side import smoke test for web/designs.js — catches the exact H1.7 regression class where
// `materializeDesign` eagerly called an engine-backed `build` (TPMS) at module-evaluation time,
// before wasm `init()` had run. That made every ES-module import of designs.js throw synchronously,
// which blanked both web/index.html and web/blocks.html (the wasm-load error handler never even ran,
// since it lives in the same module graph that failed to evaluate).
//
// This only needs `./designs.js` to *import* without throwing — that's the actual failure mode.
// Optionally, if the browser-target wasm build (web/pkg/, gitignored — see web/build.sh) is present
// locally, it goes further and calls a TPMS gallery entry's `ops()` thunk to confirm the lazy
// gallery pattern actually produces ops post-init, using `initSync` to avoid the `fetch()`-based
// default init (which needs a browser, not Node).
//
// Note: this deliberately does NOT reuse web/pkg-node (the CommonJS/node wasm target `smoke.cjs`
// builds and uses) — designs.js -> tpms-engine.js hardcodes `import './pkg/dry_wasm.js'`, the
// browser target, so a faithful check of "does importing designs.js work" has to exercise that same
// browser build, not a swapped-in node one. Wiring an automatic `web/pkg` build into CI so this
// step's second half always runs is out of scope here (that's a `.github/workflows/ci.yml` change);
// this script still fails hard on the C1 failure mode (import-time throw) with no wasm build at all,
// and self-reports when it can't go further.
import { pathToFileURL } from 'node:url';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));

let designsModule;
try {
  designsModule = await import(pathToFileURL(path.join(here, 'designs.js')).href);
} catch (error) {
  console.error('FAIL: importing web/designs.js threw at module-evaluation time:');
  console.error(error);
  process.exit(1);
}

const { DESIGNS } = designsModule;
const tpmsKeys = Object.keys(DESIGNS).filter((key) => key.startsWith('tpms_'));
if (tpmsKeys.length === 0) {
  console.error('FAIL: expected TPMS gallery entries in DESIGNS, found none');
  process.exit(1);
}
for (const key of tpmsKeys) {
  if (typeof DESIGNS[key].ops !== 'function') {
    console.error(`FAIL: DESIGNS.${key}.ops should be a lazy thunk (function), got`, typeof DESIGNS[key].ops);
    process.exit(1);
  }
}
console.log(`designs.js imported cleanly (${Object.keys(DESIGNS).length} designs, ${tpmsKeys.length} TPMS)`);

const wasmBgPath = path.join(here, 'pkg/dry_wasm_bg.wasm');
if (!fs.existsSync(wasmBgPath)) {
  console.log('SKIP: web/pkg/dry_wasm_bg.wasm not built locally (gitignored; run `bash web/build.sh` to '
    + 'regenerate) — cannot exercise live TPMS ops() output here, only the import-time fix above.');
  process.exit(0);
}

const wasmModule = await import(pathToFileURL(path.join(here, 'pkg/dry_wasm.js')).href);
wasmModule.initSync({ module: fs.readFileSync(wasmBgPath) });
const sampleKey = tpmsKeys[0];
const ops = DESIGNS[sampleKey].ops();
if (!Array.isArray(ops) || ops.length === 0) {
  console.error(`FAIL: DESIGNS.${sampleKey}.ops() returned no ops after init`);
  process.exit(1);
}
console.log(`DESIGNS.${sampleKey}.ops() produced ${ops.length} ops after init — lazy gallery pattern confirmed live`);
