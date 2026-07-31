//! Ingress validation (H1.2) — the paths that *feed* the emitter must refuse what it would later
//! have to refuse, so the H1.1 output gate is defence in depth rather than the only defence.
//!
//! Four ingress paths carry attacker- or slicer-controlled numbers into the IR: the binary codec,
//! G-code import, `ResolveParams`, and the 3MF import. Each test below pins one of them; each
//! failed before this slice with the trigger value named in its comment.

use dry_core::codec::{decode, encode};
use dry_core::{
    encode_chunked, import_3mf_xml, import_gcode, resolve_checked, simulate, Design, Feedrate,
    GcodeImportParams, Length, ResolveParams, Segment, SegmentKind, Time, Toolpath, Volume,
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

// ---- 1: the binary codec ---------------------------------------------------------------------

/// A `.dryc` carrying `00 00 00 00 00 00 F8 7F` in any f64 column decoded to `Length(NaN)`:
/// `DecodeLimits` bounds sizes, never values.
#[test]
fn binary_decode_rejects_non_finite_values() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        // The raw tuple constructor is deliberate: this is the hostile archive an attacker writes,
        // not something the checked constructor would ever produce.
        let mut end = line_to([10.0, 0.0, 0.2]);
        end.end[0] = Some(Length(value));
        let mut speed = line_to([10.0, 0.0, 0.2]);
        speed.speed = Feedrate(value);
        let mut filament = line_to([10.0, 0.0, 0.2]);
        filament.filament = Length(value);

        for segment in [end, speed, filament] {
            let toolpath = tp(vec![segment]);
            for buf in [encode(&toolpath), encode_chunked(&toolpath)] {
                let error = decode(&buf).expect_err("decode must refuse a non-finite value");
                assert!(
                    error.to_string().contains("non-finite"),
                    "expected a non-finite decode error, got {error}"
                );
            }
        }
    }
}

/// The asymmetry worth preserving: JSON *cannot* express these values, so only the binary form was
/// ever exposed. `serde_json` rejects the bare `NaN`/`Infinity` literals outright.
#[test]
fn json_codec_cannot_express_non_finite_values() {
    for literal in ["NaN", "Infinity", "-Infinity"] {
        let json = format!(
            r#"{{"version":0,"segments":[{{"start":[0,0,0.2],"end":[10,0,0.2],"travel":false,"speed":{literal},"length":10,"volume":0,"filament":0,"width":null,"height":null,"kind":"line","centre":null,"clockwise":false}}]}}"#
        );
        assert!(
            serde_json::from_str::<Toolpath>(&json).is_err(),
            "serde_json must refuse the {literal} literal"
        );
    }
}

// ---- 2: G-code import ------------------------------------------------------------------------

/// `M221 S1e400` parsed to `inf` (the word scanner admits exponent notation), which
/// `flow_ratio_from_percent` *detected and returned anyway*; `0.0 * inf = NaN` then put `E NaN`
/// into the following move.
#[test]
fn gcode_import_rejects_non_finite_word_values() {
    let params = GcodeImportParams::default();
    for source in [
        "G1 X0 Y0 F1200\nM221 S1e400\nG1 X10 E0\n",
        "G1 X0 Y0 F1200\nM106 S1e400\nG1 X10 E1\n",
        "G1 X1e400 Y0 F1200\n",
        "G1 X10 Y0 F1e400\n",
        "G1 Xnan Y0 F1200\n",
    ] {
        let error = import_gcode(source, &params)
            .expect_err("import must refuse a non-finite word value in {source}");
        assert!(
            error.to_string().contains("non-finite"),
            "expected a non-finite word error for {source:?}, got {error}"
        );
    }
}

/// A negative feedrate is not a slow move — it has no meaning on any machine, and it produced a
/// negative duration in `simulate`.
#[test]
fn gcode_import_rejects_negative_feedrate() {
    let error = import_gcode("G1 X10 Y0 F-1200\n", &GcodeImportParams::default())
        .expect_err("import must refuse a negative feedrate");
    assert!(
        error.to_string().contains("feedrate"),
        "expected a feedrate error, got {error}"
    );
}

/// Motion before the first `F` word is *valid* on a machine (it inherits the modal feedrate), so
/// import keeps accepting it — the zero is the honest "not stated in this file" value.
#[test]
fn gcode_import_still_accepts_motion_before_the_first_feedrate() {
    let toolpath = import_gcode(
        "G1 X10 Y0\nG1 X10 Y10 F1200\n",
        &GcodeImportParams::default(),
    )
    .expect("motion before the first F is a valid program");
    assert_eq!(toolpath.segments[0].speed, Feedrate::ZERO);
}

// ---- 3: `ResolveParams` ----------------------------------------------------------------------

