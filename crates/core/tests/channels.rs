//! P2.1 — the typed channel registry (temperature / fan / flow / tool / power) and the `Dwell` op. Channels are
//! authored as L1 ops, propagate with defaults through `resolve`, and ride each L2 segment; they are
//! carried for `simulate`/`verify` (and the binary codec), without disturbing the motion g-code.

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use dry_core::{
    emit, resolve, simulate, verify, Contracts, Design, EmitParams, ResolveParams, SegmentKind,
    Time,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn channels_propagate_onto_segments() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"temperature","nozzle":210},{"op":"fan","speed":0.5},{"op":"tool","index":1},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let s = &tp.segments[1];
    assert_eq!(s.temperature, Some(210.0));
    assert_eq!(s.fan, Some(0.5));
    assert_eq!(s.tool, Some(1));
}

#[test]
fn flow_multiplier_scales_deposited_volume() {
    let base = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let scaled = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"flow","ratio":0.8},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(scaled.segments[1].flow, Some(0.8));
    let got = scaled.segments[1].volume.value();
    let want = base.segments[1].volume.value() * 0.8;
    assert!((got - want).abs() < 1e-12, "got {got}, want {want}");
}

#[test]
fn dwell_adds_time_and_emits_g4() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2},
            {"op":"dwell","seconds":2.5}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let last = tp.segments.last().unwrap();
    assert_eq!(last.kind, SegmentKind::Dwell);
    assert_eq!(last.dwell_s, Some(2.5));

    let without_dwell = simulate(&resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    ));
    let m = simulate(&tp);
    assert_eq!(m.total_time_s, without_dwell.total_time_s + Time(2.5));

    let g = emit(&tp, &EmitParams::default());
    assert!(
        g.iter().any(|l| l == "G4 S2.5"),
        "expected a G4 dwell: {g:?}"
    );
}

#[test]
fn manual_gcode_resolves_emits_and_round_trips() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"manual_gcode","text":"M117 hello"},
            {"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert_eq!(tp.segments[0].kind, SegmentKind::ManualGcode);
    assert_eq!(tp.segments[0].manual_gcode.as_deref(), Some("M117 hello"));
    assert_eq!(dry_core::Toolpath::from_bytes(&tp.to_bytes()).unwrap(), tp);

    let g = emit(&tp, &EmitParams::default());
    assert_eq!(g[0], "M117 hello");
    assert!(g.iter().any(|line| line.starts_with("G1 ")));
}

#[test]
fn authored_retractions_emit_extruder_moves_and_have_time() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"retract","distance":2.0,"speed":1200.0},
            {"op":"unretract","distance":2.0,"speed":600.0},
            {"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert_eq!(tp.segments[1].kind, SegmentKind::Retract);
    assert_eq!(tp.segments[2].kind, SegmentKind::Unretract);

    let g = emit(&tp, &EmitParams::default());
    assert_eq!(g[1], "G1 F1200 E-2");
    assert_eq!(g[2], "G1 F600 E2");

    let abs = emit(
        &tp,
        &EmitParams {
            relative_e: false,
            ..EmitParams::default()
        },
    );
    assert_eq!(abs[1], "G1 F1200 E-2");
    assert_eq!(abs[2], "G1 F600 E0");

    let metrics = simulate(&tp);
    assert!(metrics.travel_time_s.value() >= 0.3);
    assert!(metrics.total_time_s.value() >= 0.9);
}

#[test]
fn default_retractions_emit_real_extruder_moves() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"retract"},
            {"op":"unretract"}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert_eq!(tp.segments[1].filament.value(), -1.0);
    assert_eq!(tp.segments[2].filament.value(), 1.0);

    let g = emit(&tp, &EmitParams::default());
    assert_eq!(g[1], "G1 F1000 E-1");
    assert_eq!(g[2], "G1 E1");
}

