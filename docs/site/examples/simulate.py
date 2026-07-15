import dry

# Simulate to get time, distances, material, and peak flow.
d = (dry.Design()
     .geometry(0.6, 0.2).extruder(True).speed(1800)
     .point(0, 0, 0.2).point(50, 0, 0.2).point(50, 50, 0.2).point(0, 50, 0.2).point(0, 0, 0.2))
metrics = d.simulate()
