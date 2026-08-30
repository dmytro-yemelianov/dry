//! An optimize pass given a parameter it cannot honour must leave the toolpath untouched.
//!
//! These passes take `&mut Toolpath` and return `()`, so they have no channel to refuse into. Each
//! of them turned a nonsensical parameter into a confident answer instead of declining — and because
//! the results were finite and plausible, nothing downstream caught them.

use dry_core::optimize::{
    apply_chip_thinning_compensation, optimize_constant_mrr, optimize_corner_feedrate,
};
use dry_core::{resolve, Design, Op, ResolveParams, Toolpath};

fn square() -> Toolpath {
    let mut d = Design::default();
    d.ops.push(Op::Geometry {
        width: Some(6.0),
        height: Some(1.0),
    });
    d.ops.push(Op::Speed { print: 800.0 });
    d.ops.push(Op::Extruder { on: true });
    for (x, y) in [
        (0.0, 0.0),
        (40.0, 0.0),
        (40.0, 40.0),
        (0.0, 40.0),
        (0.0, 0.0),
    ] {
        d.ops.push(Op::Move {
            x: Some(x),
            y: Some(y),
            z: Some(-1.0),
        });
    }
    resolve(&d, &ResolveParams::default())
}

fn speeds(t: &Toolpath) -> Vec<f64> {
    t.segments.iter().map(|s| s.speed.value()).collect()
}

/// `min_feed_ratio.clamp(0.1, 1.0)` looks like sanitisation but is not: `f64::clamp` returns `NaN`
/// for a `NaN` *self*. The `NaN` then reached a later `clamp(min_ratio, 1.0)` as a **bound**, which
/// is the case `f64::clamp` panics on — an abort, not an error, on the wasm binding.
#[test]
fn corner_feedrate_declines_a_non_finite_ratio_instead_of_panicking() {
    let before = speeds(&square());
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut t = square();
        optimize_corner_feedrate(&mut t, bad);
        assert_eq!(
            speeds(&t),
            before,
            "ratio {bad} must leave the toolpath alone"
        );
    }
    // A finite ratio still optimises: corners slow down.
    let mut t = square();
    optimize_corner_feedrate(&mut t, 0.5);
    assert_ne!(speeds(&t), before, "a valid ratio must still do something");
}

/// A stepover of `0` or `-0.5` clamped to `0.001` and commanded **350% feedrate** — the cap the
/// source itself calls "machine safety". A guardrail that converts nonsense into the most aggressive
/// legal answer is not a guardrail, and no verify rule relates feedrate to stepover.
#[test]
fn chip_thinning_declines_a_stepover_that_describes_no_cut() {
    let before = speeds(&square());
    for bad in [0.0, -0.5, 5.0, f64::NAN, f64::INFINITY] {
        let mut t = square();
        apply_chip_thinning_compensation(&mut t, bad);
        assert_eq!(
            speeds(&t),
            before,
            "stepover {bad} must not scale the feedrate"
        );
    }
    // A real thin-chip stepover still compensates, and stays under the 3.5x cap.
    let mut t = square();
    apply_chip_thinning_compensation(&mut t, 0.1);
    let after = speeds(&t);
    assert!(after[1] > before[1], "10% stepover must raise the feedrate");
    assert!(after[1] <= before[1] * 3.5 + 1e-9, "and stay under the cap");
}

/// Its guard is `depth_of_cut <= 0.0`, and `NaN <= 0.0` is false, so a `NaN` depth wrote `NaN` into
/// every speed. The feedrate bounds were unvalidated entirely, so a negative pair forced every speed
/// **negative** — reintroducing exactly what H1.2 refuses at ingress.
#[test]
fn constant_mrr_declines_parameters_it_cannot_honour() {
    let before = speeds(&square());
    let cases: [(f64, f64, f64, f64); 6] = [
        (f64::NAN, 100.0, 10.0, 1000.0),
        (1.0, f64::NAN, 10.0, 1000.0),
        (1.0, 100.0, f64::NAN, 1000.0),
        (1.0, 100.0, -500.0, -100.0), // negative bounds
        (1.0, 100.0, 1000.0, 10.0),   // min above max
        (0.0, 0.0, 0.0, 0.0),
    ];
    for (depth, mrr, lo, hi) in cases {
        let mut t = square();
        optimize_constant_mrr(&mut t, depth, mrr, lo, hi);
        let after = speeds(&t);
        assert_eq!(
            after, before,
            "({depth}, {mrr}, {lo}, {hi}) must leave the toolpath alone"
        );
        assert!(
            after.iter().all(|v| v.is_finite() && *v > 0.0),
            "no pass may produce a non-finite or non-positive feedrate"
        );
    }
    // Valid parameters still rewrite the feedrates.
    let mut t = square();
    optimize_constant_mrr(&mut t, 1.0, 3000.0, 100.0, 2000.0);
    assert_ne!(speeds(&t), before, "valid parameters must still optimise");
}