#[test]
fn stationary_deposit_has_print_time_and_flow() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"deposit","volume":5.0,"speed":300.0}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert_eq!(tp.segments[1].kind, SegmentKind::Deposit);

    let g = emit(&tp, &EmitParams::default());
    assert!(g[1].starts_with("G1 F300 E"));

    let metrics = simulate(&tp);
    assert_eq!(metrics.segment_count, 1);
    assert!(metrics.print_time_s.value() > 0.0);
    assert_eq!(metrics.extruding_distance.value(), 0.0);
    assert!(metrics.max_flow_rate.value() > 0.0);
}

#[test]
fn cold_extrusion_is_flagged_below_min_temp() {
    // extruding with no temperature set, against a min-temp contract → flagged.
    let cold = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let c = Contracts {
        min_temp: Some(180.0),
        ..Contracts::default()
    };
    assert!(verify(&cold, &c)
        .findings
        .iter()
        .any(|f| f.rule == "cold-extrusion"));

    // hot enough → no finding.
    let hot = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"temperature","nozzle":205},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert!(!verify(&hot, &c)
        .findings
        .iter()
        .any(|f| f.rule == "cold-extrusion"));
}

#[test]
fn binary_codec_round_trips_channels() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"temperature","nozzle":210},{"op":"fan","speed":0.5},{"op":"tool","index":2},
                {"op":"flow","ratio":0.9},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2},
                {"op":"dwell","seconds":1.0}]"#,
        ),
        &ResolveParams::default(),
    );
    let back = dry_core::Toolpath::from_bytes(&tp.to_bytes()).unwrap();
    assert_eq!(back, tp);
}

#[test]
fn power_channel_propagates_and_reaches_dwells() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},
                {"op":"power","level":600},
                {"op":"move","x":10,"y":0,"z":0.2},
                {"op":"dwell","seconds":0.5},
                {"op":"power","level":0},
                {"op":"move","x":20,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    // Before the first `power` op the channel is *unset*, not zero: "never commanded" and
    // "commanded off" are different machine states.
    assert_eq!(tp.segments[0].power, None);
    assert_eq!(tp.segments[1].power, Some(600.0));
    assert_eq!(tp.segments[2].kind, SegmentKind::Dwell);
    assert_eq!(tp.segments[2].power, Some(600.0));
    assert_eq!(tp.segments[3].power, Some(0.0));
}

#[test]
fn power_channel_survives_both_binary_forms_and_json() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"power","level":1200.5},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(
        dry_core::Toolpath::from_bytes(&tp.to_bytes()).unwrap(),
        tp,
        "DRY0 round trip"
    );
    assert_eq!(
        dry_core::Toolpath::from_bytes(&tp.to_streaming_bytes()).unwrap(),
        tp,
        "DRY1 round trip"
    );
    assert_eq!(dry_core::Toolpath::from_json(&tp.to_json()).unwrap(), tp);

    // The DRY0 header records the minimum reader version the body needs (spec §5.3): the power
    // column bumps it, and a power-free toolpath must be left at the older layout.
    assert_eq!(tp.to_bytes()[4], 2, "DRY0 enc_ver with power");
    let plain = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(plain.to_bytes()[4], 1, "DRY0 enc_ver without power");
    // DRY1 needs no bump: the row's flag word already says whether the field is there.
    assert_eq!(tp.to_streaming_bytes()[4], 2);
    assert_eq!(plain.to_streaming_bytes()[4], 2);
}

#[test]
fn power_ingress_refuses_negative_and_non_finite_levels() {
    for bad in ["-1", "NaN", "1e400"] {
        let json = format!(
            r#"{{"ops":[{{"op":"power","level":{bad}}},{{"op":"move","x":1,"y":0,"z":0.2}}]}}"#
        );
        // serde_json rejects the literals it cannot represent; the ones it accepts must be refused
        // by `validate_design` rather than reaching the IR.
        let Ok(d) = serde_json::from_str::<Design>(&json) else {
            continue;
        };
        let err = dry_core::resolve_checked(&d, &ResolveParams::default())
            .expect_err("power {bad} must be refused at ingress");
        assert!(
            err.to_string().contains("ops[0].level"),
            "refusal must name the offending op field, got: {err}"
        );
    }
}

