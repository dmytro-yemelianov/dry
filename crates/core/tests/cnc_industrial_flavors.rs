//! Industrial CNC Post-Processor Flavors & Robotics Dialect Tests.

use dry_core::{emit_stream, CncFrame, Design, EmitParams, FirmwareFlavor, Op, ResolveParams};

#[test]
fn test_siemens_sinumerik_emission() {
    let mut d = Design::default();
    d.ops.push(Op::Speed { print: 1200.0 });
    d.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(20.0),
        z: Some(-5.0),
    });

    let tp = dry_core::resolve(&d, &ResolveParams::default());
    let params = EmitParams {
        flavor: FirmwareFlavor::Siemens,
        cnc_frame: Some(CncFrame {
            wcs: Some(54),
            tool: Some(3),
            spindle_rpm: Some(8000.0),
            coolant: Some(true),
        }),
        five_axis: true,
        ..Default::default()
    };

    let lines = emit_stream(tp.segments.iter().cloned().map(Ok), &params).unwrap();
    assert!(lines.iter().any(|l| l.contains("G90 G94 G710")));
    assert!(lines.iter().any(|l| l.contains("T3 D1 M6")));
    assert!(lines.iter().any(|l| l.contains("S8000 M3")));
    assert!(lines.iter().any(|l| l.contains("TRAORI")));
    assert!(lines.iter().any(|l| l.contains("TRAFOOF")));
    assert!(lines.iter().any(|l| l.contains("M30")));
}

#[test]
fn test_haas_cnc_emission() {
    let mut d = Design::default();
    d.ops.push(Op::Speed { print: 1500.0 });
    d.ops.push(Op::Move {
        x: Some(25.0),
        y: Some(35.0),
        z: Some(-2.0),
    });

    let tp = dry_core::resolve(&d, &ResolveParams::default());
    let params = EmitParams {
        flavor: FirmwareFlavor::Haas,
        cnc_frame: Some(CncFrame {
            wcs: Some(54),
            tool: Some(1),
            spindle_rpm: Some(10000.0),
            coolant: Some(true),
        }),
        ..Default::default()
    };

    let lines = emit_stream(tp.segments.iter().cloned().map(Ok), &params).unwrap();
    assert!(lines.iter().any(|l| l.contains("G90 G21 G17")));
    assert!(lines.iter().any(|l| l.contains("T1 M6")));
    assert!(lines.iter().any(|l| l.contains("G43 H1")));
    assert!(lines.iter().any(|l| l.contains("G187 P2 E0.025")));
    assert!(lines.iter().any(|l| l.contains("M30")));
}

#[test]
fn test_heidenhain_tnc_emission() {
    let mut d = Design::default();
    d.ops.push(Op::Speed { print: 1000.0 });
    d.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(0.0),
    });

    let tp = dry_core::resolve(&d, &ResolveParams::default());
    let params = EmitParams {
        flavor: FirmwareFlavor::Heidenhain,
        cnc_frame: Some(CncFrame {
            tool: Some(5),
            spindle_rpm: Some(6000.0),
            coolant: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    };

    let lines = emit_stream(tp.segments.iter().cloned().map(Ok), &params).unwrap();
    assert!(lines.iter().any(|l| l.contains("BEGIN PGM DRY MM")));
    assert!(lines.iter().any(|l| l.contains("TOOL CALL 5 Z S6000")));
    assert!(lines.iter().any(|l| l.contains("END PGM DRY MM")));
}

#[test]
fn test_abb_rapid_robot_emission() {
    let mut d = Design::default();
    d.ops.push(Op::Orient {
        i: 0.0,
        j: 0.0,
        k: 1.0,
    });
    d.ops.push(Op::Speed { print: 800.0 });
    d.ops.push(Op::Move {
        x: Some(100.0),
        y: Some(200.0),
        z: Some(300.0),
    });

    let tp = dry_core::resolve(&d, &ResolveParams::default());
    let params = EmitParams {
        flavor: FirmwareFlavor::Rapid,
        ..Default::default()
    };

    let lines = emit_stream(tp.segments.iter().cloned().map(Ok), &params).unwrap();
    assert!(lines.iter().any(|l| l.contains("MODULE DryProgram")));
    assert!(lines.iter().any(|l| l.contains("PROC main()")));
    assert!(lines
        .iter()
        .any(|l| l.contains("MoveL [[100.000, 200.000, 300.000]")));
    assert!(lines.iter().any(|l| l.contains("ENDPROC")));
    assert!(lines.iter().any(|l| l.contains("ENDMODULE")));
}

/// `FirmwareFlavor::named` is the single parser the bindings share.
///
/// It exists because each binding carried its own `match` ending in `_ => Marlin`: a caller asking
/// for a flavor that binding had not learned — every one added after it was written, and every typo
/// — was answered with FFF G-code. For a program that asked for a 5-axis mill or a robot, that is a
/// silent substitution of the wrong machine.
#[test]
fn every_flavor_has_a_name_and_an_unknown_one_is_refused() {
    use dry_core::FirmwareFlavor as F;

    for (name, expected) in [
        ("marlin", F::Marlin),
        ("gcode", F::Marlin),
        ("klipper", F::Klipper),
        ("duet", F::Duet),
        ("rs274", F::Rs274),
        ("linuxcnc", F::Rs274),
        ("grbl", F::Grbl),
        ("laser", F::Grbl),
        ("krl", F::RobotKrl),
        ("siemens", F::Siemens),
        ("sinumerik", F::Siemens),
        ("heidenhain", F::Heidenhain),
        ("tnc", F::Heidenhain),
        ("haas", F::Haas),
        ("rapid", F::Rapid),
    ] {
        assert_eq!(F::named(name).unwrap(), expected, "flavor {name}");
        // Case is not significant, so a caller passing "Siemens" or "GRBL" is not silently wrong.
        assert_eq!(
            F::named(&name.to_uppercase()).unwrap(),
            expected,
            "{name} uppercased"
        );
    }

    for bad in ["", "marlni", "sinumerik840d", "fanuc", "siemen"] {
        let err = F::named(bad).expect_err("an unknown flavor must be refused, not defaulted");
        assert!(err.contains("unknown firmware flavor"), "{err}");
    }
}
