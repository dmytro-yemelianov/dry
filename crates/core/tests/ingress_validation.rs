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
        power: None,
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
        let Err(error) = import_gcode(source, &params) else {
            panic!("import must refuse a non-finite word value in {source:?}");
        };
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

/// Checking the *parsed* word is not enough: the arithmetic between the scanner and the IR
/// overflows finite words. `point_dist` squares the deltas, `G20` scales every coordinate, feedrate
/// and extrusion by 25.4, `G92` writes a converted origin straight into the position, and a flow
/// ratio multiplies the deposited length before it meets the filament cross-section. Each of these
/// reached `Length::mm`/`Feedrate` with a non-finite value — `Length(inf)` in the IR of a release
/// build, and a `debug_assert` panic in a debug one, from a 40-byte file.
#[test]
fn gcode_import_rejects_values_that_overflow_after_parsing() {
    let params = GcodeImportParams::default();
    for (source, expected) in [
        // `point_dist` across the f64 range.
        (
            "G1 X0 Y0 F1200\nG1 X1e308 Y1e308 F1200\n",
            "line 2: move length is not finite (inf)",
        ),
        // inch → mm on a coordinate.
        (
            "G20\nG1 X1e307 F1200\n",
            "line 2: coordinate X is not finite (inf)",
        ),
        // inch → mm on the feedrate.
        ("G20\nG1 X1 F1e307\n", "is not finite after unit conversion"),
        // inch → mm on the extrusion axis.
        (
            "G20\nG1 X1 E1e307 F100\n",
            "line 2: extrusion is not finite (inf)",
        ),
        // a relative move that walks off the end of the range.
        (
            "G91\nG1 X1e308 F100\nG1 X1e308\n",
            "line 3: move length is not finite (inf)",
        ),
        // `G92` seeds the position, so it must be checked where it is written.
        ("G20\nG92 X1e307\n", "is not finite after unit conversion"),
        // `M221` scales the deposit; both factors stay finite while the product does not.
        (
            "M221 S1e300\nG1 X1 E1e10 F100\n",
            "line 2: deposited volume is not finite (inf)",
        ),
    ] {
        let Err(error) = import_gcode(source, &params) else {
            panic!("import must refuse an overflowing value in {source:?}");
        };
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} for {source:?}, got {error}"
        );
    }
}

