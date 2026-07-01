import { Design } from '@dry/sdk';

// The Metrics pane calls d.simulate(); returning d keeps the canvas visible.
const d = new Design()
  .geometry(0.6, 0.2).extruder(true).speed(1800)
  .point(0, 0, 0.2).point(50, 0, 0.2).point(50, 50, 0.2).point(0, 50, 0.2).point(0, 0, 0.2);
d;
