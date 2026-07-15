// Browser loader for the Dry wasm engine. Dynamically loads the web-target glue from a runtime URL
// (the docs site copies web/pkg/ into its public assets), runs the async wasm init, and installs the
// binding. NOTE: takes the URL as a parameter and uses NO `import.meta` so it still type-checks under
// the SDK's commonjs tsc build (it is never *run* on Node — only the docs Vite build executes it).
import { setWasmBinding, type DryWasm } from './engine';

let initPromise: Promise<void> | undefined;

type DryWasmGlue = Record<string, unknown> & {
  default?: (wasmPath?: string | URL | Request) => Promise<unknown>;
};

async function importWasmFromPublic(glueUrl: string): Promise<DryWasmGlue> {
  const response = await globalThis.fetch(glueUrl);
  if (!response.ok) {
    throw new Error(`failed to load dry_wasm glue module from ${glueUrl} (${response.status} ${response.statusText})`);
  }

  const glueSource = await response.text();
  const blob = new Blob([glueSource], { type: 'application/javascript' });
  const blobUrl = URL.createObjectURL(blob);

  try {
    // Vite does not allow importing a /public file as a source module. Importing the fetched glue
    // through a blob URL leaves the public asset at runtime and avoids Vite's `?import` rewrite.
    return (await import(/* @vite-ignore */ blobUrl)) as DryWasmGlue;
  } finally {
    URL.revokeObjectURL(blobUrl);
  }
}

/** Load + initialise the web-target wasm exactly once and install it as the engine binding. */
export function initDryWeb(wasmUrl: string): Promise<void> {
  if (!initPromise) {
    const attempt = (async () => {
      const useBlobLoader =
        typeof globalThis.fetch === 'function' &&
        typeof Blob !== 'undefined' &&
        typeof URL !== 'undefined' &&
        typeof URL.createObjectURL === 'function';
      const glueUrl = new URL(wasmUrl, globalThis.location?.href ?? 'http://localhost/');
      const glue = useBlobLoader
        ? await importWasmFromPublic(glueUrl.toString())
        : (await import(/* @vite-ignore */ wasmUrl)) as DryWasmGlue;

      const init = glue.default;
      if (typeof init !== 'function') {
        throw new Error('failed to initialise dry_wasm: missing default export function');
      }

      // A blob module has no useful relative location, so provide the wasm binary URL explicitly.
      const initArg = useBlobLoader ? new URL('dry_wasm_bg.wasm', glueUrl).toString() : undefined;
      await init(initArg);
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
    initPromise = attempt.catch((error: unknown) => {
      initPromise = undefined;
      throw error;
    });
  }
  return initPromise;
}
