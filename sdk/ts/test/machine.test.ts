import test from 'node:test';
import assert from 'node:assert/strict';
import {
  MachineCatalog,
  MachineProfile,
  BUILTIN_MACHINES,
  Design,
  mm,
  mm_s,
} from '../src/index.js';

test('Machine Profile and Catalog Suite', async (t) => {
  await t.test('loads built-in machine profiles correctly', () => {
    const catalog = new MachineCatalog();
    const bambu = catalog.search({ vendor: 'Bambu' });
    assert.equal(bambu.length, 1);
    assert.equal(bambu[0].name, 'Bambu Lab X1 Carbon');
    assert.equal(bambu[0].category, '3d_printer');
    assert.deepEqual(bambu[0].bounds, [0, 256, 0, 256, 0, 256]);
  });

  await t.test('searches machines across categories (CNC, Laser, Plasma)', () => {
    const catalog = new MachineCatalog();
    const cncs = catalog.search({ category: 'cnc_mill' });
    assert.equal(cncs.length, 2); // Shapeoko 4 + Haas VF-2

    const plasma = catalog.search({ category: 'plasma_waterjet' });
    assert.equal(plasma.length, 1);
    assert.equal(plasma[0].id, 'crossfire-pro');
  });

  await t.test('maps machine profile to design pre-flight capability check', async () => {
    const catalog = new MachineCatalog();
    const voron = await catalog.get('voron-v24-350');
    assert.equal(voron.id, 'voron-v24-350');

    // Design within Voron 350 volume
    const safeDesign = new Design()
      .point(mm(10), mm(10), mm(0))
      .speed(mm_s(200))
      .point(mm(300), mm(300), mm(50));

    const safeReport = safeDesign.checkCompatibility(voron.toCapabilities());
    assert.equal(safeReport.compatible, true);
    assert.equal(safeReport.findings.length, 0);

    // Design exceeding Voron volume (X = 400mm)
    const outOfBoundsDesign = new Design()
      .point(mm(10), mm(10), mm(0))
      .speed(mm_s(200))
      .point(mm(400), mm(300), mm(50));

    const badReport = outOfBoundsDesign.checkCompatibility(voron.toCapabilities());
    assert.equal(badReport.compatible, false);
    assert(badReport.findings.some((f) => f.code === 'OUT_OF_BOUNDS_X'));
  });
});
