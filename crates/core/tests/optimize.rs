//! P2.x — L2 optimisation passes (semantics-preserving IR→IR transforms). `merge_collinear` coalesces
//! consecutive collinear moves that share all process state into one longer move: fewer segments, the
//! *same* path and the *same* deposited material. This is the clearest demonstration of the compiler
//! thesis — a pass that rewrites the IR while preserving its meaning.

use dry_core::{merge_collinear, resolve, simulate, Design, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn straight_run() -> Design {
    // three collinear points along +X — the middle one is redundant.
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    )
}

#[test]
fn collinear_moves_merge_into_one() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let opt = merge_collinear(&tp);
    // the two length-5 collinear moves become one length-10 move.
    assert!(opt.segments.len() < tp.segments.len());
    let merged = opt
        .segments
        .iter()
        .find(|s| s.length.value() > 1.0)
        .unwrap();
    assert_eq!(merged.end, tp.segments.last().unwrap().end);
    assert!((merged.length.value() - 10.0).abs() < 1e-12);
}

#[test]
fn merge_preserves_simulation_metrics() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let opt = merge_collinear(&tp);
    let (a, b) = (simulate(&tp), simulate(&opt));
    assert!((a.total_time_s.value() - b.total_time_s.value()).abs() < 1e-12);
    assert!((a.extruded_volume.value() - b.extruded_volume.value()).abs() < 1e-12);
    assert!((a.extruding_distance.value() - b.extruding_distance.value()).abs() < 1e-12);
    assert!((a.max_flow_rate.value() - b.max_flow_rate.value()).abs() < 1e-12);
    // the whole point: fewer counted moves.
    assert!(b.segment_count < a.segment_count);
}

#[test]
fn direction_changes_are_not_merged() {
    // a square: every corner turns 90°, so nothing collinear merges.
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2},
                {"op":"move","x":10,"y":10,"z":0.2},{"op":"move","x":0,"y":10,"z":0.2},
                {"op":"move","x":0,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(merge_collinear(&tp).segments.len(), tp.segments.len());
}

#[test]
fn differing_state_breaks_the_run() {
    // a speed change between two otherwise-collinear moves prevents the merge.
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2},
                {"op":"speed","print":2000},{"op":"move","x":10,"y":0,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(merge_collinear(&tp).segments.len(), tp.segments.len());
}

#[test]
fn merge_is_idempotent() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let once = merge_collinear(&tp);
    let twice = merge_collinear(&once);
    assert_eq!(once, twice);
}
