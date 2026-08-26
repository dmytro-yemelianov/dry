//! Input-domain guards on the D4 analytics and motion-planning entry points.
//!
//! Each of these accepted a non-finite input and returned a non-finite result rather than refusing
//! it. `<= 0.0` and `< 0.0` are both false for NaN, so a magnitude guard alone lets NaN through.

use dry_core::{
    calculate_mrr, calculate_scurve_profile, estimate_cutting_power_kw, resolve_checked, Design,
    Op, ResolveParams, SCurveParams,
};

#[test]
fn scurve_rejects_non_finite_velocities() {
    let base = SCurveParams {
        v_start: 0.0,
        v_target: 100.0,
        max_acceleration: 1000.0,
        max_jerk: 10000.0,
    };

    for (v_start, v_target) in [
        (f64::NAN, 100.0),
        (0.0, f64::NAN),
        (f64::INFINITY, 100.0),
        (0.0, f64::INFINITY),
    ] {
        let params = SCurveParams {
            v_start,
            v_target,
            ..base.clone()
        };
        assert!(
            calculate_scurve_profile(&params).is_err(),
            "a non-finite velocity must be refused, not planned: {v_start} -> {v_target}"
        );
    }

    // The valid case still plans, and its outputs are finite.
    let profile = calculate_scurve_profile(&base).expect("a finite request must plan");
    assert!(profile.total_duration.is_finite() && profile.total_duration > 0.0);
    assert!(profile.total_distance.is_finite() && profile.total_distance > 0.0);
    assert!(profile.peak_acceleration.is_finite());
}

#[test]
fn mrr_returns_zero_for_non_finite_dimensions() {
    assert_eq!(calculate_mrr(f64::NAN, 2.0, 500.0), 0.0);
    assert_eq!(calculate_mrr(3.0, f64::NAN, 500.0), 0.0);
    assert_eq!(calculate_mrr(3.0, 2.0, f64::INFINITY), 0.0);
    // A valid request is unchanged: 3 x 2 x 500 mm^3/min = 3 cm^3/min.
    assert!((calculate_mrr(3.0, 2.0, 500.0) - 3.0).abs() < 1e-12);
}

/// Power scales as 1/eta, so clamping a too-small efficiency *upwards* lowers the estimate — an
/// under-estimate of what the spindle must supply. Only the impossible upper end is clamped.
#[test]
fn cutting_power_does_not_round_efficiency_in_the_unsafe_direction() {
    let mrr = 3.0;
    let kc = 700.0;

    let at_half = estimate_cutting_power_kw(mrr, kc, 0.5);
    let at_twentieth = estimate_cutting_power_kw(mrr, kc, 0.05);
    assert!(
        at_twentieth > at_half,
        "a less efficient spindle must be reported as needing more power, not less"
    );
    // 3 * 700 / (60_000 * 0.05) = 0.7 kW
    assert!((at_twentieth - 0.7).abs() < 1e-12, "got {at_twentieth}");

    // Efficiency above 1 is physically impossible; clamping it to 1 errs high, which is safe.
    let at_one = estimate_cutting_power_kw(mrr, kc, 1.0);
    assert_eq!(estimate_cutting_power_kw(mrr, kc, 4.0), at_one);

    // Out-of-domain inputs return the documented sentinel rather than a non-finite number.
    assert_eq!(estimate_cutting_power_kw(mrr, kc, 0.0), 0.0);
    assert_eq!(estimate_cutting_power_kw(mrr, kc, f64::NAN), 0.0);
    assert_eq!(estimate_cutting_power_kw(f64::NAN, kc, 0.85), 0.0);
}

/// An orientation vector whose components are each finite but whose squares overflow was accepted:
/// `libm::sqrt(i*i + j*j + k*k)` is infinity, and `mag <= 0.0` is false for infinity. Every consumer
/// that normalises then divides by that magnitude and gets exactly (0, 0, 0) — the degenerate
/// orientation the check exists to refuse.
#[test]
fn orientation_with_an_unnormalizable_magnitude_is_rejected() {
    let build = |i: f64, j: f64, k: f64| {
        let mut design = Design::default();
        design.ops.push(Op::Move {
            x: Some(0.0),
            y: Some(0.0),
            z: Some(0.0),
        });
        design.ops.push(Op::Orient { i, j, k });
        design.ops.push(Op::Move {
            x: Some(10.0),
            y: Some(0.0),
            z: Some(0.0),
        });
        design
    };

    for (i, j, k) in [(1e200, 1e200, 1e200), (f64::MAX, 0.0, 0.0), (0.0, 0.0, 0.0)] {
        let error = resolve_checked(&build(i, j, k), &ResolveParams::default())
            .expect_err("an unnormalizable orientation must be refused");
        assert!(
            format!("{error}").contains("non-zero magnitude"),
            "expected a magnitude error for ({i:e}, {j:e}, {k:e}), got {error}"
        );
    }

    // Ordinary directions, unit or not, still resolve and still normalise to a real direction.
    for (i, j, k) in [(0.0, 0.0, 1.0), (1.0, 1.0, 1.0), (1e-8, 0.0, 1e-8)] {
        let toolpath = resolve_checked(&build(i, j, k), &ResolveParams::default())
            .expect("a normalizable orientation must resolve");
        let o = toolpath
            .segments
            .iter()
            .find_map(|s| s.orientation)
            .expect("the orientation rides the segment");
        let mag = libm::sqrt(o[0] * o[0] + o[1] * o[1] + o[2] * o[2]);
        assert!(
            mag.is_finite() && mag > 0.0,
            "magnitude {mag} is not normalizable"
        );
        assert!(
            (o[0] / mag).is_finite() && (o[1] / mag).is_finite() && (o[2] / mag).is_finite(),
            "normalising ({i:e}, {j:e}, {k:e}) did not yield a finite direction"
        );
    }
}
