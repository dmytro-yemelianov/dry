use dry_core::{
    check_compatibility, resolve, AxisRange, Design, MachineCapabilities, Op, ResolveParams,
};

#[test]
fn test_multi_category_machine_capability_checks() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 15000.0 }); // 15000 mm/min
    design.ops.push(Op::Move {
        x: Some(250.0),
        y: Some(250.0),
        z: Some(10.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    // 1. 3D Printer (Bambu X1C volume 256x256x256, max feedrate 30000 mm/min)
    let bambu_caps = MachineCapabilities {
        name: "Bambu Lab X1 Carbon".into(),
        x_range: AxisRange::new(0.0, 256.0),
        y_range: AxisRange::new(0.0, 256.0),
        z_range: AxisRange::new(0.0, 256.0),
        max_feedrate_mm_min: Some(30000.0),
        max_spindle_rpm: None,
    };
    let r1 = check_compatibility(&toolpath, &bambu_caps);
    assert!(r1.compatible, "Bambu should accommodate design");

    // 2. Small CNC Router (volume 150x150x50, max feedrate 5000 mm/min)
    let small_cnc_caps = MachineCapabilities {
        name: "Desktop CNC".into(),
        x_range: AxisRange::new(0.0, 150.0), // Too small for X=250
        y_range: AxisRange::new(0.0, 150.0),
        z_range: AxisRange::new(0.0, 50.0),
        max_feedrate_mm_min: Some(5000.0), // Too slow for 15000 mm/min
        max_spindle_rpm: Some(24000.0),
    };
    let r2 = check_compatibility(&toolpath, &small_cnc_caps);
    assert!(!r2.compatible, "Desktop CNC should fail on bounds and feedrate");
    assert!(r2.findings.iter().any(|f| f.code == "OUT_OF_BOUNDS_X"));
    assert!(r2.findings.iter().any(|f| f.code == "EXCEEDS_MAX_FEEDRATE"));
}
