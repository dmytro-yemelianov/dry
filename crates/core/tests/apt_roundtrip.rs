use dry_core::apt::{import_apt, AptImportParams, UnknownMajorWordPolicy};
use dry_core::emit::{emit_stream_to_writer, AptFrame, EmitParams, FirmwareFlavor};

#[test]
fn apt_roundtrip_linear_and_multax() {
    let source = r#"
PARTNO / ROUNDTRIP_TEST
UNITS / MM
LOADTL / 3
SPINDL / RPM, 8000.0, CLW
FEDRAT / MMPM, 1500.0
RAPID
GOTO / 0.0, 0.0, 10.0
FEDRAT / MMPM, 500.0
GOTO / 10.0, 20.0, 10.0
MULTAX / ON
GOTO / 15.0, 25.0, 5.0, 0.0, 0.70710678, 0.70710678
MULTAX / OFF
RAPID
GOTO / 0.0, 0.0, 50.0
SPINDL / OFF
FINI
"#;
    let import_params = AptImportParams {
        version: 0,
        assume_units: None,
        unknown_major_words: UnknownMajorWordPolicy::Refuse,
        arc_tolerance_rel: 1e-4,
    };

    let path1 = import_apt(source, &import_params).expect("import source");
    assert_eq!(path1.segments.len(), 4);

    let emit_params = EmitParams {
        flavor: FirmwareFlavor::Apt,
        apt_frame: AptFrame {
            decimals: 6,
            partno: Some("ROUNDTRIP_TEST".to_string()),
            machin: None,
        },
        ..EmitParams::default()
    };

    let mut emitted = Vec::new();
    emit_stream_to_writer(
        path1.segments.clone().into_iter().map(Ok),
        &emit_params,
        &mut emitted,
    )
    .expect("emit apt");

    let emitted_str = String::from_utf8(emitted).expect("utf-8 apt");
    assert!(emitted_str.contains("PARTNO / ROUNDTRIP_TEST"));
    assert!(emitted_str.contains("LOADTL / 3"));
    assert!(emitted_str.contains("MULTAX / ON"));
    assert!(emitted_str.contains("FINI"));

    // Re-import emitted APT
    let path2 = import_apt(&emitted_str, &import_params).expect("re-import emitted apt");
    assert_eq!(path1.segments.len(), path2.segments.len());

    for (s1, s2) in path1.segments.iter().zip(path2.segments.iter()) {
        assert_eq!(s1.kind, s2.kind);
        assert_eq!(s1.travel, s2.travel);
        assert_eq!(s1.tool, s2.tool);
        for axis in 0..3 {
            assert!((s1.end[axis].unwrap().value() - s2.end[axis].unwrap().value()).abs() < 1e-4);
        }
        if let (Some(o1), Some(o2)) = (s1.orientation, s2.orientation) {
            for i in 0..3 {
                assert!((o1[i] - o2[i]).abs() < 1e-4);
            }
        }
    }
}

#[test]
fn apt_emit_full_turn_arc_rejected() {
    use dry_core::ir::{Segment, SegmentKind};
    use dry_core::units::{Feedrate, Length, Volume};

    let full_turn_seg = Segment {
        start: [
            Some(Length::mm(10.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
        ],
        end: [
            Some(Length::mm(10.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
        ],
        travel: false,
        speed: Feedrate(1000.0),
        length: Length::mm(62.83),
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width: None,
        height: None,
        kind: SegmentKind::Arc,
        centre: Some([Length::mm(0.0), Length::mm(0.0)]),
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        power: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    };

    let emit_params = EmitParams {
        flavor: FirmwareFlavor::Apt,
        ..EmitParams::default()
    };
    let mut out = Vec::new();
    let err = emit_stream_to_writer(vec![Ok(full_turn_seg)], &emit_params, &mut out).unwrap_err();
    assert!(err.to_string().contains("[apt-arc-full-turn]"));
}
