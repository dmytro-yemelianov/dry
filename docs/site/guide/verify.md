# 4. Verify

`.verify(printer, maxFlow, minTemp, bounds, ...)` checks the resolved toolpath against machine-safety
contracts and returns findings. The example prints a point outside the build volume; shrink or grow
the bounds and watch the out-of-bounds finding appear and clear.

<LiveExample src="verify" :outputs="['verify', 'gcode']" />
