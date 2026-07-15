import dry

# Compare the standard optimization vs the kinematics-aware balanced pass.
d = (dry.Design()
     .geometry(0.6, 0.2).extruder(True)
     .point(0, 0, 0.2).arc(cx=25, cy=0, x=50, y=0).point(50, 50, 0.2))
ir = d.balanced_ir("generic", {"max_acceleration_mm_s2": 3000, "max_junction_velocity_mm_s": 8})
