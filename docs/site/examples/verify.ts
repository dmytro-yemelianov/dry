import { Design } from '@dry/sdk';

// The Verify pane checks bounds; returning d keeps the canvas visible.
const d = new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(0, 0, 0.2).point(300, 0, 0.2);
d;
