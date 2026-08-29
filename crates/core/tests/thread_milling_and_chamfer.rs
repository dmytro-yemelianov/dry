//! Parametric CNC Thread Milling & 3D Chamfering Integration Tests.

use dry_core::{
    generate_chamfer_ops, generate_thread_milling_ops, resolve_checked, simulate, ChamferParams,
    ResolveParams, ThreadMillParams,
};

#[test]
fn test_m10_internal_thread_milling_e2e() {
    let params = ThreadMillParams {
        nominal_diameter: 10.0,
        pitch: 1.5,
        thread_depth: 12.0,
        tool_diameter: 6.0,
        is_internal: true,
        right_hand: true,
        climb: true,
        feedrate: 800.0,
        spindle_rpm: 5000.0,
    };

    let ops = generate_thread_milling_ops(&params, 0.0, 0.0, 0.0).expect("Thread mill generation failed");
    assert!(ops.len() >= 18, "Should have lead-in, multi-turn helical arcs, and lead-out");

    let design = dry_core::Design { ops };
    let resolve_params = ResolveParams {
        print_speed: 800.0,
        travel_speed: 3000.0,
        dia: 1.75,
        retraction_speed: None,
        retraction_distance: None,
    };

    let toolpath = resolve_checked(&design, &resolve_params).expect("Thread mill ops must resolve to L2 IR");
    assert!(!toolpath.segments.is_empty());

    let metrics = simulate(&toolpath);
    assert!(metrics.total_time_s.value() > 0.0, "Machining time must be positive");
    assert!(metrics.extruding_distance.value() > 0.0 || metrics.travel_distance.value() > 0.0);
}

#[test]
fn test_3d_chamfering_contour_e2e() {
    let params = ChamferParams {
        chamfer_width: 1.5,
        chamfer_angle_deg: 45.0,
        tip_diameter: 1.0,
        cutter_diameter: 12.0,
        feedrate: 1500.0,
        spindle_rpm: 6000.0,
    };

    let contour = vec![
        [0.0, 0.0],
        [100.0, 0.0],
        [100.0, 50.0],
        [0.0, 50.0],
        [0.0, 0.0],
    ];

    let ops = generate_chamfer_ops(&params, &contour, 0.0).expect("Chamfer generation failed");
    assert_eq!(ops.len(), 9);

    let design = dry_core::Design { ops };
    let resolve_params = ResolveParams {
        print_speed: 1500.0,
        travel_speed: 3000.0,
        dia: 1.75,
        retraction_speed: None,
        retraction_distance: None,
    };

    let toolpath = resolve_checked(&design, &resolve_params).expect("Chamfer ops must resolve");
    assert_eq!(toolpath.segments.len(), 7);
}

#[test]
fn test_external_thread_milling_and_left_hand() {
    let params_ext = ThreadMillParams {
        nominal_diameter: 20.0,
        pitch: 2.0,
        thread_depth: 10.0,
        tool_diameter: 6.0,
        is_internal: false,
        right_hand: false, // Left-hand
        climb: false,      // Conventional
        feedrate: 600.0,
        spindle_rpm: 4000.0,
    };

    let ops = generate_thread_milling_ops(&params_ext, 50.0, 50.0, 0.0).expect("External thread mill failed");
    assert!(!ops.is_empty());

    let design = dry_core::Design { ops };
    let toolpath = resolve_checked(&design, &ResolveParams::default()).expect("External thread resolves");
    assert!(!toolpath.segments.is_empty());
}
