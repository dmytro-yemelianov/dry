use dry_core::{
    adaptive_speed_with_params, coasting_with_dist, optimize_aggressive_pipeline,
    optimize_pipeline, resolve, simulate, z_hop_with_params, Design, Length, ResolveParams,
    SegmentKind,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn test_coasting_preserves_geometry_and_reduces_volume() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"extruder","on":false},
            {"op":"move","x":20,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );

    let original_printing = tp.segments.iter().find(|s| !s.travel).cloned().unwrap();

    let opt = coasting_with_dist(&tp, Length::mm(2.0));

    let printing_segs: Vec<_> = opt.segments.iter().filter(|s| !s.travel).collect();
    assert_eq!(printing_segs.len(), 2);

    let s_print = printing_segs[0];
    let s_coast = printing_segs[1];

    assert!((s_print.length.value() - 8.0).abs() < 1e-9);
    assert!((s_coast.length.value() - 2.0).abs() < 1e-9);

    // Geometry boundary check
    assert_eq!(s_print.start, original_printing.start);
    assert_eq!(s_coast.end, original_printing.end);
    assert_eq!(s_print.end, s_coast.start);

    // Volume check
    assert!(s_print.volume.value() > 0.0);
    assert_eq!(s_coast.volume.value(), 0.0);
    assert_eq!(s_coast.filament.value(), 0.0);
    assert!((s_print.volume.value() - original_printing.volume.value() * 0.8).abs() < 1e-9);
}

#[test]
fn test_zhop_splits_eligible_travels() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"extruder","on":false},
            {"op":"move","x":20,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );

    let opt = z_hop_with_params(&tp, Length::mm(0.5), Length::mm(5.0));

    let travel_segs: Vec<_> = opt.segments.iter().filter(|s| s.travel).collect();
    // Initially:
    // - travel to (0,0) (start is undefined, not eligible for zhop)
    // - travel to (20,0) (eligible, split into 3 moves)
    // So 1 + 3 = 4 travel segments total.
    assert_eq!(travel_segs.len(), 4);

    let lift = travel_segs[1];
    let travel = travel_segs[2];
    let lower = travel_segs[3];

    assert!((lift.length.value() - 0.5).abs() < 1e-9);
    assert!((travel.length.value() - 10.0).abs() < 1e-9);
    assert!((lower.length.value() - 0.5).abs() < 1e-9);

    assert_eq!(lift.start[2].unwrap().value(), 0.2);
    assert_eq!(lift.end[2].unwrap().value(), 0.7);
    assert_eq!(travel.start[2].unwrap().value(), 0.7);
    assert_eq!(travel.end[2].unwrap().value(), 0.7);
    assert_eq!(lower.start[2].unwrap().value(), 0.7);
    assert_eq!(lower.end[2].unwrap().value(), 0.2);
}

#[test]
fn test_coasting_leaves_spline_segments_unchanged() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"extruder","on":true},
            {"op":"spline","points":[[10,0,0.2],[10,10,0.2],[0,10,0.2]]}]"#,
        ),
        &ResolveParams::default(),
    );

    let opt = coasting_with_dist(&tp, Length::mm(2.0));
    let splines: Vec<_> = opt
        .segments
        .iter()
        .filter(|segment| segment.kind == SegmentKind::Spline)
        .collect();
    assert_eq!(splines.len(), 1);
    assert_eq!(splines[0], &tp.segments[1]);
}

#[test]
fn test_adaptive_speed_reduces_speed_on_corners() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"move","x":10,"y":10,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );

    let opt = adaptive_speed_with_params(&tp, 500.0);

    let printing_segs: Vec<_> = opt.segments.iter().filter(|s| !s.travel).collect();
    assert_eq!(printing_segs.len(), 2);

    // The 90-degree corner has dot product 0, factor is sqrt(0.5) ~ 0.707.
    let speed_before = tp
        .segments
        .iter()
        .find(|s| !s.travel)
        .unwrap()
        .speed
        .value();
    let speed_after_1 = printing_segs[0].speed.value();
    let speed_after_2 = printing_segs[1].speed.value();

    assert!((speed_after_1 - speed_before * std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    assert!((speed_after_2 - speed_before * std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-3);
}

#[test]
fn test_pipelines_preserve_total_extruded_volume() {
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"extruder","on":true},
            {"op":"move","x":10,"y":0,"z":0.2},
            {"op":"move","x":20,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );

    let safe = optimize_pipeline(&tp);
    let aggressive = optimize_aggressive_pipeline(&tp);

    let m_orig = simulate(&tp);
    let m_safe = simulate(&safe);
    let m_agg = simulate(&aggressive);

    // Standard pipeline (merge_collinear + arc_fit) preserves volume exactly
    assert!((m_orig.extruded_volume.value() - m_safe.extruded_volume.value()).abs() < 1e-9);

    // Aggressive pipeline includes coasting, which stops extrusion early.
    // The geometry is preserved, but total simulated extrusion volume is lower.
    assert!(m_agg.extruded_volume.value() < m_orig.extruded_volume.value());
}
