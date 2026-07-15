// Node loader for the Dry wasm engine. Loads the nodejs-target glue (built into ../../wasm by build.sh)
// and installs it as the engine binding. index.ts imports this for its side effect, so the published
// Node package keeps auto-initialising on import — behaviour-identical to the pre-split engine.ts.
import * as path from 'node:path';
import { createRequire } from 'node:module';
import { setWasmBinding, type DryWasm } from './engine';

const requireWasm = createRequire(__filename);
// compiled to dist/src/engine.node.js → two `..` reach the package root, then /wasm.
const wasm = requireWasm(path.join(__dirname, '..', '..', 'wasm', 'dry_wasm.js')) as DryWasm;
setWasmBinding(wasm);
