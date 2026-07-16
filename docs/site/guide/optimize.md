# 5. Optimize

`.optimizedIr()` runs the standard L2 optimization. `.balancedIr(printer, kinematics)` adds
kinematics-aware arc-speed clamping and junction-velocity capping. Compare the IR each produces.

Reference: [TypeScript `optimizedIr`](../reference/generated/typescript-sdk/design#optimizedir) /
[`balancedIr`](../reference/generated/typescript-sdk/design#balancedir),
[profiles and reports](../reference/generated/profiles-and-reports).

<LiveExample src="optimize" :outputs="['ir', 'metrics']" />

Try dropping `max_acceleration_mm_s2` and see the balanced IR change.
