# 1. Author a Path

The `Design` API is a chain of L1 ops. The engine resolves extrusion, feedrates, and units for you.
Move points, sweep a `G3` arc, drop a spline, then read the motion g-code on the right.

Reference: [`Design`](../reference/generated/typescript-sdk/design#design), [Python `Design`](../reference/generated/python-sdk/design#design).

<LiveExample src="author" :outputs="['gcode', 'ir']" />

Try changing the arc center, or adding `.spline([[20,30,0.2],[0,40,0.2]])`.
