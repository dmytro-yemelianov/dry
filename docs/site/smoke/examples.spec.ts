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
