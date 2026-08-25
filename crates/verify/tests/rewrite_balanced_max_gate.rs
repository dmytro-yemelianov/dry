//! The `balanced` and `max` optimisation-mode gates (`docs/11-profiles-and-reports.md`).
//!
//! `balanced` runs the geometry-canonicalisation subset plus conservative `adaptive_speed` shaping;
//! `max` runs the full order-changing pipeline (merge → arc → adaptive-speed → coasting → travel-reorder
//! → z-hop). Both reuse the *same* accept-unless-new-error gate as `safe` ([`apply_gated`]): a rewrite is
//! kept only when it introduces no new error rule relative to the input under the active contracts. These
//! tests pin the balanced/max behaviour and the gate-rejection fallbacks.

use kmet_contracts::{Contracts, RuleId};
use kmet_kernel::{
    balanced_pipeline, max_pipeline, Feedrate, Length, OptimizeMode, Segment, SegmentKind,
    Toolpath, Volume,
};
use kmet_verify::{apply_gated, verify};

/// A valid extruding line move at `speed` mm/min; override per case.
fn line_at(start: [f64; 3], end: [f64; 3], speed: f64) -> Segment {
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
        speed: Feedrate(speed),
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

/// A straight travel move of the given length along +X at `z` (used to trigger z-hop / reordering).
fn travel(start: [f64; 3], end: [f64; 3]) -> Segment {
    Segment {
        travel: true,
        volume: Volume::ZERO,
        filament: Length::ZERO,
        ..line_at(start, end, 9000.0)
    }
}

fn tp(segments: Vec<Segment>) -> Toolpath {
    Toolpath {
        version: 0,
        meta: None,
        segments,
    }
}

// --- balanced -------------------------------------------------------------------------------------

#[test]
fn balanced_accepts_clean_rewrite() {
    // A collinear extruding run with no contracts: `safe_pipeline` merges it to one move and
    // `adaptive_speed` cannot shape an isolated single move, so the rewrite is always accepted.
    let input = tp(vec![
        line_at([0.0, 1.0, 0.2], [10.0, 1.0, 0.2], 1500.0),
        line_at([10.0, 1.0, 0.2], [20.0, 1.0, 0.2], 1500.0),
        line_at([20.0, 1.0, 0.2], [30.0, 1.0, 0.2], 1500.0),
    ]);
    let result = apply_gated(&input, &Contracts::default(), OptimizeMode::Balanced, None);
    assert!(
        result.accepted,
        "clean collinear run must accept under balanced"
    );
    assert!(result.new_error_rules.is_empty());
    assert_eq!(
        result.toolpath.segments.len(),
        1,
        "collinear run merges to one move"
    );
    assert_eq!(result.toolpath, balanced_pipeline(&input, None));
}

#[test]
fn balanced_shapes_speed_within_range() {
    // A sharp 90° corner: `adaptive_speed` scales both legs' feedrate by the junction factor
    // (cos-of-half-angle ≈ 0.707). With a generous range the shaped speed stays in-bounds → accepted,
    // and the rewrite genuinely changed the feedrates (proving the adaptive-speed pass ran).
    let input = tp(vec![
        line_at([0.0, 0.0, 0.2], [10.0, 0.0, 0.2], 1500.0),
        line_at([10.0, 0.0, 0.2], [10.0, 10.0, 0.2], 1500.0),
    ]);
    let contracts = Contracts {
        speed_range: Some([600.0, 6000.0]),
        ..Contracts::default()
    };
    let result = apply_gated(&input, &contracts, OptimizeMode::Balanced, None);
    assert!(result.accepted, "shaped speed inside the range must accept");
    assert!(result.new_error_rules.is_empty());
    // adaptive_speed dropped the corner feedrate below the authored 1500 mm/min.
    let shaped = result.toolpath.segments[0].speed.value();
    assert!(
        shaped < 1500.0,
        "balanced should shape the corner speed below the authored feedrate (got {shaped})"
    );
}

#[test]
fn balanced_rejected_when_adaptive_speed_drops_below_min() {
    // Same 90° corner, but a feedrate floor just above the shaped speed: the authored 1500 mm/min is
    // in-range, but `adaptive_speed` scales the corner to ~1060 mm/min < 1200 → a NEW `speed` error →
    // the whole span is rejected and falls back to the input verbatim.
    let input = tp(vec![
        line_at([0.0, 0.0, 0.2], [10.0, 0.0, 0.2], 1500.0),
        line_at([10.0, 0.0, 0.2], [10.0, 10.0, 0.2], 1500.0),
    ]);
    let contracts = Contracts {
        speed_range: Some([1200.0, 6000.0]),
        ..Contracts::default()
    };
    // Sanity: the authored input is in-range (no pre-existing speed error).
    let baseline = verify(&input, &contracts);
    assert!(
        baseline.ok(),
        "authored 1500 mm/min must start in the [1200, 6000] range"
    );
    // `speed` is the rule this gate turns on; pin that the contract actually put it in force.
    assert!(baseline.evaluated(RuleId::Speed));
    let result = apply_gated(&input, &contracts, OptimizeMode::Balanced, None);
    assert!(
        !result.accepted,
        "a shaped feedrate that drops below the range minimum must be rejected"
    );
    assert!(
        result.new_error_rules.contains(&"speed".to_string()),
        "rejection must name the introduced `speed` rule (got {:?})",
        result.new_error_rules
    );
    // a rejected span is returned verbatim (unchanged, still the authored feedrate).
    assert_eq!(result.toolpath, input);
}

// --- max ------------------------------------------------------------------------------------------

#[test]
fn max_rejected_under_monotonic_z() {
    // Two extruding runs separated by a long travel, all at a constant Z = 0.2 (so the input honours a
    // `monotonic_z` contract). `max` runs z-hop, which lifts the travel and then *lowers* back down — a
    // Z-decreasing move that violates `monotonic-z`. The new error rejects the span; the input is kept.
    let input = tp(vec![
        line_at([0.0, 0.0, 0.2], [10.0, 0.0, 0.2], 1500.0),
        travel([10.0, 0.0, 0.2], [40.0, 0.0, 0.2]),
        line_at([40.0, 0.0, 0.2], [50.0, 0.0, 0.2], 1500.0),
    ]);
    let contracts = Contracts {
        monotonic_z: true,
        ..Contracts::default()
    };
    // Sanity: the authored input never decreases Z.
    let baseline = verify(&input, &contracts);
    assert!(baseline.evaluated(RuleId::MonotonicZ));
    assert!(
        baseline.ok(),
        "the constant-Z authored input must satisfy monotonic-z"
    );
    let result = apply_gated(&input, &contracts, OptimizeMode::Max, None);
    assert!(
        !result.accepted,
        "z-hop's lowering move must be rejected under monotonic-z"
    );
    assert!(
        result.new_error_rules.contains(&"monotonic-z".to_string()),
        "rejection must name the introduced `monotonic-z` rule (got {:?})",
        result.new_error_rules
    );
    assert_eq!(
        result.toolpath, input,
        "a rejected span falls back to the input"
    );
}

#[test]
fn max_accepted_and_reduces_segments_under_permissive_contracts() {
    // Two long collinear extruding runs (5 sub-moves each) joined by a travel, no contracts. `max`
    // merges each run to a single move (then coasting trims a tiny tail) and z-hops the travel; the net
    // segment count still falls well below the input, and nothing introduces a new error → accepted.
    let mut segs = Vec::new();
    for k in 0..5 {
        let x0 = 10.0 * k as f64;
        segs.push(line_at([x0, 0.0, 0.2], [x0 + 10.0, 0.0, 0.2], 1500.0));
    }
    segs.push(travel([50.0, 0.0, 0.2], [50.0, 50.0, 0.2]));
    for k in 0..5 {
        let y0 = 50.0 - 10.0 * k as f64;
        segs.push(line_at([50.0, y0, 0.2], [50.0, y0 - 10.0, 0.2], 1500.0));
    }
    let input = tp(segs);
    let before = input.segments.len();
    let result = apply_gated(&input, &Contracts::default(), OptimizeMode::Max, None);
    assert!(
        result.accepted,
        "max under permissive contracts must accept (no new error): {:?}",
        result.new_error_rules
    );
    assert!(
        result.toolpath.segments.len() < before,
        "max should reduce the segment count ({} → {})",
        before,
        result.toolpath.segments.len()
    );
    assert_eq!(result.toolpath, max_pipeline(&input));
}
