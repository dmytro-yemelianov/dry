//! P3.1 — L2 `travel_reorder` optimisation pass. Where `merge_collinear` and `arc_fit` rewrite the
//! geometry *within* a run, `travel_reorder` leaves every extrusion run's geometry untouched and only
//! reorders the *independent* runs (and rewrites the connecting travel moves) to shorten total travel.
//! Like `arc_fit`, this pass has no FullControl oracle: it is Dry's own well-specified transform, tested
//! directly against a constructed layout where the authored order zig-zags across the bed.

use dry_core::{
    optimize_aggressive_pipeline, optimize_pipeline, resolve, simulate, travel_reorder, Design,
    ResolveParams, Segment,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

/// Total travel distance: the sum of `length` over travel segments.
fn travel_distance(tp: &dry_core::Toolpath) -> f64 {
    tp.segments
        .iter()
        .filter(|s| s.travel)
        .map(|s| s.length.value())
        .sum()
}

/// The multiset of extruding (non-travel) segments, as a sortable, comparable key. We compare the
/// deposited geometry/material exactly: start, end, length, volume — so a reorder that preserves the
/// runs verbatim leaves this multiset unchanged.
fn extruding_key(tp: &dry_core::Toolpath) -> Vec<String> {
    let mut v: Vec<String> = tp
        .segments
        .iter()
        .filter(|s| !s.travel)
        .map(|s| {
            format!(
                "{:?}|{:?}|{:.9}|{:.9}",
                s.start.map(|o| o.map(|l| l.value())),
                s.end.map(|o| o.map(|l| l.value())),
                s.length.value(),
                s.volume.value(),
            )
        })
        .collect();
    v.sort();
    v
}

/// Three short extruded horizontal segments (islands) authored in a zig-zag order: island A near the
/// origin, then island C far away, then island B in the middle. Visiting them in authored order
/// (A → C → B) crosses the bed twice; the nearest-neighbour order (A → B → C) is shorter.
fn zigzag_islands() -> Design {
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},
            {"op":"extruder","on":false},{"op":"move","x":0,"y":0,"z":0.2},
            {"op":"extruder","on":true},{"op":"move","x":2,"y":0,"z":0.2},
            {"op":"extruder","on":false},{"op":"move","x":100,"y":0,"z":0.2},
            {"op":"extruder","on":true},{"op":"move","x":102,"y":0,"z":0.2},
            {"op":"extruder","on":false},{"op":"move","x":50,"y":0,"z":0.2},
            {"op":"extruder","on":true},{"op":"move","x":52,"y":0,"z":0.2}]"#,
    )
}

#[test]
fn reorder_shortens_total_travel() {
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    let opt = travel_reorder(&tp);
    let (before, after) = (travel_distance(&tp), travel_distance(&opt));
    assert!(
        after < before,
        "travel should strictly shorten: {before} → {after}"
    );
}

#[test]
fn standard_pipeline_does_not_reorder_travel() {
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    let safe = optimize_pipeline(&tp);
    let aggressive = optimize_aggressive_pipeline(&tp);
    assert_eq!(extruding_key(&safe), extruding_key(&tp));
    assert_eq!(safe.segments, tp.segments);
    assert!(travel_distance(&aggressive) < travel_distance(&safe));
}

#[test]
fn reorder_preserves_extruded_material_and_runs() {
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    let opt = travel_reorder(&tp);
    // total deposited volume unchanged.
    let (a, b) = (simulate(&tp), simulate(&opt));
    assert!(
        (a.extruded_volume.value() - b.extruded_volume.value()).abs() < 1e-12,
        "volume preserved: {} vs {}",
        a.extruded_volume.value(),
        b.extruded_volume.value()
    );
    // the multiset of extruding segments is identical (each run kept verbatim).
    assert_eq!(extruding_key(&tp), extruding_key(&opt));
}

#[test]
fn first_run_is_preserved() {
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    let opt = travel_reorder(&tp);
    // the first extruding segment (the start of the first run) is identical and still first.
    let first_in = |t: &dry_core::Toolpath| -> Segment {
        t.segments.iter().find(|s| !s.travel).cloned().unwrap()
    };
    assert_eq!(first_in(&tp), first_in(&opt));
}

#[test]
fn reorder_is_idempotent() {
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    let once = travel_reorder(&tp);
    let twice = travel_reorder(&once);
    assert_eq!(once, twice);
}

#[test]
fn single_run_is_unchanged() {
    // one extrusion run (no independent islands) ⇒ nothing to reorder.
    let tp = resolve(
        &design(
            r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
                {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2},
                {"op":"move","x":10,"y":10,"z":0.2}]"#,
        ),
        &ResolveParams::default(),
    );
    assert_eq!(travel_reorder(&tp), tp);
}

/// The other half of the beam-state decision: a regenerated travel commands the beam off, but only on
/// a toolpath that has a beam. Writing `power: Some(0.0)` unconditionally would mean *inventing* the
/// channel on every FFF print — and `emit` refuses a toolpath carrying `power` on any flavor but GRBL
/// rather than dropping it, so reordering an ordinary print would stop it emitting at all.
#[test]
fn reorder_does_not_invent_a_power_channel() {
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    assert!(tp.segments.iter().all(|s| s.power.is_none()));
    let opt = travel_reorder(&tp);
    assert!(
        opt.segments.iter().all(|s| s.power.is_none()),
        "the reorder gave a power-free toolpath a beam state: {:?}",
        opt.segments.iter().map(|s| s.power).collect::<Vec<_>>()
    );
    // and the default (Marlin) flavor, which has no rendering for the channel, still emits it.
    dry_core::emit_stream(
        opt.segments.iter().cloned().map(Ok),
        &dry_core::EmitParams::default(),
    )
    .expect("a reordered FFF print must still emit");
}

#[test]
fn travel_never_grows() {
    // the constructed case shortens, but the contract is the weaker `after <= before` for any input.
    let tp = resolve(&zigzag_islands(), &ResolveParams::default());
    let opt = travel_reorder(&tp);
    assert!(travel_distance(&opt) <= travel_distance(&tp) + 1e-12);
}
