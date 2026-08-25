//! P2.x — the `Spline` L1 op (Catmull-Rom). A spline starts at the running position (P0) and passes
//! through each control point; `resolve` keeps the curve intact in the L2 toolpath as a first-class
//! spline segment, and `emit` lowers/resolves it to a chain of line segments.

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use kmet_kernel::{emit, resolve, Design, EmitParams, ResolveParams, SegmentKind};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn spline_keeps_curves_intact_in_l2_toolpath_and_lowers_in_emit() {
    // start at (0,0,0.2) via a move, then a spline through three control points.
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"spline","points":[[10,0,0.2],[10,10,0.2],[0,10,0.2]]}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());

    // We expect exactly 2 segments: 1 line segment + 1 spline segment.
    assert_eq!(tp.segments.len(), 2);
    assert_eq!(tp.segments[0].kind, SegmentKind::Line);
    assert_eq!(tp.segments[1].kind, SegmentKind::Spline);

    let spline_seg = &tp.segments[1];
    assert!(!spline_seg.travel);
    assert!(spline_seg.control_points.is_some());
    let ctrl_pts = spline_seg.control_points.as_ref().unwrap();
    assert_eq!(ctrl_pts.len(), 3);
    assert_eq!(ctrl_pts[0][0].value(), 10.0);
    assert_eq!(ctrl_pts[2][1].value(), 10.0);

    // Emit should lower/resolve the spline into 48 sub-moves.
    // 1 positioning move + 48 spline sub-moves = 49 total g-code lines.
    let gcode = emit(&tp, &EmitParams::default());
    assert_eq!(gcode.len(), 49);
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
    let gcode = emit(&tp, &EmitParams::default());

    // Parse emitted X and Y coordinates to verify it passes through control points
    let mut coordinates = Vec::new();
    for line in gcode {
        if line.starts_with("G1") {
            let mut x = None;
            let mut y = None;
            for token in line.split_whitespace() {
                if let Some(value) = token.strip_prefix('X') {
                    x = value.parse::<f64>().ok();
                } else if let Some(value) = token.strip_prefix('Y') {
                    y = value.parse::<f64>().ok();
                }
            }
            if let (Some(xv), Some(yv)) = (x, y) {
                coordinates.push((xv, yv));
            }
        }
    }

    for (cx, cy) in pts {
        let hit = coordinates
            .iter()
            .any(|&(x, y)| (x - cx).abs() < 1e-9 && (y - cy).abs() < 1e-9);
        assert!(
            hit,
            "emitted path must pass through control point ({cx},{cy})"
        );
    }
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
    let gcode = emit(&tp, &EmitParams::default());

    for line in gcode {
        if line.starts_with("G1") {
            for token in line.split_whitespace() {
                if let Some(value) = token.strip_prefix('Y') {
                    let y = value.parse::<f64>().unwrap();
                    assert!(
                        y.abs() < 1e-9,
                        "collinear spline must not deviate in y (got {y})"
                    );
                }
                if let Some(value) = token.strip_prefix('Z') {
                    let z = value.parse::<f64>().unwrap();
                    assert!((z - 0.2).abs() < 1e-9, "z stays constant (got {z})");
                }
            }
        }
    }
}

#[test]
fn gradual_corner_design_emits_curved_spline_segments() {
    // A spline with a gradual cornering profile should emit a dense, visibly curved output.
    // NOTE: this is a Catmull-Rom spline, NOT a clothoid, and the name stays as commit d47446c set
    // it. P5.5 has since landed curvature-linear (Euler spiral) cornering as its own `Op::Clothoid`
    // node -- see crates/core/src/clothoid.rs and crates/core/tests/clothoid.rs -- but this test does
    // not exercise it, and renaming a spline test after a clothoid would put the name back where
    // d47446c found it.
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},
            {"op":"spline","points":[[8,0,0.2],[16,4,0.2],[24,14,0.2],[34,26,0.2],[40,40,0.2]]}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let gcode = emit(&tp, &EmitParams::default());

    // 1 move + 5 spans * 16 samples = 81 lines.
    assert_eq!(gcode.len(), 81);

    let mut points = Vec::new();
    for line in gcode.iter().skip(1) {
        if !line.starts_with('G') {
            continue;
        }
        let mut x = None;
        let mut y = None;
        for token in line.split_whitespace() {
            if let Some(v) = token.strip_prefix('X') {
                x = v.parse::<f64>().ok();
            } else if let Some(v) = token.strip_prefix('Y') {
                y = v.parse::<f64>().ok();
            }
        }
        if let (Some(xv), Some(yv)) = (x, y) {
            points.push((xv, yv));
        }
    }

    assert_eq!(
        points.last(),
        Some(&(40.0, 40.0)),
        "spline endpoint preserved"
    );
    assert!(
        points.windows(3).any(|w| {
            let (x1, y1) = w[0];
            let (x2, y2) = w[1];
            let (x3, y3) = w[2];
            let area = (x2 - x1) * (y3 - y1) - (y2 - y1) * (x3 - x1);
            area.abs() > 1e-6
        }),
        "a gradually cornered spline should produce non-collinear intermediate points"
    );
}
