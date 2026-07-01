import dry

# Lower the L1 design to the typed L2 Dry IR ({ version, segments }).
ir = (dry.Design()
      .geometry(0.6, 0.2).extruder(True)
      .point(0, 0, 0.2).point(20, 0, 0.2).point(20, 20, 0.2).point(0, 20, 0.2).point(0, 0, 0.2)
      .ir())
