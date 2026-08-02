//! The `safe` optimisation mode gate (`docs/11-profiles-and-reports.md`).
//!
//! `safe` runs only the geometry-canonicalisation subset (`merge_collinear` then `arc_fit`) and is
//! *gated*: a rewritten span is only accepted when it introduces **no new error rule** relative to the
//! input. Pre-existing errors do not block; warning-only new findings do not block. A rejected span is
//! returned verbatim. These tests pin that contract.

use dry_core::{
    apply_safe_gated, safe_pipeline, Contracts, Feedrate, Length, Segment, SegmentKind, Toolpath,
    Volume,
};

/// A valid extruding line move; override per case.
fn line(start: [f64; 3], end: [f64; 3]) -> Segment {
    Segment {
        start: [
            Some(Length::mm(start[0])),
            Some(Length::mm(start[1])),
            Some(Length::mm(start[2])),
        ],
        end: [
            Some(Length::mm(end[0])),
            Some(Length::mm(end[1])),
            Some(Length::mm(end[2])),
        ],
        travel: false,
        speed: Feedrate(1500.0),
        length: Length::mm(
            (end[0] - start[0])
                .hypot(end[1] - start[1])
                .hypot(end[2] - start[2]),
        ),
        volume: Volume(0.4),
        filament: Length::mm(0.16),
        width: Some(Length::mm(0.4)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: Some(210.0),
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

/// Three line moves whose four endpoints lie exactly on a circle of radius 5 centred at (10, 0), swept
/// counter-clockwise across the top of the circle. Every endpoint stays under y = 4.5, but the fitted
/// arc bulges through its topmost point (10, 5). `arc_fit` therefore folds the run into one arc whose
/// swept extreme leaves a y ≤ 4.5 build volume — the canonical "rewrite introduces a bounds violation".
fn arc_run_over_top() -> Vec<Segment> {
    let (cx, cy, r) = (10.0_f64, 0.0_f64, 5.0_f64);
    let pt = |deg: f64| {
        let a: f64 = deg.to_radians();
        [cx + r * a.cos(), cy + r * a.sin(), 0.2]
    };
    let p = [pt(30.0), pt(60.0), pt(120.0), pt(150.0)];
    vec![line(p[0], p[1]), line(p[1], p[2]), line(p[2], p[3])]
}

/// Bounds that admit every chord endpoint of [`arc_run_over_top`] but not the fitted arc's top point.
fn bounds_below_arc_top() -> Contracts {
    Contracts {
        bounds: Some([[0.0, 200.0], [0.0, 4.5], [0.0, 200.0]]),
        ..Contracts::default()
    }
}

#[test]
fn safe_accepted_when_valid_and_no_contracts() {
    // A collinear extruding run, no contracts active: nothing can become unsafe, so the canonicalised
    // rewrite is always accepted (and is exactly `safe_pipeline`).
    let input = tp(vec![
        line([0.0, 1.0, 0.2], [10.0, 1.0, 0.2]),
        line([10.0, 1.0, 0.2], [20.0, 1.0, 0.2]),
        line([20.0, 1.0, 0.2], [30.0, 1.0, 0.2]),
    ]);
    let result = apply_safe_gated(&input, &Contracts::default());
    assert!(result.accepted, "valid input with no contracts must accept");
    assert!(result.new_error_rules.is_empty());
    // collinear merge collapses the three sub-moves into one.
    assert_eq!(result.toolpath.segments.len(), 1);
    assert_eq!(result.toolpath, safe_pipeline(&input));
}

#[test]
fn safe_rejected_when_arc_fit_breaks_bounds() {
    let input = tp(arc_run_over_top());
    let contracts = bounds_below_arc_top();
    // Sanity: the input itself is in-bounds (all chord endpoints have y ≤ 4.33).
    let baseline = dry_core::verify(&input, &contracts);
    assert!(baseline.ok(), "the chord run must start in-bounds");
    // ...and that the in-bounds claim is non-vacuous: `bounds` is the rule this whole gate turns on,
    // so a contract that silently failed to supply it would make the sanity check meaningless.
    assert!(baseline.evaluated(dry_core::RuleId::Bounds));
    let result = apply_safe_gated(&input, &contracts);
    assert!(
        !result.accepted,
        "an arc fit that leaves the build volume must be rejected"
    );
    // a rejected span is returned verbatim (unchanged segment count, still plain lines).
    assert_eq!(result.toolpath, input);
    assert_eq!(result.toolpath.segments.len(), 3);
}

#[test]
fn new_error_rules_correct_on_rejection() {
    let input = tp(arc_run_over_top());
    let result = apply_safe_gated(&input, &bounds_below_arc_top());
    assert_eq!(result.new_error_rules, vec!["bounds".to_string()]);
}

#[test]
fn pre_existing_error_does_not_block() {
    // A collinear run whose final sub-move ends outside the build volume (x = 80 > 50): the input
    // already has a `bounds` error. `merge_collinear` collapses the run into one move that is *still*
    // out of bounds — the same rule, so no NEW error rule appears and the rewrite is accepted.
    let input = tp(vec![
        line([0.0, 1.0, 0.2], [20.0, 1.0, 0.2]),
        line([20.0, 1.0, 0.2], [40.0, 1.0, 0.2]),
        line([40.0, 1.0, 0.2], [80.0, 1.0, 0.2]),
    ]);
    let contracts = Contracts {
        bounds: Some([[0.0, 50.0], [0.0, 50.0], [0.0, 200.0]]),
        ..Contracts::default()
    };
    assert!(
        !dry_core::verify(&input, &contracts).ok(),
        "the input must already carry a bounds error"
    );
    let result = apply_safe_gated(&input, &contracts);
    assert!(
        result.accepted,
        "a pre-existing error must not block a rewrite that adds no new error rule"
    );
    assert!(result.new_error_rules.is_empty());
    // the rewrite still happened (three collinear sub-moves merged into one).
    assert_eq!(result.toolpath.segments.len(), 1);
    assert_eq!(result.toolpath, safe_pipeline(&input));
}
