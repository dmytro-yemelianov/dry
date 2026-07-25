import { beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({ initDryWeb: vi.fn() }));

vi.mock('@sdk/engine.web', () => ({ initDryWeb: mocks.initDryWeb }));

beforeEach(() => {
  vi.resetModules();
  mocks.initDryWeb.mockReset();
});

test('deduplicates concurrent docs engine initialization', async () => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => { finish = resolve; });
  mocks.initDryWeb.mockReturnValue(pending);
  const { initDryEngine } = await import('./dry-engine');

  const first = initDryEngine();
  const second = initDryEngine();

  expect(second).toBe(first);
  expect(mocks.initDryWeb).toHaveBeenCalledOnce();
  finish();
  await first;
});

test('allows docs engine initialization to retry after rejection', async () => {
  mocks.initDryWeb
    .mockRejectedValueOnce(new Error('temporary asset failure'))
    .mockResolvedValueOnce(undefined);
  const { initDryEngine } = await import('./dry-engine');

  await expect(initDryEngine()).rejects.toThrow('temporary asset failure');
  await expect(initDryEngine()).resolves.toBeUndefined();
  expect(mocks.initDryWeb).toHaveBeenCalledTimes(2);
});
