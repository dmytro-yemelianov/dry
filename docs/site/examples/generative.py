import dry

# TPMS generation is exposed as a Python g-code helper.
gcode = dry.tpms_gcode({
    "surface": "gyroid",
    "cellSize": 10,
    "cellsX": 2,
    "cellsY": 2,
    "cellsZ": 1,
    "layerHeight": 0.2,
})
