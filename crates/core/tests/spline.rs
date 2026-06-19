//! P2.x — the `Spline` L1 op (Catmull-Rom). A spline starts at the running position (P0) and passes
//! through each control point; `resolve` lowers it to a chain of line `Segment`s (the same deposition /
//! state / channel attachment a `Move` produces), sampling `SAMPLES` points per span. The curve
//! interpolates its control points (a Catmull-Rom passes through them at span boundaries), so the
//! resolved positions land exactly on every through-point.

use dry_core::{resolve, Design, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

// resolved end position of a segment as raw f64 (x, y, z), defaulting unset axes to 0.
fn end_xyz(s: &dry_core::Segment) -> (f64, f64, f64) {
    (
        s.end[0].map(|l| l.value()).unwrap_or(0.0),
        s.end[1].map(|l| l.value()).unwrap_or(0.0),
        s.end[2].map(|l| l.value()).unwrap_or(0.0),
    )
}

#[test]
fn spline_lowers_to_many_line_segments_and_deposits_material() {
    // start at (0,0,0.2) via a move, then a spline through three control points.
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"spline","points":[[10,0,0.2],[10,10,0.2],[0,10,0.2]]}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    // 1 positioning move + spline sub-moves. 3 spans × SAMPLES (16) = 48 spline segments.
    let spline_segs: Vec<_> = tp.segments.iter().skip(1).collect();
    assert_eq!(spline_segs.len(), 48, "expected 3 spans × 16 samples");
    for s in &spline_segs {
        assert_eq!(s.kind, "line");
        assert!(!s.travel, "extruder is on, so spline sub-moves deposit");
    }
    let total: f64 = spline_segs.iter().map(|s| s.volume.value()).sum();
    assert!(total > 0.0, "spline deposited volume must be positive");
}

#[test]
fn spline_passes_through_its_control_points() {
    let pts = [(10.0_f64, 0.0_f64), (10.0, 10.0), (0.0, 10.0)];
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"spline","points":[[10,0,0.2],[10,10,0.2],[0,10,0.2]]}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let ends: Vec<(f64, f64, f64)> = tp.segments.iter().map(end_xyz).collect();
    // every control point is hit exactly by some resolved position (span boundary).
    for (cx, cy) in pts {
        let hit = ends
            .iter()
            .any(|&(x, y, _)| (x - cx).abs() < 1e-9 && (y - cy).abs() < 1e-9);
        assert!(hit, "spline must pass through control point ({cx},{cy})");
    }
    // running position advances to the last control point.
    let last = end_xyz(tp.segments.last().unwrap());
    assert!((last.0 - 0.0).abs() < 1e-9 && (last.1 - 10.0).abs() < 1e-9);
}

#[test]
fn collinear_control_points_yield_a_straight_path() {
    // control points all on the x-axis line y=0 → the spline stays (near-)straight.
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"spline","points":[[5,0,0.2],[10,0,0.2],[20,0,0.2]]}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    for s in tp.segments.iter().skip(1) {
        let (_, y, z) = end_xyz(s);
        assert!(
            y.abs() < 1e-9,
            "collinear spline must not deviate in y (got {y})"
        );
        assert!((z - 0.2).abs() < 1e-9, "z stays constant (got {z})");
    }
}
