import { Design } from '@dry/sdk';

// Verify against machine-safety contracts. Shrink the bounds below and watch out-of-bounds fire.
new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(0, 0, 0.2).point(300, 0, 0.2)
  .verify('generic', 0, 0, [[0, 250], [0, 210], [0, 220]]);
