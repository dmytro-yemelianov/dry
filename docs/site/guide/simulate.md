# 3. Simulate

`.simulate()` runs the motion model and returns metrics: total/print/travel time, extruding and
travel distance, extruded volume, filament length, and the peak volumetric flow rate.

<LiveExample src="simulate" :outputs="['metrics', 'gcode']" />

Try raising `.speed(...)` and watch `total_time_s` and `max_flow_rate` move.
