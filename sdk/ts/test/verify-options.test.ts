import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Design } from '../src/index';

const d = () => new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(150, 0, 0.2);

test('the options form matches the positional form it replaces', () => {
  const positional = d().verify('generic', 0, 0, '0,100,0,100,0,50', false, '300,9000');
  const options = d().verify({ bounds: '0,100,0,100,0,50', speedRange: '300,9000' });
  assert.deepEqual(options, positional);
});

test('reaching a late contract no longer needs nine placeholders', () => {
  const positional = d().verify('generic', 0, 0, '', false, '', 0, 0, 0, [0.3, 0.5], [2000, 3000]);
  const options = d().verify({ firstLayerHeightRange: [0.3, 0.5], firstLayerSpeedRange: [2000, 3000] });
  assert.deepEqual(options, positional);
});

test('a misspelled option is an error, not a silently disabled rule', () => {
  assert.throws(() => d().verify({ maxflow: 5 } as never), /unknown verify option 'maxflow'/);
  assert.throws(() => d().verify({ bounds: '0,100,0,100,0,50', speedrange: '1,2' } as never), /unknown verify option 'speedrange'/);
});

test('no arguments still verifies with every contract disabled', () => {
  assert.deepEqual(d().verify(), d().verify({}));
});

// `gcode` carries three consecutive booleans. Transposing two of them type-checks cleanly and
// changes the emitted program, which is the failure the options form removes.
test('gcode options match the positional form', () => {
  const oriented = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .orient(0, 0, 1)
    .point(10, 0, 0.2);

  assert.deepEqual(
    oriented.gcode({ fiveAxis: true, rotaryAxes: 'ab' }),
    oriented.gcode('generic', true, false, true, 'ab')
  );
  assert.deepEqual(oriented.gcode(), oriented.gcode({}));
});

test('a misspelled gcode option is an error', () => {
  const d = new Design().geometry(0.6, 0.2).extruder(true).point(0, 0, 0.2).point(10, 0, 0.2);
  assert.throws(() => d.gcode({ fiveaxis: true } as never), /unknown gcode option 'fiveaxis'/);
});

// Why the options form is worth having here: transposing two of the three booleans type-checks
// cleanly and silently emits a different program. On a design with a travel move, swapping
// `relativeE` and `travelG1E0` turns `G0 F8000 X30` — a rapid — into `G1 F8000 X30 E0.498902`, a
// travel that extrudes, and switches E from relative to accumulating absolute.
test('transposed booleans really do change the program', () => {
  const withTravel = new Design()
    .geometry(0.6, 0.2)
    .extruder(true)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .extruder(false)
    .point(30, 0, 0.2)
    .extruder(true)
    .point(40, 0, 0.2);

  const intended = withTravel.gcode({ relativeE: true, travelG1E0: false });
  const transposed = withTravel.gcode({ relativeE: false, travelG1E0: true });

  assert.notDeepEqual(intended, transposed, 'these flags are not interchangeable');
  assert.ok(
    intended.some((line) => line.startsWith('G0 ')),
    'the intended form emits the travel as a rapid'
  );
  assert.ok(
    !transposed.some((line) => line.startsWith('G0 ')),
    'the transposed form emits no rapid at all — every move extrudes'
  );
});
