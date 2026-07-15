import { beforeEach, expect, test, vi } from 'vitest';

const wasmExports = `
export const resolve_gcode = () => [];
export const tpms_ops_json = () => '[]';
export const resolve_metrics = () => '{}';
export const metrics_ir = () => '{}';
export const resolve_ir = () => '{}';
export const resolve_binary = () => new Uint8Array();
export const resolve_optimized_ir = () => '{}';
export const resolve_balanced_ir = () => '{}';
export const resolve_verify = () => '{}';
`;

function moduleUrl(source: string): string {
  return `data:text/javascript;charset=utf-8,${encodeURIComponent(source)}`;
}

beforeEach(() => {
  vi.resetModules();
  delete (globalThis as { dryWebInitCount?: number }).dryWebInitCount;
});

test('deduplicates concurrent web engine initialization', async () => {
  const { initDryWeb } = await import('@sdk/engine.web');
  const url = moduleUrl(`
    export default async function init() {
      globalThis.dryWebInitCount = (globalThis.dryWebInitCount || 0) + 1;
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    ${wasmExports}
  `);

  const first = initDryWeb(url);
  const second = initDryWeb(url);

  expect(second).toBe(first);
  await first;
  expect((globalThis as { dryWebInitCount?: number }).dryWebInitCount).toBe(1);
});

test('allows a retry after initialization rejects', async () => {
  const { initDryWeb } = await import('@sdk/engine.web');
  const failingUrl = moduleUrl(`
    export default async function init() { throw new Error('transient wasm failure'); }
    ${wasmExports}
  `);
  const workingUrl = moduleUrl(`
    export default async function init() {
      globalThis.dryWebInitCount = (globalThis.dryWebInitCount || 0) + 1;
    }
    ${wasmExports}
  `);

  await expect(initDryWeb(failingUrl)).rejects.toThrow('transient wasm failure');
  await expect(initDryWeb(workingUrl)).resolves.toBeUndefined();
  expect((globalThis as { dryWebInitCount?: number }).dryWebInitCount).toBe(1);
});
