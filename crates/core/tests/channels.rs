//! P2.1 — the typed channel registry (temperature / fan / flow / tool) and the `Dwell` op. Channels are
//! authored as L1 ops, propagate with defaults through `resolve`, and ride each L2 segment; they are
//! carried for `simulate`/`verify` (and the binary codec), without disturbing the motion g-code.

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
