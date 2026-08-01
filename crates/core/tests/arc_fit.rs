//! P3.1 — L2 `arc_fit` optimisation pass. Where `merge_collinear` coalesces collinear runs, `arc_fit`
//! recognises a run of consecutive line moves whose points all lie on a *common circle* (with a
//! consistent winding) and replaces it with a single G2/G3 arc — fewer segments and the same deposited
//! material, with native-arc geometric length. This pass has no FullControl oracle: it is Dry's own
//! well-specified transform, tested directly against constructed circular and non-circular cases.

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use dry_core::{arc_fit, emit, resolve, simulate, Design, EmitParams, ResolveParams, SegmentKind};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

/// Author explicit `move` ops tracing `pts` (XY at constant Z), extruding, with a fixed bead. The
/// points are chosen to lie *exactly* on a circle so the circumcircle fit is numerically clean.
fn polyline(pts: &[(f64, f64)], z: f64) -> Design {
    let mut ops =
        String::from(r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true}"#);
    for (x, y) in pts {
        ops.push_str(&format!(
            ",{{\"op\":\"move\",\"x\":{x},\"y\":{y},\"z\":{z}}}"
        ));
    }
    ops.push(']');
    design(&ops)
}

/// Seven integer points on the radius-5 circle about the origin (Pythagorean: every `x²+y²==25`),
/// traced counter-clockwise. Exact f64 coordinates ⇒ the circumcircle fit is exact.
fn circle_pts() -> Vec<(f64, f64)> {
    vec![
        (5.0, 0.0),
        (4.0, 3.0),
        (3.0, 4.0),
        (0.0, 5.0),
        (-3.0, 4.0),
        (-4.0, 3.0),
        (-5.0, 0.0),
    ]
}

fn circle_run() -> Design {
    polyline(&circle_pts(), 0.2)
}

#[test]
fn circular_run_collapses_to_an_arc() {
    let tp = resolve(&circle_run(), &ResolveParams::default());
    let opt = arc_fit(&tp);
    // the run of line moves becomes (at least) one arc; segment count drops.
    assert!(opt.segments.len() < tp.segments.len());
    let arc = opt
        .segments
        .iter()
        .find(|s| s.kind == SegmentKind::Arc)
        .expect("a fitted arc segment");
    let [cx, cy] = arc.centre.expect("the arc carries a centre");
    assert!(cx.value().abs() < 1e-6, "centre x ~ 0, got {}", cx.value());
    assert!(cy.value().abs() < 1e-6, "centre y ~ 0, got {}", cy.value());
    // the arc spans the circular run: it starts at the first defined point (5,0) and ends at the
    // last one (-5,0). The leading positioning move (undefined start) is not part of the arc.
    let arc_start = arc.start.map(|o| o.map(|v| v.value()));
    let arc_end = arc.end.map(|o| o.map(|v| v.value()));
    assert_eq!(arc_start, [Some(5.0), Some(0.0), Some(0.2)]);
    assert_eq!(arc_end, [Some(-5.0), Some(0.0), Some(0.2)]);
    assert_eq!(arc.end, tp.segments.last().unwrap().end);
}

#[test]
fn arc_fit_preserves_extruded_volume() {
    let tp = resolve(&circle_run(), &ResolveParams::default());
    let opt = arc_fit(&tp);
    let (a, b) = (simulate(&tp), simulate(&opt));
    assert!(
        (a.extruded_volume.value() - b.extruded_volume.value()).abs() < 1e-9,
        "volume preserved: {} vs {}",
        a.extruded_volume.value(),
        b.extruded_volume.value()
    );
    assert!(b.segment_count < a.segment_count);
}

#[test]
fn fitted_arc_emits_g2_or_g3() {
    let tp = resolve(&circle_run(), &ResolveParams::default());
    let opt = arc_fit(&tp);
    let gcode = emit(&opt, &EmitParams::default());
    assert!(
        gcode
            .iter()
            .any(|l| l.starts_with("G2 ") || l.starts_with("G3 ")),
        "expected a G2/G3 line, got:\n{}",
        gcode.join("\n")
    );
}

#[test]
fn zigzag_is_not_fitted() {
    // a non-circular zig-zag: its points do not lie on any common circle.
    let tp = resolve(
        &polyline(
            &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 1.0), (4.0, 0.0)],
            0.2,
        ),
        &ResolveParams::default(),
    );
    let opt = arc_fit(&tp);
    assert_eq!(opt.segments.len(), tp.segments.len());
    assert!(opt.segments.iter().all(|s| s.kind != SegmentKind::Arc));
}

#[test]
fn arc_fit_is_idempotent() {
    let tp = resolve(&circle_run(), &ResolveParams::default());
    let once = arc_fit(&tp);
    let twice = arc_fit(&once);
    assert_eq!(once, twice);
}

#[test]
fn empty_and_short_runs_pass_through() {
    // two line moves (< 3) cannot justify an arc.
    let tp = resolve(
        &polyline(&[(5.0, 0.0), (4.0, 3.0), (3.0, 4.0)], 0.2),
        &ResolveParams::default(),
    );
    let opt = arc_fit(&tp);
    // three points = two line segments: too few to fit an arc, so unchanged.
    assert_eq!(opt.segments.len(), tp.segments.len());
}
