//! ISO/ASTM 52915 3MF Toolpath Extension Conformance Suite (Track C1.1).

use dry_core::codec::threemf::{export_3mf_xml, import_3mf_xml};
use dry_core::ir::{Segment, SegmentKind, Toolpath};
use dry_core::units::{Feedrate, Length, Volume};

#[test]
fn iso_astm_52915_3mf_toolpath_full_roundtrip() {
    let original = Toolpath {
        version: 0,
        meta: None,
        segments: vec![
            // Segment 0: 5-Axis Conformal Extrusion with width/height/temperature/fan
            Segment {
                start: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                ],
                end: [
                    Some(Length::mm(15.0)),
                    Some(Length::mm(20.0)),
                    Some(Length::mm(5.0)),
                ],
                travel: false,
                speed: Feedrate(2400.0),
                length: Length::mm(25.4951),
                volume: Volume(1.25),
                filament: Length::mm(0.42),
                width: Some(Length::mm(0.45)),
                height: Some(Length::mm(0.20)),
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: Some(220.0),
                fan: Some(80.0),
                flow: None,
                tool: None,
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: Some([
                    0.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                    std::f64::consts::FRAC_1_SQRT_2,
                ]),
                control_points: None,
            },
            // Segment 1: Planar Arc with center
            Segment {
                start: [
                    Some(Length::mm(15.0)),
                    Some(Length::mm(20.0)),
                    Some(Length::mm(5.0)),
                ],
                end: [
                    Some(Length::mm(25.0)),
                    Some(Length::mm(30.0)),
                    Some(Length::mm(5.0)),
                ],
                travel: false,
                speed: Feedrate(1800.0),
                length: Length::mm(15.708),
                volume: Volume(0.85),
                filament: Length::mm(0.28),
                width: Some(Length::mm(0.45)),
                height: Some(Length::mm(0.20)),
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(15.0), Length::mm(30.0)]),
                clockwise: true,
                temperature: Some(220.0),
                fan: Some(80.0),
                flow: None,
                tool: None,
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            },
            // Segment 2: Dwell
            Segment {
                start: [
                    Some(Length::mm(25.0)),
                    Some(Length::mm(30.0)),
                    Some(Length::mm(5.0)),
                ],
                end: [
                    Some(Length::mm(25.0)),
                    Some(Length::mm(30.0)),
                    Some(Length::mm(5.0)),
                ],
                travel: false,
                speed: Feedrate(0.0),
                length: Length::ZERO,
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: None,
                height: None,
                kind: SegmentKind::Dwell,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: None,
                power: None,
                dwell_s: Some(1.5),
                manual_gcode: None,
                orientation: None,
                control_points: None,
            },
        ],
    };

    let xml = export_3mf_xml(&original).unwrap();
    assert!(xml.contains("xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\""));
    assert!(
        xml.contains("xmlns:tp=\"http://schemas.microsoft.com/3dmanufacturing/toolpath/2022/07\"")
    );
    assert!(xml.contains("width=\"0.4500\""));
    assert!(xml.contains("height=\"0.2000\""));
    assert!(xml.contains("fan=\"80.0\""));
    assert!(xml.contains("i=\"0.000000\" j=\"0.707107\" k=\"0.707107\""));
    assert!(xml.contains("cx=\"15.0000\" cy=\"30.0000\" cw=\"true\""));
    assert!(xml.contains("dwell=\"1.500\""));

    let imported = import_3mf_xml(&xml).expect("3MF import must succeed");
    assert_eq!(imported.segments.len(), 3);

    // Verify Segment 0 properties
    let s0 = &imported.segments[0];
    assert_eq!(s0.end[0], Some(Length::mm(15.0)));
    assert_eq!(s0.end[1], Some(Length::mm(20.0)));
    assert_eq!(s0.end[2], Some(Length::mm(5.0)));
    assert_eq!(s0.width, Some(Length::mm(0.45)));
    assert_eq!(s0.height, Some(Length::mm(0.20)));
    assert_eq!(s0.temperature, Some(220.0));
    assert_eq!(s0.fan, Some(80.0));
    let orient = s0.orientation.unwrap();
    assert!((orient[0] - 0.0).abs() < 1e-5);
    assert!((orient[1] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    assert!((orient[2] - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-5);

    // Verify Segment 1 properties (Arc)
    let s1 = &imported.segments[1];
    assert_eq!(s1.kind, SegmentKind::Arc);
    assert_eq!(s1.centre, Some([Length::mm(15.0), Length::mm(30.0)]));
    assert!(s1.clockwise);

    // Verify Segment 2 properties (Dwell)
    let s2 = &imported.segments[2];
    assert_eq!(s2.kind, SegmentKind::Dwell);
    assert_eq!(s2.dwell_s, Some(1.5));
}

#[test]
fn iso_astm_52915_error_rejection() {
    // Rejection of negative feedrates
    let bad_feedrate_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:tp="http://schemas.microsoft.com/3dmanufacturing/toolpath/2022/07">
  <build><tp:toolpath>
    <tp:segment id="0" type="line" travel="false" x="10.0" feedrate="-500.0"/>
  </tp:toolpath></build>
</model>"#;
    assert!(import_3mf_xml(bad_feedrate_xml).is_err());

    // Rejection of non-finite coordinates
    let nan_coord_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<model xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:tp="http://schemas.microsoft.com/3dmanufacturing/toolpath/2022/07">
  <build><tp:toolpath>
    <tp:segment id="0" type="line" travel="false" x="NaN" feedrate="1200.0"/>
  </tp:toolpath></build>
</model>"#;
    assert!(import_3mf_xml(nan_coord_xml).is_err());
}
