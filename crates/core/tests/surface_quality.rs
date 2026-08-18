use dry_core::{
    calculate_cusp_height, estimate_surface_roughness_ra, evaluate_surface_quality,
};

#[test]
fn test_cusp_height_and_surface_roughness() {
    // 6mm ball endmill (radius = 3.0mm), 0.5mm stepover
    let r = 3.0;
    let s = 0.5;

    let cusp = calculate_cusp_height(r, s).expect("cusp height calculation succeeds");
    // h = 3 - sqrt(9 - 0.25^2) = 3 - sqrt(9 - 0.0625) = 3 - sqrt(8.9375) ~ 3 - 2.989565 ~ 0.010435 mm (10.4 um)
    assert!((cusp - 0.010435).abs() < 1e-4);

    let ra = estimate_surface_roughness_ra(cusp);
    // Ra ~ 10.435 / 4 = 2.608 um
    assert!((ra - 2.608).abs() < 1e-2);

    let report = evaluate_surface_quality(r, s).expect("report generates");
    assert_eq!(report.tool_radius_mm, 3.0);
    assert_eq!(report.stepover_mm, 0.5);
    assert!((report.cusp_height_mm - cusp).abs() < 1e-6);
}

#[test]
fn test_invalid_stepover_exceeding_diameter_fails() {
    // Stepover of 8mm with radius 3mm (diameter 6mm) leaves uncut stock
    assert!(calculate_cusp_height(3.0, 8.0).is_err());
}
