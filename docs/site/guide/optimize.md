# 5. Optimize

`.optimizedIr()` runs the standard L2 optimization. `.balancedIr(printer, kinematics)` adds
kinematics-aware arc-speed clamping and junction-velocity capping. Compare the IR each produces.

<LiveExample src="optimize" :outputs="['ir', 'metrics']" />

Try dropping `max_acceleration_mm_s2` and see the balanced IR change.
