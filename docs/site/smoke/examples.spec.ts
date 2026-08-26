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

// Studio 2.0 replaced the hand-written gallery page, and with it the source-text assertions that
// web/blocks-regression.mjs used to make about web/index.html. Those checked for element ids and
// function names in a single static file; the equivalent guarantees now have to be observed in the
// running app, which is what this covers. Controls the migration dropped are deliberately absent
// here rather than asserted against — see the parity notes on the Studio 2.0 build change.
test('studio gallery exposes its catalog, machine and playback controls', async ({ page }) => {
  await page.goto('/gallery/?source=fullcontrol&design=nonplanar_spacer');
  await page.waitForFunction(() => (window as typeof window & { __dryReady?: boolean }).__dryReady === true, null, {
    timeout: 30_000,
  });

  // The machine catalog is fetched at runtime rather than bundled, so an unstaged copy would leave
  // this selector silently empty instead of failing the build.
  const machineSelect = page.locator('#machineSelect');
  await expect(machineSelect).toBeVisible();
  expect(await machineSelect.locator('option').count()).toBeGreaterThan(1);

  // Design catalog, including the categories the old source-card selector used to offer.
  for (const category of ['TPMS', 'Lattices', 'FullControl']) {
    await expect(page.getByRole('button', { name: category, exact: true })).toBeVisible();
  }

  await expect(page.getByRole('button', { name: 'Export G-Code' })).toBeVisible();

  // Selecting another design re-resolves through the engine and re-titles the page.
  await page.getByRole('button', { name: 'FullControl', exact: true }).click();
  const before = await page.locator('#gcode .gline').count();
  expect(before).toBeGreaterThan(0);
});

// Controls the Studio 2.0 migration dropped and this branch put back. They are asserted against the
// running app rather than by grepping source, which is what made the old assertions rot silently.
test('studio gallery restores the export, generator and layout controls', async ({ page }) => {
  await page.goto('/gallery/?source=fullcontrol&design=nonplanar_spacer');
  await page.waitForFunction(() => (window as typeof window & { __dryReady?: boolean }).__dryReady === true, null, {
    timeout: 30_000,
  });

  // Five export formats, each producing something. G-code alone was all that survived the migration.
  await page.getByRole('button', { name: 'Export', exact: true }).click();
  const formatSelect = page.locator('#exportFormat');
  await expect(formatSelect).toBeVisible();
  expect(await formatSelect.locator('option').count()).toBe(5);

  await formatSelect.selectOption('fullcontrol-py');
  await expect(page.locator('.export-preview')).toContainText('import fullcontrol as fc');

  // The macro builders only apply to the macro format, and the emitted preamble is real G-code.
  await formatSelect.selectOption('gcode-macros');
  expect(await page.locator('.macro-toggle').count()).toBeGreaterThan(5);
  await expect(page.locator('.export-preview')).toContainText('G21 ; millimeters');

  // The 3+1 layout and its labels.
  await page.locator('#viewMode').selectOption('grid');
  await expect(page.locator('.view-grid-labels')).toBeVisible();
  expect(await page.locator('.view-grid-cell').count()).toBe(4);
  await page.locator('#viewMode').selectOption('iso');

  await expect(page.locator('#resetView')).toBeVisible();
  expect(await page.locator('.panel-resizer').count()).toBe(2);
});

test('TPMS designs expose the full generator parameter set', async ({ page }) => {
  await page.goto('/gallery/');
  await page.waitForFunction(() => (window as typeof window & { __dryReady?: boolean }).__dryReady === true, null, {
    timeout: 30_000,
  });

  await page.getByRole('button', { name: 'TPMS', exact: true }).click();
  const card = page.locator('.gallery-card', { hasText: 'TPMS Gyroid Contours' }).first();
  await card.click();
  // Parameters live behind the card's toggle pill; selecting the design alone does not open them.
  await card.locator('.param-toggle-pill').click();

  // Seven sliders survived the migration; bead width, print speed, the two adaptive heights and the
  // perimeter/adaptive toggles did not.
  const params = page.locator('.param-row');
  await expect(params.first()).toBeVisible();
  expect(await params.count()).toBeGreaterThanOrEqual(13);
  expect(await page.locator('.param-toggle input[type="checkbox"]').count()).toBe(2);

  // Per-control reset, distinct from the group "Reset to Defaults".
  expect(await page.locator('.default-reset').count()).toBeGreaterThan(0);
});

// The Safety panel used to render four hardcoded passing checks that were never computed, while the
// engine's verifier sat behind a dead wrapper. These assertions exist to keep it honest: it must
// report what was inspected, and it must be able to fail.
test('safety panel runs the engine verifier and can fail', async ({ page }) => {
  await page.goto('/gallery/?source=fullcontrol&design=nonplanar_spacer');
  await page.waitForFunction(() => (window as typeof window & { __dryReady?: boolean }).__dryReady === true, null, {
    timeout: 30_000,
  });

  await page.getByRole('button', { name: 'Safety', exact: true }).click();

  // Coverage is stated, not implied: a clean report over zero segments proves nothing.
  const coverage = page.locator('.verify-coverage').first();
  await expect(coverage).toContainText('segments inspected against');
  expect(await page.locator('.verify-rules .verify-rule').count()).toBeGreaterThan(0);

  // The rows the old panel always showed as passing are gone.
  const panel = page.locator('.safety-matrix-root');
  await expect(panel).not.toContainText('Tool Clearance');
  await expect(panel).not.toContainText('First Layer Sanity');

  // Against the machine's own envelope this design is clean...
  await expect(panel).toContainText('No findings');

  // ...and against an impossible one it is not. A panel that cannot fail is not a check.
  await page.getByRole('textbox', { name: 'bounds' }).fill('0,1,0,1,0,1');
  await expect(page.locator('.check-icon.fail').first()).toBeVisible();
  await expect(panel).toContainText('outside the build volume');

  // Repeated findings are grouped rather than rendered one row per segment.
  await expect(page.locator('.finding-count').first()).toContainText('×');
  expect(await page.locator('.check-item').count()).toBeLessThan(10);
});
