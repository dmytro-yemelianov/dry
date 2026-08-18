use dry_core::{
    emit_plasma_waterjet, resolve, CuttingParams, Design, LeadInType, Op, ResolveParams,
};

#[test]
fn test_plasma_waterjet_pierce_and_cut_sequence() {
    let mut design = Design::default();
    // Rapid move to start cut at (10, 10, 0)
    design.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    // Cut straight line to (100, 10, 0)
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 2500.0 });
    design.ops.push(Op::Move {
        x: Some(100.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    // Turn off torch and rapid away
    design.ops.push(Op::Extruder { on: false });
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    let params = CuttingParams {
        pierce_height: 3.5,
        pierce_delay_s: 0.6,
        cut_height: 1.5,
        safe_traverse_height: 20.0,
        cut_feedrate: 2500.0,
        lead_in_type: LeadInType::Linear,
        lead_in_radius: 5.0,
    };

    let gcode = emit_plasma_waterjet(&toolpath, &params);
    let output = gcode.join("\n");

    // Pierce sequence checks
    assert!(output.contains("G00 Z3.500 ; Move to pierce height"));
    assert!(output.contains("M03 ; Torch ON"));
    assert!(output.contains("G04 P0.60 ; Pierce delay"));
    assert!(output.contains("G01 Z1.500 F1500.0 ; Drop to cut height"));
    // Cut motion
    assert!(output.contains("G01 X100.000 Y10.000 F2500.0"));
    // Retract sequence
    assert!(output.contains("M05 ; Torch off"));
    assert!(output.contains("G00 Z20.000 ; Retract"));
    assert!(output.contains("M30 ; Program end"));
}
