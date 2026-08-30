use dry_core::generate::{
    generate_lathe_facing_ops, generate_lathe_od_turning_ops, LatheFacingParams, LatheTurningParams,
};
use dry_core::resolve::{resolve, Design, ResolveParams};

#[test]
fn test_lathe_facing_operations() {
    let params = LatheFacingParams {
        stock_diameter: 50.0,
        target_z: 0.0,
        clearance_x: 2.0,
        clearance_z: 2.0,
        feedrate: 300.0,
        spindle_rpm: 1500.0,
        passes: 2,
        depth_per_pass: 1.0,
    };

    let ops = generate_lathe_facing_ops(&params).expect("Failed to generate facing ops");
    assert!(!ops.is_empty());

    let design = Design { ops };
    let toolpath = resolve(&design, &ResolveParams::default());
    assert!(!toolpath.segments.is_empty());

    // Verify facing passes cross through center
    let has_center_pass = toolpath
        .segments
        .iter()
        .any(|seg| seg.end[0].map(|x| x.value() <= 0.0).unwrap_or(false));
    assert!(
        has_center_pass,
        "Facing operation must cut across the center axis"
    );
}

#[test]
fn test_lathe_od_roughing_and_finishing() {
    let params = LatheTurningParams {
        raw_diameter: 60.0,
        target_diameter: 40.0,
        cut_length: 50.0,
        depth_of_cut: 2.5,
        finish_allowance: 0.5,
        clearance_x: 2.0,
        clearance_z: 2.0,
        rough_feedrate: 350.0,
        finish_feedrate: 180.0,
        spindle_rpm: 1400.0,
    };

    let ops = generate_lathe_od_turning_ops(&params).expect("Failed to generate OD turning ops");
    assert!(!ops.is_empty());

    let design = Design { ops };
    let toolpath = resolve(&design, &ResolveParams::default());
    assert!(!toolpath.segments.is_empty());

    // Verify deepest cutting reach
    let has_target_reach = toolpath
        .segments
        .iter()
        .any(|seg| seg.end[2].map(|z| z.value() <= -50.0).unwrap_or(false));
    assert!(
        has_target_reach,
        "OD turning must reach the commanded cut length"
    );

    // Verify finished radius
    let has_final_radius = toolpath.segments.iter().any(|seg| {
        seg.end[0]
            .map(|x| (x.value() - 20.0).abs() < 1e-4)
            .unwrap_or(false)
    });
    assert!(
        has_final_radius,
        "Finishing pass must cut at target radius 20mm (diameter 40mm)"
    );
}

#[test]
fn test_lathe_invalid_parameter_rejection() {
    let bad_facing = LatheFacingParams {
        stock_diameter: -10.0,
        ..Default::default()
    };
    assert!(generate_lathe_facing_ops(&bad_facing).is_err());

    let bad_od = LatheTurningParams {
        raw_diameter: 30.0,
        target_diameter: 40.0, // Target larger than raw!
        ..Default::default()
    };
    assert!(generate_lathe_od_turning_ops(&bad_od).is_err());
}