/// The optimiser must never delete a commanded power transition. `merge_collinear` coalesces
/// collinear moves that share *all* process state, so the beam-off has to be part of that state:
/// merging `S600, S600, M5` into two moves leaves the laser lit across the move the program
/// authored dark. End-to-end (resolve → `optimize_pipeline` → emit), not a predicate unit test,
/// because the predicate is only reachable through the pipeline every published surface calls.
#[test]
fn optimize_pipeline_preserves_a_commanded_beam_off() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"power","level":600},
                {"op":"move","x":0,"y":0,"z":0.2},
                {"op":"move","x":10,"y":0,"z":0.2},
                {"op":"power","level":0},
                {"op":"move","x":20,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let powers: Vec<Option<f64>> = tp.segments.iter().map(|s| s.power).collect();
    assert_eq!(
        powers,
        vec![Some(600.0), Some(600.0), Some(0.0)],
        "resolved power channel"
    );

    let optimized = dry_core::optimize_pipeline(&tp);
    let optimized_powers: Vec<Option<f64>> = optimized.segments.iter().map(|s| s.power).collect();
    assert_eq!(
        optimized_powers,
        vec![Some(600.0), Some(600.0), Some(0.0)],
        "the optimiser must not merge across a power change"
    );

    let grbl = EmitParams {
        flavor: dry_core::FirmwareFlavor::Grbl,
        ..EmitParams::default()
    };
    let lines = dry_core::emit_stream(optimized.segments.iter().cloned().map(Ok), &grbl)
        .expect("grbl emit");
    let off = lines
        .iter()
        .position(|l| l == "M5")
        .expect("the commanded beam-off must survive optimisation");
    assert!(
        lines[off + 1..].iter().any(|l| l.starts_with("G1 X20")),
        "the dark move must follow the M5, not precede it:\n{lines:#?}"
    );
}

/// The same hazard through `arc_fit`: a same-state run is what becomes one arc, and an arc carries a
/// single power. A run that spans a power change must break into two runs, or the change is gone.
#[test]
fn arc_fit_does_not_swallow_a_power_change() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"power","level":600},
                {"op":"move","x":10.0,"y":0.0,"z":0.2},
                {"op":"move","x":9.659258262890683,"y":2.5881904510252074,"z":0.2},
                {"op":"move","x":8.660254037844387,"y":4.999999999999999,"z":0.2},
                {"op":"move","x":7.0710678118654755,"y":7.071067811865475,"z":0.2},
                {"op":"power","level":0},
                {"op":"move","x":5.000000000000001,"y":8.660254037844386,"z":0.2},
                {"op":"move","x":2.5881904510252074,"y":9.659258262890683,"z":0.2},
                {"op":"move","x":6.123233995736766e-16,"y":10.0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(tp.segments.len(), 7, "one positioning move plus six chords");

    let optimized = dry_core::optimize_pipeline(&tp);
    let powers: Vec<Option<f64>> = optimized.segments.iter().map(|s| s.power).collect();
    assert!(
        powers.contains(&Some(0.0)),
        "arc fitting deleted the beam-off: {powers:?}"
    );
    // The lit run and the dark run each fit their own arc; nothing spans the transition.
    assert_eq!(
        optimized
            .segments
            .iter()
            .filter(|s| s.kind == SegmentKind::Arc)
            .count(),
        2,
        "each same-power run should fit its own arc: {:?}",
        optimized
            .segments
            .iter()
            .map(|s| (s.kind, s.power))
            .collect::<Vec<_>>()
    );
}

/// Three laser islands, each lit only while it cuts: every rapid between them is authored dark
/// (`power 0` before the non-extruding move). Reordering them may not change that.
fn dark_rapid_islands() -> Design {
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"extruder","on":false},{"op":"move","x":10,"y":0,"z":0.2},
            {"op":"power","level":600},
            {"op":"extruder","on":true},{"op":"move","x":20,"y":0,"z":0.2},
            {"op":"power","level":0},
            {"op":"extruder","on":false},{"op":"move","x":50,"y":0,"z":0.2},
            {"op":"power","level":300},
            {"op":"extruder","on":true},{"op":"move","x":60,"y":0,"z":0.2},
            {"op":"power","level":0},
            {"op":"extruder","on":false},{"op":"move","x":30,"y":0,"z":0.2},
            {"op":"power","level":300},
            {"op":"extruder","on":true},{"op":"move","x":40,"y":0,"z":0.2}]"#,
    )
}

