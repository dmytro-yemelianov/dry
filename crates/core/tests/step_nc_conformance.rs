//! ISO 14649 / ISO 10303-238 (STEP-NC AP 238) Compliance & Toolframe Verification Suite (Track C1.2).

use dry_core::emit::emit_step_nc;
use dry_core::ir::{Segment, SegmentKind, Toolpath};
use dry_core::units::{Feedrate, Length, Volume};

#[test]
fn iso_14649_step_nc_ap238_conformance() {
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![
            // Segment 0: 5-Axis Rapid Transit
            Segment {
                start: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(50.0)),
                ],
                end: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(20.0)),
                    Some(Length::mm(25.0)),
                ],
                travel: true,
                speed: Feedrate(6000.0),
                length: Length::mm(30.4138),
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: None,
                height: None,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: None,
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: Some([0.0, 0.0, 1.0]),
                control_points: None,
            },
            // Segment 1: Multi-Axis 5-Axis Milling Pass with Tool Orientation
            Segment {
                start: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(20.0)),
                    Some(Length::mm(25.0)),
                ],
                end: [
                    Some(Length::mm(30.0)),
                    Some(Length::mm(40.0)),
                    Some(Length::mm(20.0)),
                ],
                travel: false,
                speed: Feedrate(1500.0),
                length: Length::mm(28.7228),
                volume: Volume(1.8),
                filament: Length::ZERO,
                width: Some(Length::mm(6.0)),
                height: Some(Length::mm(2.5)),
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: Some(1),
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: Some([0.353553, 0.353553, 0.866025]),
                control_points: None,
            },
        ],
    };

    let xml = emit_step_nc(&tp, &dry_core::emit::EmitParams::default())
        .expect("STEP-NC emit must succeed");

    assert!(xml.contains("xmlns=\"urn:iso:std:iso-10303-14649\""));
    assert!(xml.contains("schema=\"ISO-10303-238:2020\""));
    assert!(xml.contains("standard=\"ISO 14649-10/11\""));
    assert!(xml.contains("<toolframe i=\"0.353553\" j=\"0.353553\" k=\"0.866025\"/>"));
    assert!(xml.contains("type=\"rapid\""));
    assert!(xml.contains("type=\"motion\""));
}
