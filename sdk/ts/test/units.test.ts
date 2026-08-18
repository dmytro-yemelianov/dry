import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mm, cm, inch, deg, rad, mm_s, mm_min, celsius, s, ms, Design } from '../src/index';

test('dimensional unit conversions are accurate', () => {
  assert.equal(mm(15), 15);
  assert.equal(cm(2.5), 25.0);
  assert.equal(inch(1.0), 25.4);
  assert.equal(inch(2.0), 50.8);

  assert.ok(Math.abs(deg(180) - Math.PI) < 1e-9);
  assert.ok(Math.abs(deg(90) - Math.PI / 2) < 1e-9);
  assert.equal(rad(1.5), 1.5);

  assert.equal(mm_s(10), 600); // 10 mm/s = 600 mm/min
  assert.equal(mm_min(1200), 1200);

  assert.equal(celsius(215), 215);
  assert.equal(s(2.5), 2.5);
  assert.equal(ms(500), 0.5);
});

test('Design authors seamlessly with dimensional unit constructors', () => {
  const d = new Design()
    .geometry(mm(0.6), mm(0.2))
    .extruder(true)
    .speed(mm_s(20)) // 1200 mm/min
    .temperature(celsius(210))
    .dwell(ms(500))
    .point(inch(0), inch(0), mm(0.2))
    .point(inch(1), inch(0), mm(0.2));

  const gcode = d.gcode();
  assert.ok(gcode.some((line) => line.includes('X25.4')));
  assert.ok(gcode.some((line) => line.includes('F1200')));
});
