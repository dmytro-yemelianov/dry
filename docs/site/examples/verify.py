import dry

# Verify against machine-safety contracts.
d = (dry.Design()
     .geometry(0.6, 0.2).extruder(True)
     .point(0, 0, 0.2).point(300, 0, 0.2))
report = d.verify("generic", bounds=[[0, 250], [0, 210], [0, 220]])
