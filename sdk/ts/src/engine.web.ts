// Browser loader for the Dry wasm engine. Dynamically loads the web-target glue from a runtime URL
// (the docs site copies web/pkg/ into its public assets), runs the async wasm init, and installs the
// binding. NOTE: takes the URL as a parameter and uses NO `import.meta` so it still type-checks under
// the SDK's commonjs tsc build (it is never *run* on Node — only the docs Vite build executes it).
import { setWasmBinding, type DryWasm } from './engine';

let initPromise: Promise<void> | undefined;

/** Load + initialise the web-target wasm exactly once and install it as the engine binding. */
export function initDryWeb(wasmUrl: string): Promise<void> {
  if (!initPromise) {
    initPromise = (async () => {
      const glue: Record<string, unknown> = await import(/* @vite-ignore */ wasmUrl);
      // wasm-bindgen --target web: default export is the async init; it fetches dry_wasm_bg.wasm
      // relative to the glue's own URL.
      await (glue.default as () => Promise<unknown>)();
      const fn = (k: string) => glue[k] as DryWasm[keyof DryWasm];
      setWasmBinding({
        resolve_gcode: fn('resolve_gcode'),
        tpms_ops_json: fn('tpms_ops_json'),
        resolve_metrics: fn('resolve_metrics'),
        metrics_ir: fn('metrics_ir'),
        resolve_ir: fn('resolve_ir'),
        resolve_binary: fn('resolve_binary'),
        resolve_optimized_ir: fn('resolve_optimized_ir'),
        resolve_balanced_ir: fn('resolve_balanced_ir'),
        resolve_verify: fn('resolve_verify'),
      } as DryWasm);
    })();
  }
  return initPromise;
}
