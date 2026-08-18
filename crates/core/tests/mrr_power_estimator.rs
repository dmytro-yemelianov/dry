use dry_core::{calculate_mrr, estimate_cutting_power_kw, evaluate_mrr};

#[test]
fn test_mrr_and_cutting_power_physics() {
    // Milling 6061 Aluminum: ap = 3.0mm, ae = 6.0mm (full slot 6mm endmill), vf = 1200 mm/min
    // Specific cutting force kc for Al 6061 ~ 700 N/mm^2, efficiency = 0.85
    let ap = 3.0;
    let ae = 6.0;
    let vf = 1200.0;
    let kc_aluminum = 700.0;
    let eta = 0.85;

    // MRR = (3.0 * 6.0 * 1200) / 1000 = 21.6 cm^3/min
    let mrr = calculate_mrr(ap, ae, vf);
    assert!((mrr - 21.6).abs() < 1e-4);

    // Power = (21.6 * 700) / (60 * 1000 * 0.85) = 15120 / 51000 ~ 0.2965 kW (296.5 W)
    let power = estimate_cutting_power_kw(mrr, kc_aluminum, eta);
    assert!((power - 0.29647).abs() < 1e-3);

    let report = evaluate_mrr(ap, ae, vf, kc_aluminum, eta);
    assert_eq!(report.depth_of_cut_mm, 3.0);
    assert_eq!(report.width_of_cut_mm, 6.0);
    assert_eq!(report.feedrate_mm_min, 1200.0);
    assert!((report.mrr_cm3_min - 21.6).abs() < 1e-4);
    assert!((report.cutting_power_kw - power).abs() < 1e-6);
}

#[test]
fn test_zero_or_negative_inputs_return_zero_power() {
    assert_eq!(calculate_mrr(0.0, 5.0, 1000.0), 0.0);
    assert_eq!(estimate_cutting_power_kw(0.0, 700.0, 0.85), 0.0);
}