/// `retraction_distance: Some(-2.0)` made `filament: Length::mm(-dist)` **positive**, so `verify`
/// classified the retract as an unretract and `max_retraction_distance` never applied. The per-op
/// `Retract { distance }` was already checked positive; the params fallback bypassed the guard.
#[test]
fn resolve_rejects_non_positive_retraction_params() {
    let design: Design = serde_json::from_str(
        r#"{"ops":[{"op":"move","x":0,"y":0,"z":0.2},{"op":"retract"},{"op":"unretract"}]}"#,
    )
    .expect("design parses");

    for bad in [-2.0, 0.0, f64::NAN, f64::INFINITY] {
        let distance = ResolveParams {
            retraction_distance: Some(bad),
            ..ResolveParams::default()
        };
        let error = resolve_checked(&design, &distance)
            .expect_err("resolve must refuse a non-positive retraction distance");
        assert!(
            error.to_string().contains("retraction_distance"),
            "expected a retraction_distance error, got {error}"
        );

        let speed = ResolveParams {
            retraction_speed: Some(bad),
            ..ResolveParams::default()
        };
        let error = resolve_checked(&design, &speed)
            .expect_err("resolve must refuse a non-positive retraction speed");
        assert!(
            error.to_string().contains("retraction_speed"),
            "expected a retraction_speed error, got {error}"
        );
    }
}

#[test]
fn resolve_still_accepts_valid_retraction_params() {
    let design: Design = serde_json::from_str(
        r#"{"ops":[{"op":"move","x":0,"y":0,"z":0.2},{"op":"retract"},{"op":"unretract"}]}"#,
    )
    .expect("design parses");
    let params = ResolveParams {
        retraction_distance: Some(2.0),
        retraction_speed: Some(2400.0),
        ..ResolveParams::default()
    };
    let toolpath = resolve_checked(&design, &params).expect("valid retraction params resolve");
    assert_eq!(toolpath.segments[1].filament, Length::mm(-2.0));
    assert_eq!(toolpath.segments[2].filament, Length::mm(2.0));
}

// ---- 4: feedrate sign and zero ---------------------------------------------------------------

/// A **zero**-speed move still contributes nothing to any metric, and that is deliberate: it is the
/// branch `Dry.Semantics.SimulateMetrics.segmentMotionTime` models, pinned by
/// `proofs/fixtures/simulate-metrics-refinement-v0.json` (case `zero-speed-without-filament-motion`
/// expects `extruding_distance = 0` for a 10 mm segment). Counting its distance here — the obvious
/// "stop dropping it" fix — breaks that refinement, so the zero case is closed at ingress instead
/// (3MF) or left to the source program (G-code modal feedrate). This test exists so the next
/// attempt fails *here*, next to the reason, rather than in the refinement corpus.
#[test]
fn zero_speed_accounting_stays_as_the_lean_model_specifies_it() {
    let mut travel = line_to([10.0, 0.0, 0.2]);
    travel.travel = true;
    travel.volume = Volume::ZERO;
    travel.filament = Length::ZERO;
    travel.speed = Feedrate::ZERO;

    let metrics = simulate(&tp(vec![travel]));
    assert_eq!(metrics.travel_distance, Length::ZERO);
    assert_eq!(metrics.total_time_s, Time::ZERO);
    assert_eq!(metrics.segment_count, 0);
}

/// A negative feedrate passed the `== ZERO` check entirely and produced a negative duration that
/// was *subtracted* from `total_time_s`. Negative speed is outside the branch the Lean model
/// specifies (the claim excludes "invalid or zero speed behavior outside the modeled branch") and
/// carries no corpus case, so this is the one accounting change the slice makes.
#[test]
fn simulate_never_accrues_negative_time() {
    let mut backwards = line_to([10.0, 0.0, 0.2]);
    backwards.speed = Feedrate(-1200.0);

    let metrics = simulate(&tp(vec![backwards]));
    assert_eq!(metrics.total_time_s, Time::ZERO); // was -0.5 s
    assert_eq!(metrics.print_time_s, Time::ZERO);
    assert_eq!(metrics.segment_count, 0);
}

// ---- 5: the 3MF import -----------------------------------------------------------------------

#[test]
fn threemf_import_rejects_invalid_attributes() {
    for attrs in [
        r#" x="nan" y="0.0" z="0.2" feedrate="1200.0""#,
        r#" x="10.0" y="0.0" z="0.2" feedrate="inf""#,
        r#" x="10.0" y="0.0" z="0.2" feedrate="-1200.0""#,
    ] {
        let xml = format!(
            "<model>\n  <build>\n    <tp:toolpath>\n      <tp:segment id=\"0\" type=\"line\" travel=\"true\"{attrs}/>\n    </tp:toolpath>\n  </build>\n</model>\n"
        );
        let error = import_3mf_xml(&xml).expect_err("3MF import must refuse an invalid attribute");
        assert!(
            !error.to_string().is_empty(),
            "expected a 3MF error for {attrs:?}"
        );
    }
}

/// A 3MF segment that moves must say how fast: the exporter always writes `feedrate` for a moving
/// segment, and a missing one silently became a zero-speed (invisible) move.
#[test]
fn threemf_import_rejects_motion_without_a_feedrate() {
    let xml = "<model>\n  <build>\n    <tp:toolpath>\n      <tp:segment id=\"0\" type=\"line\" travel=\"true\" x=\"10.0\" y=\"0.0\" z=\"0.2\"/>\n    </tp:toolpath>\n  </build>\n</model>\n";
    let error = import_3mf_xml(xml).expect_err("3MF import must refuse motion with no feedrate");
    assert!(
        error.to_string().contains("feedrate"),
        "expected a feedrate error, got {error}"
    );
}