/// Every rapid in an emitted GRBL program that runs with the beam lit, read off the program text the
/// way the controller reads it: `S` sets the level, `M3` arms it, `M5` disarms it, all modal.
fn lit_rapids(lines: &[String]) -> Vec<String> {
    let mut level = 0.0_f64;
    let mut armed = false;
    let mut hits = Vec::new();
    for line in lines {
        for word in line.split_whitespace() {
            match word {
                "M3" => armed = true,
                "M5" => armed = false,
                w => {
                    if let Some(rest) = w.strip_prefix('S') {
                        level = rest.parse().expect("an S word carries a number");
                    }
                }
            }
        }
        if line.starts_with("G0") && armed && level > 0.0 {
            hits.push(format!("{line}   [beam armed at S{level}]"));
        }
    }
    hits
}

fn grbl_lines(tp: &dry_core::Toolpath) -> Vec<String> {
    let params = EmitParams {
        flavor: dry_core::FirmwareFlavor::Grbl,
        ..EmitParams::default()
    };
    dry_core::emit_stream(tp.segments.iter().cloned().map(Ok), &params).expect("grbl emit")
}

/// F1/NEW-1: the same hazard as the merge, reached through the pass that *synthesises* motion.
/// `travel_reorder` regenerates every connecting travel; taking the beam state from an arbitrary
/// original travel deleted two commanded `M5`s and commanded the laser **up** to S600 for a 10 mm
/// rapid. A rapid the program authored dark must still be dark after the reorder.
#[test]
fn travel_reorder_leaves_a_dark_rapid_dark() {
    let tp = resolve(&dark_rapid_islands(), &ResolveParams::default());
    let before = grbl_lines(&tp);
    assert!(
        lit_rapids(&before).is_empty(),
        "the authored program already cuts on a rapid; the fixture is wrong:\n{:#?}",
        lit_rapids(&before)
    );
    let beam_offs = |l: &[String]| l.iter().filter(|x| x.as_str() == "M5").count();

    let opt = dry_core::travel_reorder(&tp);
    let after = grbl_lines(&opt);
    assert!(
        lit_rapids(&after).is_empty(),
        "travel_reorder lit a rapid:\n{:#?}\nfull program:\n{after:#?}",
        lit_rapids(&after)
    );
    assert_eq!(
        beam_offs(&after),
        beam_offs(&before),
        "a commanded beam-off was deleted: {before:#?}\n→\n{after:#?}"
    );
}

/// The same property through the pipeline the CLI's `--reorder-travel` and the bindings actually run,
/// where `coasting` and `z_hop` also rewrite the stream around the reorder — `z_hop` in particular
/// replaces each travel with three, copying its beam state onto all of them.
#[test]
fn optimize_aggressive_pipeline_leaves_a_dark_rapid_dark() {
    let tp = resolve(&dark_rapid_islands(), &ResolveParams::default());
    let opt = dry_core::optimize_aggressive_pipeline(&tp);
    let after = grbl_lines(&opt);
    assert!(
        lit_rapids(&after).is_empty(),
        "the aggressive pipeline lit a rapid:\n{:#?}\nfull program:\n{after:#?}",
        lit_rapids(&after)
    );
    assert!(
        after.iter().any(|l| l == "M5"),
        "every commanded beam-off vanished:\n{after:#?}"
    );
}

#[test]
fn reverse_round_trips_the_power_channel() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"power","level":600},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    let design_back = dry_core::reverse(&tp).expect("reverse");
    let again = resolve(&design_back, &ResolveParams::default());
    assert_eq!(
        again.segments.last().unwrap().power,
        tp.segments.last().unwrap().power
    );
}
