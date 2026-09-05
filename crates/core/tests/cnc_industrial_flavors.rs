//! Industrial CNC Post-Processor Flavors & Robotics Dialect Tests.

use dry_core::{
    emit_stream, CncFrame, Design, EmitParams, Feedrate, FirmwareFlavor, Length, Op, ResolveParams,
    Segment, SegmentKind, Volume,
};

fn rapid_segment(kind: SegmentKind) -> Segment {
    Segment {
        start: [None, None, None],
        end: [None, None, None],
        travel: false,
        speed: Feedrate(1200.0),
        length: Length::ZERO,
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width: None,
        height: None,
        kind,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
        power: None,
    }
}

fn mm(value: f64) -> Option<Length> {
    Some(Length::mm(value))
}

fn rapid_program(segments: Vec<Segment>) -> Result<String, String> {
    let params = EmitParams {
        flavor: FirmwareFlavor::Rapid,
        ..EmitParams::default()
    };
    emit_stream(segments.into_iter().map(Ok), &params)
        .map(|lines| {
            let mut program = lines.join("\n");
            if !program.ends_with('\n') {
                program.push('\n');
            }
            program
        })
        .map_err(|error| error.to_string())
}

fn rapid_reference_segments() -> Vec<Segment> {
    vec![
        Segment {
            end: [mm(10.0), mm(0.0), mm(5.0)],
            travel: true,
            speed: Feedrate(3000.0),
            orientation: Some([0.0, 0.0, 1.0]),
            ..rapid_segment(SegmentKind::Line)
        },
        Segment {
            start: [mm(10.0), mm(0.0), mm(5.0)],
            end: [mm(20.0), mm(0.0), mm(5.0)],
            orientation: Some([0.0, 0.0, 1.0]),
            ..rapid_segment(SegmentKind::Line)
        },
        Segment {
            start: [mm(20.0), mm(0.0), mm(5.0)],
            end: [mm(30.0), mm(10.0), mm(5.0)],
            centre: Some([Length::mm(20.0), Length::mm(10.0)]),
            orientation: Some([0.6, 0.0, 0.8]),
            ..rapid_segment(SegmentKind::Arc)
        },
        Segment {
            dwell_s: Some(1.5),
            ..rapid_segment(SegmentKind::Dwell)
        },
        Segment {
            start: [mm(30.0), mm(10.0), mm(5.0)],
            end: [mm(30.0), mm(20.0), mm(5.0)],
            speed: Feedrate(600.0),
            orientation: Some([0.0, -1.0, 0.0]),
            ..rapid_segment(SegmentKind::Line)
        },
    ]
}

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

#[test]
fn rapid_dwell_and_arc_direction_are_emitted_as_waittime_and_distinct_circle_points() {
    let mut ccw_segments = rapid_reference_segments();
    let ccw = rapid_program(ccw_segments.clone()).unwrap();
    assert!(ccw.contains("    WaitTime 1.500;\n"), "{ccw}");
    assert!(
        ccw.contains("MoveC [[27.071, 2.929, 5.000]"),
        "the first MoveC target must be a point on the CCW arc, not its centre:\n{ccw}"
    );

    ccw_segments[2].clockwise = true;
    let cw = rapid_program(ccw_segments).unwrap();
    assert!(
        cw.contains("MoveC [[12.929, 17.071, 5.000]"),
        "the clockwise sweep must put CirPoint on the opposite side:\n{cw}"
    );
    assert_ne!(
        ccw, cw,
        "opposite arc directions must not emit the same MoveC"
    );
}

#[test]
fn rapid_movec_uses_the_segments_explicit_start_for_its_circle_point() {
    let segment = Segment {
        start: [mm(10.0), mm(0.0), mm(0.0)],
        end: [mm(0.0), mm(10.0), mm(0.0)],
        centre: Some([Length::mm(0.0), Length::mm(0.0)]),
        orientation: Some([0.0, 0.0, 1.0]),
        ..rapid_segment(SegmentKind::Arc)
    };

    let program = rapid_program(vec![segment]).unwrap();
    assert!(
        program.contains("MoveC [[7.071, 7.071, 0.000]"),
        "CirPoint must be derived from the segment's explicit start, not the previous modal position:\n{program}"
    );
}

/// This is a structural drift oracle only: no ABB controller or RobotStudio instance runs in CI.
#[test]
fn rapid_structural_golden_does_not_drift() {
    let program = rapid_program(rapid_reference_segments()).unwrap();
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/reports/robot/reference-rapid.mod"
    );
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(std::path::Path::new(golden_path).parent().unwrap()).unwrap();
        std::fs::write(golden_path, &program).unwrap();
    }
    let golden = std::fs::read_to_string(golden_path).expect(
        "RAPID structural golden exists — update only after independently reviewing the emitter \
         contract and exact bytes",
    );
    assert_eq!(
        program, golden,
        "RAPID output drifted from its structural golden"
    );
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
