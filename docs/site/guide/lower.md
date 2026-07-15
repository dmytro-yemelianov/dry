# 2. Lower to the Dry IR

`.ir()` lowers the design to the typed L2 Dry IR: an array of motion `segments` with endpoints,
kind (line/arc/spline), width/height, and process channels. This is the product the targets emit from.

Reference: [TypeScript `Toolpath` and `Segment`](../reference/generated/typescript-sdk/types#toolpath),
[IR overview](../reference/generated/ir).

<LiveExample src="lower" :outputs="['ir', 'gcode']" />
