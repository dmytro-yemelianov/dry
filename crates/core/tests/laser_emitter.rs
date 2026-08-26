use dry_core::{emit_grbl_laser, resolve, Design, LaserMode, LaserParams, Op, ResolveParams};

#[test]
fn test_grbl_laser_emission_constant_and_dynamic() {
    let mut design = Design::default();
    // Move rapidly to (10, 10, 0)
    design.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    // Set power to 800 and cut line to (50, 10, 0)
    design.ops.push(Op::Power { level: 800.0 });
    design.ops.push(Op::Speed { print: 1500.0 });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    // Cut line to (50, 50, 0)
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(0.0),
    });
    // Turn off and rapid to (0,0,0)
    design.ops.push(Op::Extruder { on: false });
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    let params = LaserParams {
        mode: LaserMode::Dynamic,
        max_power_s: 1000.0,
        default_feedrate: 1200.0,
    };

    let gcode = emit_grbl_laser(&toolpath, &params);
    let output = gcode.join("\n");

    // Dynamic mode should command M4
    assert!(output.contains("M4 S800"));
    // Rapids should command G0 and turn off laser with M5
    assert!(output.contains("M5 ; Laser off"));
    assert!(output.contains("G0 X10.000 Y10.000") || output.contains("G0 X0.000 Y0.000"));
    // Cutting lines should be G1 with feedrate
    assert!(output.contains("G1 X50.000 Y10.000"));
    assert!(output.contains("M2 ; End of program"));
}
