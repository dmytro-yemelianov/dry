//! Digital Twin Physics & Thermal Simulator Tests.

use dry_core::physics::{
    analyze_machining_physics, CuttingToolGeometry, MachiningOperationParams, WorkpieceMaterial,
};

#[test]
fn test_aluminum_milling_physics_deflection_and_forces() {
    let tool = CuttingToolGeometry {
        diameter_mm: 12.0,
        flute_count: 3,
        stickout_length_mm: 35.0,
        core_diameter_ratio: 0.75,
        modulus_gpa: 600.0, // Carbide
        corner_radius_mm: 0.5,
    };

    let params = MachiningOperationParams {
        spindle_rpm: 10000.0,
        feedrate_mm_min: 3000.0,
        axial_depth_ap_mm: 5.0,
        radial_depth_ae_mm: 6.0,
        ambient_temp_c: 20.0,
    };

    let report = analyze_machining_physics(&tool, WorkpieceMaterial::Aluminum6061, &params);

    // Vc = pi * 12 * 10000 / 1000 = 376.99 m/min
    assert!((report.cutting_speed_m_min - 376.99).abs() < 1.0);
    // fz = 3000 / (10000 * 3) = 0.1 mm/tooth
    assert!((report.feed_per_tooth_mm - 0.1).abs() < 1e-5);
    // MRR = 5 * 6 * 3000 / 1000 = 90 cm^3/min
    assert_eq!(report.material_removal_rate_cm3_min, 90.0);

    // Positive cutting forces and power
    assert!(report.tangential_force_n > 50.0);
    assert!(report.spindle_power_kw > 0.5);
    assert!(report.spindle_torque_nm > 0.2);

    // Deflection should be modest (< 15 um) and no chatter for 35mm stickout on 12mm endmill
    assert!(report.tool_deflection_um > 0.1);
    assert!(report.tool_deflection_um < 20.0);
    assert!(!report.chatter_risk);
}

#[test]
fn test_titanium_deep_slot_chatter_and_thermal_alert() {
    // Long overhang tool (60mm stickout on 8mm endmill -> L/D = 7.5)
    let tool = CuttingToolGeometry {
        diameter_mm: 8.0,
        flute_count: 4,
        stickout_length_mm: 60.0,
        core_diameter_ratio: 0.70,
        modulus_gpa: 600.0,
        corner_radius_mm: 0.2,
    };

    let params = MachiningOperationParams {
        spindle_rpm: 2500.0,
        feedrate_mm_min: 500.0,
        axial_depth_ap_mm: 8.0,
        radial_depth_ae_mm: 8.0, // Full slotting
        ambient_temp_c: 22.0,
    };

    let report = analyze_machining_physics(&tool, WorkpieceMaterial::TitaniumTi6Al4V, &params);

    // High force on titanium (kc = 2800 N/mm2)
    assert!(report.tangential_force_n > 500.0);
    // High tool deflection and chatter risk flagged due to L/D > 4
    assert!(report.chatter_risk);
    assert!(report.tool_deflection_um > 15.0);

    // Higher shear temperature on Titanium
    assert!(report.shear_temperature_c > 100.0);
}

/// A clamped result must say so. Both bounds are guardrails against absurd inputs, but a clamped
/// value is indistinguishable from a computed one in the report, and both fields are read as process
/// predictions.
#[test]
fn a_clamped_result_reports_itself_as_saturated() {
    use dry_core::{
        analyze_machining_physics, CuttingToolGeometry, MachiningOperationParams, WorkpieceMaterial,
    };

    let tool = CuttingToolGeometry {
        diameter_mm: 10.0,
        flute_count: 4,
        stickout_length_mm: 40.0,
        core_diameter_ratio: 0.6,
        modulus_gpa: 600.0,
        corner_radius_mm: 0.5,
    };

    // Titanium at ~251 m/min is roughly five times any sane cutting speed for it, and saturates
    // both models at once.
    let absurd = MachiningOperationParams {
        spindle_rpm: 8000.0,
        feedrate_mm_min: 1200.0,
        axial_depth_ap_mm: 5.0,
        radial_depth_ae_mm: 2.0,
        ambient_temp_c: 20.0,
    };
    let r = analyze_machining_physics(&tool, WorkpieceMaterial::TitaniumTi6Al4V, &absurd);
    assert!(
        r.model_saturated,
        "a result pinned to both clamps must not present itself as a prediction"
    );
    assert_eq!(r.estimated_tool_life_min, 0.1, "the floor, not an estimate");
    assert_eq!(
        r.shear_temperature_c, 1220.0,
        "ambient + the 1200 C ceiling"
    );

    // An ordinary aluminium cut is inside the model, and must not be flagged.
    let sane = MachiningOperationParams {
        spindle_rpm: 6000.0,
        feedrate_mm_min: 900.0,
        axial_depth_ap_mm: 2.0,
        radial_depth_ae_mm: 3.0,
        ambient_temp_c: 20.0,
    };
    let r = analyze_machining_physics(&tool, WorkpieceMaterial::Aluminum6061, &sane);
    assert!(
        !r.model_saturated,
        "an in-range cut must not be flagged: temp={} life={}",
        r.shear_temperature_c, r.estimated_tool_life_min
    );
}