/// A finite `filament_diameter` can still square to a non-finite cross-section, which would then
/// make every extruding segment's `volume` non-finite. `GcodeImportParams` is caller JSON on the
/// wasm and PyO3 surfaces.
#[test]
fn gcode_import_rejects_a_diameter_with_no_finite_cross_section() {
    let params = GcodeImportParams {
        filament_diameter: 1e200,
        ..GcodeImportParams::default()
    };
    let Err(error) = import_gcode("G1 X1 E1 F100\n", &params) else {
        panic!("import must refuse a diameter with no finite cross-section");
    };
    assert!(
        error.to_string().contains("finite cross-section"),
        "expected a cross-section error, got {error}"
    );
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

/// `validate_design` bounds its inputs with `is_finite` and no magnitude, which does not survive
/// `dist`: two ops 1e200 apart square to `Area(inf)`, and `Area::sqrt` returns `Some(Length(inf))`
/// because `inf >= 0.0`. Schema-valid JSON therefore put a non-finite length in the IR on every
/// `resolve_*` surface — the same "gate the input, not the constructed quantity" seam H1.2 closed
/// in the two importers.
#[test]
fn resolve_rejects_a_design_whose_lowered_distance_overflows() {
    let design: Design = serde_json::from_str(
        r#"{"ops":[{"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":1e200,"y":0,"z":0.2}]}"#,
    )
    .expect("design parses");

    let Err(error) = resolve_checked(&design, &ResolveParams::default()) else {
        panic!("resolve must refuse a design whose lowered distance overflows");
    };
    assert!(
        error
            .to_string()
            .contains("segments[1].length resolved to inf"),
        "expected a lowered-length error, got {error}"
    );
}

/// A finite, positive `dia` is not enough: `π·(dia/2)²` underflows to zero below `dia ≈ 4e-162`,
/// and every extruding op divides by it. `Op::Deposit` produced `Length(inf)` filament, a travel's
/// `0.0 / 0.0` produced `Length(NaN)`, and `simulate` then read the `NaN` back through
/// `Length::mm(s.filament.value().abs())` — tripping the `debug_assert` in a debug build and
/// poisoning `total_time_s` in a release one.
#[test]
fn resolve_rejects_a_diameter_with_no_bead_cross_section() {
    let design: Design = serde_json::from_str(
        r#"{"ops":[{"op":"move","x":0,"y":0,"z":0.2},{"op":"deposit","volume":1.0,"speed":600}]}"#,
    )
    .expect("design parses");

    for dia in [1e-200, 1e200] {
        let params = ResolveParams {
            dia,
            ..ResolveParams::default()
        };
        let Err(error) = resolve_checked(&design, &params) else {
            panic!("resolve must refuse dia {dia:e}, which has no usable bead cross-section");
        };
        assert!(
            error.to_string().contains("bead cross-section"),
            "expected a cross-section error for dia {dia:e}, got {error}"
        );
    }
}

/// The ordinary diameters stay accepted — the guard above must not be a magnitude policy in
/// disguise.
#[test]
fn resolve_still_accepts_ordinary_filament_diameters() {
    let design: Design = serde_json::from_str(
        r#"{"ops":[{"op":"move","x":0,"y":0,"z":0.2},{"op":"deposit","volume":1.0,"speed":600}]}"#,
    )
    .expect("design parses");

    for dia in [1.75, 2.85, 3.0, 1e-6, 1e6] {
        let params = ResolveParams {
            dia,
            ..ResolveParams::default()
        };
        let toolpath = resolve_checked(&design, &params)
            .unwrap_or_else(|e| panic!("dia {dia} must resolve, got {e}"));
        assert!(toolpath
            .segments
            .iter()
            .all(|s| s.filament.value().is_finite()));
    }
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
    // The corpus case is an *extruding* segment, not a travel: `length = 10`, `speed = 0`,
    // `filament = 6`, `volume = 3`. Reproduce it exactly, because the two halves of its expectation
    // pull in opposite directions and only the pair pins the model.
    let mut extruding = line_to([10.0, 0.0, 0.2]);
    extruding.travel = false;
    extruding.length = Length::mm(10.0);
    extruding.speed = Feedrate::ZERO;
    extruding.filament = Length::mm(6.0);
    extruding.volume = Volume(3.0);

    let metrics = simulate(&tp(vec![extruding]));
    // (a) the move is un-timeable, so it contributes no distance, no time and no segment count …
    assert_eq!(metrics.extruding_distance, Length::ZERO);
    assert_eq!(metrics.travel_distance, Length::ZERO);
    assert_eq!(metrics.print_time_s, Time::ZERO);
    assert_eq!(metrics.total_time_s, Time::ZERO);
    assert_eq!(metrics.segment_count, 0);
    // … (b) but the *materials* still accrue: `withMaterials` in
    // `formal/Dry/Semantics/SimulateMetrics.lean` adds volume and filament on every segment,
    // un-timeable or not. Dropping the segment wholesale would break the model just as surely as
    // counting its distance would.
    assert_eq!(metrics.extruded_volume, Volume(3.0));
    assert_eq!(metrics.filament_length, Length::mm(6.0));
    assert_eq!(metrics.max_flow_rate, dry_core::Flow::ZERO);
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
    // Each case must fail for *its own* reason: asserting only that some error came back passes
    // just as happily when a later guard rejects the segment for something unrelated.
    for (attrs, expected) in [
        (
            r#" x="nan" y="0.0" z="0.2" feedrate="1200.0""#,
            r#"3MF error: attribute x="nan" is not a finite length"#,
        ),
        (
            r#" x="10.0" y="0.0" z="0.2" feedrate="inf""#,
            r#"3MF error: attribute feedrate="inf" is not finite"#,
        ),
        (
            r#" x="10.0" y="0.0" z="0.2" feedrate="-1200.0""#,
            "3MF error: segment feedrate must not be negative",
        ),
    ] {
        let xml = format!(
            "<model>\n  <build>\n    <tp:toolpath>\n      <tp:segment id=\"0\" type=\"line\" travel=\"true\"{attrs}/>\n    </tp:toolpath>\n  </build>\n</model>\n"
        );
        let Err(error) = import_3mf_xml(&xml) else {
            panic!("3MF import must refuse {attrs:?}");
        };
        assert_eq!(error.to_string(), expected, "wrong rejection for {attrs:?}");
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

/// …and dry's own exporter must therefore *be* a legitimate producer of `feedrate="0.0"`. A
/// zero-speed moving segment is exactly what the G-code importer preserves for motion before the
/// first `F`; writing the attribute only when `speed > 0` made that export un-importable, so the
/// rejection above broke dry's own round-trip.
/// The second case is the one that matters: the *first* segment of a G-code import has an undefined
/// start, so its IR `length` is zero even though it moves. Keying the export guard on `seg.length`
/// therefore omitted `feedrate` for it and the re-import failed — unless the program happened to
/// start at the importer's implicit origin, which is why an `X0 Y0` fixture alone proves nothing.
/// The guard mirrors the importer's own running-position delta instead.
#[test]
fn threemf_round_trips_a_zero_speed_moving_segment() {
    for source in [
        "G1 X0 Y0\nG1 X10 Y0\nG1 X10 Y10\n",
        // first motion away from the origin — segment 0 moves with `length == 0`.
        "G1 X10 Y0\nG1 X20 Y0 F1200\n",
        // never any `F` at all, off origin.
        "G1 X10 Y5\nG1 X20 Y5\n",
    ] {
        let toolpath = import_gcode(source, &GcodeImportParams::default())
            .expect("motion before the first F is a valid program");
        assert_eq!(toolpath.segments[0].speed, Feedrate::ZERO);

        let xml = dry_core::export_3mf_xml(&toolpath);
        assert!(
            xml.contains(r#"feedrate="0.0""#),
            "a moving zero-speed segment must still carry its feedrate for {source:?}:\n{xml}"
        );
        let reimported = import_3mf_xml(&xml)
            .unwrap_or_else(|e| panic!("dry's own 3MF export must re-import {source:?}: {e}"));
        assert_eq!(reimported.segments.len(), toolpath.segments.len());
    }
}

/// `parse_length_attr` admits only finite text, but the squared deltas overflow it: `x="1e308"`
/// against an origin of zero produced `Length(inf)` in release and tripped `Length::mm`'s
/// `debug_assert` in debug.
#[test]
fn threemf_import_rejects_a_length_that_overflows_after_parsing() {
    let xml = "<model>\n  <build>\n    <tp:toolpath>\n      <tp:segment id=\"0\" type=\"line\" travel=\"true\" x=\"1e308\" y=\"1e308\" z=\"0.2\" feedrate=\"1200.0\"/>\n    </tp:toolpath>\n  </build>\n</model>\n";
    let Err(error) = import_3mf_xml(xml) else {
        panic!("3MF import must refuse a length that overflows");
    };
    assert_eq!(error.to_string(), "3MF error: segment length is not finite");
}
