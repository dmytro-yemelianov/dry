use dry_core::{design, DesignBuilder, FirmwareFlavor};

#[test]
fn test_rust_sdk_fluent_authoring_parity() {
    // 1. Fluent Builder
    let d1 = DesignBuilder::new()
        .geometry(0.5, 0.2)
        .speed(1500.0)
        .extruder(true)
        .point(0.0, 0.0, 0.2)
        .point(50.0, 0.0, 0.2)
        .point(50.0, 50.0, 0.2)
        .point(0.0, 50.0, 0.2)
        .point(0.0, 0.0, 0.2)
        .build();

    // 2. Declarative Macro
    let d2 = design! {
        geometry(0.5, 0.2);
        speed(1500.0);
        extruder(true);
        point(0.0, 0.0, 0.2);
        point(50.0, 0.0, 0.2);
        point(50.0, 50.0, 0.2);
        point(0.0, 50.0, 0.2);
        point(0.0, 0.0, 0.2);
    };

    assert_eq!(d1.ops, d2.ops);

    let tp1 = d1.ir().unwrap();
    let tp2 = d2.ir().unwrap();
    assert_eq!(tp1, tp2);

    let gcode = d1.gcode(FirmwareFlavor::Marlin).unwrap();
    assert!(!gcode.is_empty());
    assert!(gcode.iter().any(|line| line.contains("X50")));
}

#[test]
fn test_rust_sdk_channels_and_arcs() {
    let d = design! {
        geometry(0.4, 0.2);
        speed(1800.0);
        temperature(210.0);
        fan(0.75);
        flow(1.05);
        extruder(true);
        point(10.0, 10.0, 0.2);
        arc(20.0, 10.0, 30.0, 10.0, 0.2, false);
        dwell(1.0);
    };

    assert_eq!(d.ops.len(), 9);
    let tp = d.ir().unwrap();
    assert!(tp.segments.len() >= 2);
}
