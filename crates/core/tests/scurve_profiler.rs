use dry_core::{calculate_scurve_profile, SCurveParams};

#[test]
fn test_scurve_trapezoidal_profile() {
    // Large delta V: v_start = 0, v_target = 200 mm/s, a_max = 2000 mm/s^2, j_max = 20000 mm/s^3
    let params = SCurveParams {
        v_start: 0.0,
        v_target: 200.0,
        max_acceleration: 2000.0,
        max_jerk: 20000.0,
    };

    let profile = calculate_scurve_profile(&params).expect("profile calculation succeeds");
    assert_eq!(profile.peak_acceleration, 2000.0);
    // t_j = 2000 / 20000 = 0.1s
    assert!((profile.t_jerk_inc - 0.1).abs() < 1e-6);
    assert!((profile.t_jerk_dec - 0.1).abs() < 1e-6);
    // delta_v_j = 2000^2 / 20000 = 200 mm/s -> t_a = 0
    assert!(profile.t_const_acc >= 0.0);
    assert!(profile.total_duration > 0.0);
    assert!(profile.total_distance > 0.0);
}

#[test]
fn test_scurve_triangular_profile() {
    // Small delta V: v_start = 100, v_target = 110 mm/s, a_max = 5000 mm/s^2, j_max = 50000 mm/s^3
    let params = SCurveParams {
        v_start: 100.0,
        v_target: 110.0,
        max_acceleration: 5000.0,
        max_jerk: 50000.0,
    };

    let profile = calculate_scurve_profile(&params).expect("profile calculation succeeds");
    // a_peak = sqrt(10 * 50000) = sqrt(500000) ~ 707.1 mm/s^2 < 5000
    assert!(profile.peak_acceleration < 5000.0);
    assert_eq!(profile.t_const_acc, 0.0);
    assert!(profile.total_duration > 0.0);
}
