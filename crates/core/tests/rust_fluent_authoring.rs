//! Native Rust Authoring SDK Fluent Builder Tests (P4.2).

use dry_core::{simulate, Design, ResolveParams};

#[test]
fn test_fluent_rust_authoring_square_and_arc() {
    let design = Design::new()
        .feedrate(1200.0)
        .geometry(0.5, 0.2)
        .temperature(215.0)
        .fan(0.8)
        .extruder(true)
        .move_to(0.0, 0.0, 0.2)
        .line_to(50.0, 0.0, 0.2)
        .line_to(50.0, 50.0, 0.2)
        .arc_to(25.0, 50.0, 0.0, 50.0, 0.2, true)
        .line_to(0.0, 0.0, 0.2)
        .extruder(false)
        .retract(Some(1.2), Some(1800.0))
        .move_to_z(10.0);

    assert_eq!(design.ops.len(), 13);

    let params = ResolveParams {
        print_speed: 1200.0,
        travel_speed: 6000.0,
        dia: 1.75,
        retraction_speed: Some(1800.0),
        retraction_distance: Some(1.2),
    };

    let toolpath = design.resolve(&params).expect("Fluent design should resolve cleanly");
    assert!(!toolpath.segments.is_empty());

    let metrics = simulate(&toolpath);
    assert!(metrics.total_time_s.value() > 0.0);
    assert!(metrics.extruded_volume.value() > 0.0);
}

#[test]
fn test_fluent_5axis_and_clothoid_authoring() {
    let design = Design::new()
        .feedrate(800.0)
        .orient(0.0, 0.1, 0.99)
        .move_to(10.0, 10.0, 5.0)
        .clothoid_to(50.0, 10.0, 50.0, 50.0, 5.0, 15.0)
        .dwell(2.0);

    assert_eq!(design.ops.len(), 5);

    let params = ResolveParams::default();
    let toolpath = design.resolve(&params).expect("5-axis clothoid design should resolve");
    assert!(toolpath.segments.len() > 10, "Clothoid blend should produce fine segment spans");
}
