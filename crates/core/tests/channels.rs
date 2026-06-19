//! P2.1 — the typed channel registry (temperature / fan / flow / tool) and the `Dwell` op. Channels are
//! authored as L1 ops, propagate with defaults through `resolve`, and ride each L2 segment; they are
//! carried for `simulate`/`verify` (and the binary codec), without disturbing the motion g-code.

use dry_core::{
    emit, resolve, simulate, verify, Contracts, Design, EmitParams, ResolveParams, Time,
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
    assert_eq!(last.kind, "dwell");
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
