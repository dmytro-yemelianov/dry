# 2. Lower to the Dry IR

`.ir()` lowers the design to the typed L2 Dry IR: an array of motion `segments` with endpoints,
kind (line/arc/spline), width/height, and process channels. This is the product the targets emit from.

<LiveExample src="lower" :outputs="['ir', 'gcode']" />
