//! The emitter must reject what it cannot faithfully represent.
//!
//! `dry emit` never runs the verifier — it streams IR straight to the emitter — so `emit` is the
//! last gate before a machine. These tests pin the three rejections that guard the g-code *words*
//! themselves: non-finite values (which `format!("{v:.6}")` renders as the syntactically valid
//! words `NaN`/`inf`), an out-of-contract `CncFrame` reaching the `pub`/`Deserialize` path without
//! passing `Profile::validate`, and an arc with no endpoint (a full 360° circle on RS-274, not the
//! no-op the missing words suggest).
//!
//! The five-axis orientation rejection lives in `tests/kinematics.rs`, next to the model helpers.

use dry_core::{
    emit, emit_stream, CncFrame, EmitParams, Feedrate, FirmwareFlavor, Length, Segment,
    SegmentKind, Toolpath, Volume,
};

/// An otherwise valid extruding line move from the origin to `end`.
fn line_to(end: [f64; 3]) -> Segment {
    Segment {
        start: [
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        end: [
            Some(Length::mm(end[0])),
            Some(Length::mm(end[1])),
            Some(Length::mm(end[2])),
        ],
        travel: false,
        speed: Feedrate(1200.0),
        length: Length::mm(10.0),
        volume: Volume(0.8),
        filament: Length::mm(0.32),
        width: Some(Length::mm(0.4)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
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
    }
}

fn tp(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments,
    }
}

/// Emit through the infallible entry point without tripping its `debug_assert`, so one assertion
/// covers both build profiles: debug panics, release returns no lines, neither writes a word.
fn emit_refusing(toolpath: &Toolpath, params: &EmitParams) -> Vec<String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| emit(toolpath, params)))
        .unwrap_or_default()
}

fn assert_refused(segments: Vec<Segment>, params: &EmitParams, expected: &str) {
    let error = emit_stream(segments.clone().into_iter().map(Ok), params)
        .expect_err("emit should refuse this program")
        .to_string();
    assert!(
        error.contains(expected),
        "expected an error mentioning {expected:?}, got {error:?}"
    );
    assert!(
        emit_refusing(&tp(segments), params).is_empty(),
        "the infallible emit() must refuse the whole program, not emit part of it"
    );
}

// ---- C1: non-finite values ------------------------------------------------------------------

#[test]
fn non_finite_coordinates_are_refused() {
    for (axis, bad) in [(0usize, 'X'), (1, 'Y'), (2, 'Z')] {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut segment = line_to([10.0, 0.0, 0.2]);
            segment.end[axis] = Some(Length::mm(value));
            assert_refused(
                vec![segment],
                &EmitParams::default(),
                &format!("non-finite {bad}"),
            );
        }
    }
}

#[test]
fn non_finite_feedrate_and_filament_are_refused() {
    for value in [f64::NAN, f64::INFINITY] {
        let mut speed = line_to([10.0, 0.0, 0.2]);
        speed.speed = Feedrate(value);
        assert_refused(vec![speed], &EmitParams::default(), "non-finite F");

        let mut filament = line_to([10.0, 0.0, 0.2]);
        filament.filament = Length::mm(value);
        assert_refused(vec![filament], &EmitParams::default(), "non-finite E");
    }
}

/// The absolute-E accumulator is a running sum, so a finite-looking segment can still push the
/// emitted `E` word non-finite.
#[test]
fn non_finite_absolute_e_accumulator_is_refused() {
    let mut segment = line_to([10.0, 0.0, 0.2]);
    segment.filament = Length::mm(f64::MAX);
    let params = EmitParams {
        relative_e: false,
        ..EmitParams::default()
    };
    assert_refused(
        vec![segment.clone(), segment.clone(), segment],
        &params,
        "non-finite E",
    );
}

#[test]
fn non_finite_arc_offsets_are_refused() {
    let mut segment = line_to([10.0, 0.0, 0.2]);
    segment.kind = SegmentKind::Arc;
    segment.centre = Some([Length::mm(f64::NAN), Length::mm(0.0)]);
    assert_refused(vec![segment], &EmitParams::default(), "non-finite I");
}

