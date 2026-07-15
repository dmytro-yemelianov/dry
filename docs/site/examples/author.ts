import { Design } from '@dry/sdk';

// A line, a G3 arc, then a line. The engine resolves extrusion for you.
new Design()
  .geometry(0.6, 0.2).extruder(true)
  .point(10, 0, 0.2)
  .arc({ cx: 0, cy: 0, x: 0, y: 10 })
  .point(0, 20, 0.2);
