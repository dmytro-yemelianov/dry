import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { oracleGcode } from './oracle';

const here = path.dirname(fileURLToPath(import.meta.url));
const examplesDir = path.resolve(here, '../examples');

const PAGES: Array<{ name: string; url: string }> = [
  { name: 'author', url: '/guide/author' },
  { name: 'lower', url: '/guide/lower' },
  { name: 'simulate', url: '/guide/simulate' },
  { name: 'verify', url: '/guide/verify' },
  { name: 'optimize', url: '/guide/optimize' },
  { name: 'generative', url: '/guide/generative' },
];

for (const { name, url } of PAGES) {
  test(`live example '${name}' executes and matches the engine`, async ({ page }) => {
    await page.goto(url);
    const root = page.locator('.live').first();
    await expect(root).toBeVisible();
    await expect(root.locator('.live-loading')).toHaveCount(0, { timeout: 30_000 });
    await expect(root.locator('.live-error')).toHaveCount(0);

    const src = fs.readFileSync(path.join(examplesDir, `${name}.ts`), 'utf8');
    const expected = oracleGcode(src);
    test.skip(expected.length === 0, `the '${name}' example does not produce g-code`);
    await root.getByRole('button', { name: 'gcode' }).click();
    const shown = (await root.locator('.live-out').first().innerText()).trim().split('\n');
    expect(shown).toEqual(expected);
  });
}

test('public WASM gallery renders licensed FullControl samples', async ({ page }) => {
  const galleryResponse = await page.goto('/gallery/?source=fullcontrol&design=nonplanar_spacer');
  expect(galleryResponse?.status()).toBe(200);

  const wasmGlueResponse = await page.request.get('/gallery/pkg/dry_wasm.js');
  expect(wasmGlueResponse.status()).toBe(200);
  expect(wasmGlueResponse.headers()['content-type']).toContain('text/javascript');

  const wasmBinaryResponse = await page.request.get('/gallery/pkg/dry_wasm_bg.wasm');
  expect(wasmBinaryResponse.status()).toBe(200);
  expect(wasmBinaryResponse.headers()['content-type']).toContain('application/wasm');

  await page.waitForFunction(() => (window as typeof window & { __dryReady?: boolean }).__dryReady === true, null, {
    timeout: 30_000,
  });
  await expect(page.locator('#viewport canvas')).toHaveCount(1);

  const inventory = await page.evaluate(() =>
    (window as typeof window & { __galleryInventory?: { fullcontrol: string[] } }).__galleryInventory,
  );
  expect(inventory?.fullcontrol).toHaveLength(28);
  expect(inventory?.fullcontrol).toContain('nonplanar_spacer');
  expect(inventory?.fullcontrol).toContain('overhang_challenge_plus');
  await expect(page.locator('#designTitle')).toHaveText('Nonplanar Spacer');
  await expect(page.locator('#designLinks')).toContainText('Original notebook');
  await expect(page.locator('#designLinks')).toContainText('fullcontrol.xyz');
  await expect(page.locator('#sourceError')).toBeHidden();
  expect(await page.locator('#gcode .gline').count()).toBeGreaterThan(0);

  await page.getByRole('textbox', { name: 'bounds' }).fill('0,not-a-number,10');
  await expect(page.locator('#sourceError')).toContainText('bounds must contain only finite comma-separated numbers');
  await page.getByRole('textbox', { name: 'bounds' }).fill('');
  await expect(page.locator('#sourceError')).toBeHidden();

  await page.goto('/gallery/?source=fullcontrol&design=overhang_challenge_plus');
  await page.waitForFunction(() => (window as typeof window & { __dryReady?: boolean }).__dryReady === true, null, {
    timeout: 30_000,
  });
  await expect(page.locator('#designTitle')).toHaveText('Overhang Challenge Plus');
  await expect(page.locator('#designLinks')).toContainText('fullcontrol.xyz');
  await expect(page.locator('#sourceError')).toBeHidden();
  expect(await page.locator('#gcode .gline').count()).toBeGreaterThan(0);
});