#[test]
fn non_finite_dwell_is_refused() {
    for flavor in [
        FirmwareFlavor::Marlin,
        // Klipper casts seconds to integer milliseconds, and `NaN as u64` saturates to 0 — the
        // cast must not launder the value past the gate.
        FirmwareFlavor::Klipper,
        FirmwareFlavor::Rs274,
        FirmwareFlavor::Grbl,
        FirmwareFlavor::RobotKrl,
    ] {
        let segment = Segment {
            kind: SegmentKind::Dwell,
            dwell_s: Some(f64::NAN),
            ..line_to([10.0, 0.0, 0.2])
        };
        let params = EmitParams {
            flavor,
            ..EmitParams::default()
        };
        assert_refused(vec![segment], &params, "non-finite dwell");
    }
}

/// The regression this whole slice exists for: the confirmed pre-fix output was
/// `G1 FNaN Xinf YNaN E0`, which a controller reads as a well-formed move.
#[test]
fn no_emitted_word_ever_carries_nan_or_inf() {
    let mut segment = line_to([10.0, 0.0, 0.2]);
    segment.end[0] = Some(Length::mm(f64::INFINITY));
    segment.end[1] = Some(Length::mm(f64::NAN));
    segment.speed = Feedrate(f64::NAN);

    let lines = emit_refusing(&tp(vec![segment]), &EmitParams::default());
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("NaN") || line.contains("inf")),
        "non-finite word reached the emitted program: {lines:?}"
    );
}

// ---- CncFrame on the pub/Deserialize path ----------------------------------------------------

fn framed(frame: CncFrame) -> EmitParams {
    EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(frame),
        ..EmitParams::default()
    }
}

/// `wcs: 0` used to render a bare `G0` where `G54` belongs — silently leaving the previous work
/// offset in force, i.e. cutting at the wrong origin.
#[test]
fn out_of_range_wcs_is_refused() {
    for wcs in [0u8, 1, 53, 60, 255] {
        let params = framed(CncFrame {
            wcs: Some(wcs),
            ..CncFrame::default()
        });
        assert_refused(vec![line_to([10.0, 0.0, -1.0])], &params, "wcs must be 54");
    }
}

/// `spindle_rpm: 0` used to render `S0 M3` immediately before a cutting move.
#[test]
fn non_positive_or_non_finite_spindle_rpm_is_refused() {
    for rpm in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let params = framed(CncFrame {
            spindle_rpm: Some(rpm),
            ..CncFrame::default()
        });
        assert_refused(
            vec![line_to([10.0, 0.0, -1.0])],
            &params,
            "spindle_rpm must be finite and > 0",
        );
    }
}

#[test]
fn valid_cnc_frames_are_unaffected() {
    for wcs in 54u8..=59 {
        let params = framed(CncFrame {
            wcs: Some(wcs),
            tool: Some(1),
            spindle_rpm: Some(10_000.0),
            coolant: Some(true),
        });
        let lines = emit(&tp(vec![line_to([10.0, 0.0, -1.0])]), &params);
        assert_eq!(lines[1], format!("G{wcs}"));
    }
    assert!(CncFrame::default().validate().is_ok());
}

// ---- arc without an explicit endpoint ---------------------------------------------------------

/// `G3 F1200 I-10 J0` — no X/Y word at all — is a **full 360° circle** on RS-274, not the no-op the
/// missing endpoint suggests. The importer refuses the same construct, so the round-trip corpus
/// cannot see it.
#[test]
fn arc_without_an_explicit_endpoint_is_refused() {
    for (dropped, params) in [
        (0usize, EmitParams::default()),
        (1, EmitParams::default()),
        (
            0,
            EmitParams {
                five_axis: true,
                ..EmitParams::default()
            },
        ),
    ] {
        let mut segment = line_to([10.0, 0.0, 0.2]);
        segment.kind = SegmentKind::Arc;
        segment.centre = Some([Length::mm(-10.0), Length::mm(0.0)]);
        segment.end[dropped] = None;
        assert_refused(vec![segment], &params, "arc segment needs an explicit end");
    }
}

#[test]
fn arcs_that_close_on_themselves_still_emit_their_endpoint() {
    // A genuine full circle names its endpoint; only the *implicit* one is refused.
    let mut segment = line_to([0.0, 0.0, 0.2]);
    segment.kind = SegmentKind::Arc;
    segment.centre = Some([Length::mm(-10.0), Length::mm(0.0)]);
    let lines = emit(&tp(vec![segment]), &EmitParams::default());
    assert!(
        lines.iter().any(|l| l.contains('X') && l.contains('Y')),
        "a closed arc must still carry X/Y: {lines:?}"
    );
}
