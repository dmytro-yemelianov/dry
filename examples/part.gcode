; sample part for the Dry post-slicer review pilot guide
; a single-layer 20mm square outline, Marlin-style, relative extrusion
M140 S60
M104 S210
M190 S60
M109 S210
G28
G90
M83
G1 Z0.2 F600
G1 X0 Y0 F9000
G1 X20 Y0 E0.8 F1200
G1 X20 Y20 E0.8
G1 X0 Y20 E0.8
G1 X0 Y0 E0.8
G1 Z2.0 F600
M104 S0
M140 S0
