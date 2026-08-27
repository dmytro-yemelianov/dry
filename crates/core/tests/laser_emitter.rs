use dry_core::{
    emit_grbl_laser, resolve, Design, LaserError, LaserMode, LaserParams, Op, ResolveParams,
};

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

    let gcode = emit_grbl_laser(&toolpath, &params).expect("power is commanded, so this emits");
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

/// A cutting move that never commanded power must be refused, not fired at maximum.
///
/// `Segment::power` documents `None` as "never commanded", distinct from `Some(0.0)` meaning
/// commanded off. The emitter used to read that as `params.max_power_s`, so the least-informed
/// input produced the most dangerous output a laser can be given: full beam. Because `f64::min`
/// returns its non-NaN operand, `Some(NaN)` produced full beam too, and a negative power was
/// emitted verbatim as `S-5`.
#[test]
fn uncommanded_and_unusable_power_are_refused() {
    let cut_with = |power: Option<f64>| {
        let mut design = Design::default();
        design.ops.push(Op::Move {
            x: Some(0.0),
            y: Some(0.0),
            z: Some(0.0),
        });
        if let Some(level) = power {
            design.ops.push(Op::Power { level });
        }
        design.ops.push(Op::Speed { print: 1500.0 });
        design.ops.push(Op::Extruder { on: true });
        design.ops.push(Op::Move {
            x: Some(50.0),
            y: Some(0.0),
            z: Some(0.0),
        });
        resolve(&design, &ResolveParams::default())
    };
    let params = LaserParams::default();

    let toolpath = cut_with(None);
    assert!(
        matches!(
            emit_grbl_laser(&toolpath, &params),
            Err(LaserError::UncommandedPower { .. })
        ),
        "a cut with no commanded power must be refused rather than fired at max"
    );

    // Commanded off is a legitimate value and still emits.
    let gcode = emit_grbl_laser(&cut_with(Some(0.0)), &params).expect("S0 is a legal command");
    assert!(
        gcode.join("\n").contains("S0"),
        "commanded-off must survive as S0"
    );

    // And a normal power still emits unchanged.
    let gcode = emit_grbl_laser(&cut_with(Some(200.0)), &params).expect("normal power emits");
    assert!(gcode.join("\n").contains("S200"));
}

/// A power above the machine maximum is still clamped — that end of the range is the hardware's
/// to enforce, and the caller is asking for something it cannot do.
#[test]
fn power_above_the_machine_maximum_is_still_clamped() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.0),
    });
    design.ops.push(Op::Power { level: 5000.0 });
    design.ops.push(Op::Speed { print: 1500.0 });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(0.0),
        z: Some(0.0),
    });
    let toolpath = resolve(&design, &ResolveParams::default());

    let params = LaserParams {
        max_power_s: 1000.0,
        ..LaserParams::default()
    };
    let gcode = emit_grbl_laser(&toolpath, &params).expect("clamping is not a refusal");
    assert!(
        gcode.join("\n").contains("S1000"),
        "must clamp to the machine maximum"
    );
}
