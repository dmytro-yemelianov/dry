//! Two surfaces that reach a machine or an interchange file had not inherited the H1.1 emit gate.
//!
//! The core hardening audit's unifying finding was that "non-finite quantities could reach metal":
//! `num()` printed `NaN`/`inf` verbatim as g-code words. H1.1 closed that for `emit_stream`.
//! `emit/plasma.rs` and `codec/threemf.rs`'s exporter were written afterwards, formatted their own
//! words with `{:.3}`, and reopened it — emitting the literal text `XNaN` on a plasma table, and a
//! 3MF document that is well-formed XML describing a motion no machine can execute.
//!
//! Both are only reachable with hand-built IR, which is exactly the gap H1.1 named: every
//! conformance corpus is oracle-generated and therefore well-formed by construction, so a defect
//! reachable only this way is invisible to a green suite.

use dry_core::codec::threemf::export_3mf_xml;
use dry_core::ir::{Segment, SegmentKind, Toolpath};
use dry_core::units::{Feedrate, Length, Volume};
use dry_core::{emit_plasma_waterjet, CuttingParams};

/// Builds IR directly, bypassing `resolve`, because that is the only way to hold a non-finite value:
/// `Length::mm` carries a `debug_assert` and every ingress path refuses one (H1.2).
fn segment_with(x: f64, speed: f64) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments: vec![Segment {
            start: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
            ],
            end: [
                Some(Length(x)),
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
            ],
            travel: false,
            speed: Feedrate(speed),
            length: Length::mm(10.0),
            volume: Volume(1.0),
            filament: Length::mm(0.4),
            width: Some(Length::mm(0.45)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            temperature: None,
            fan: None,
            flow: None,
            tool: None,
            power: Some(50.0),
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        }],
    }
}

#[test]
fn plasma_refuses_what_it_cannot_represent() {
    // A valid program still emits, so the guard has not disabled the emitter.
    let ok = emit_plasma_waterjet(&segment_with(10.0, 1200.0), &CuttingParams::default())
        .expect("a finite program must still emit");
    assert!(ok.iter().any(|l| l.starts_with("G01 X10.000")));
    assert!(!ok.iter().any(|l| l.contains("NaN") || l.contains("inf")));

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = emit_plasma_waterjet(&segment_with(bad, 1200.0), &CuttingParams::default())
            .expect_err("a non-finite coordinate must be refused, not printed");
        assert!(err.to_string().contains("non-finite"), "{err}");
    }

    // A non-finite *speed* is refused rather than substituted. `speed <= ZERO` is false for a NaN so
    // the segment was treated as a cut, and `speed > ZERO` is false too so it silently inherited
    // `params.cut_feedrate` — an unknown commanded speed laundered into a plausible one.
    let err = emit_plasma_waterjet(&segment_with(10.0, f64::NAN), &CuttingParams::default())
        .expect_err("a non-finite speed must be refused");
    assert!(err.to_string().contains("non-finite speed"), "{err}");
}

#[test]
fn threemf_export_refuses_what_the_importer_would_reject() {
    let ok = export_3mf_xml(&segment_with(10.0, 1200.0)).expect("a finite toolpath must export");
    assert!(ok.contains("<tp:segment"));
    assert!(!ok.contains("NaN") && !ok.contains("inf"));

    for (label, x, speed) in [
        ("coordinate", f64::NAN, 1200.0),
        ("speed", 10.0, f64::NAN),
        ("infinite coordinate", f64::INFINITY, 1200.0),
    ] {
        let err = export_3mf_xml(&segment_with(x, speed))
            .expect_err(&format!("a non-finite {label} must be refused"));
        assert!(err.message.contains("non-finite"), "{}", err.message);
    }
}
