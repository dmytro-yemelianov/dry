use dry_core::{
    check_compatibility, resolve, AxisRange, Design, MachineCapabilities, Op, ResolveParams,
};

#[test]
fn test_compatibility_engine_detects_out_of_bounds() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.0),
    });
    design.ops.push(Op::Move {
        x: Some(250.0), // exceeds max X of 200 mm
        y: Some(50.0),
        z: Some(10.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    let caps = MachineCapabilities::new(
        "Standard-Mill-3Axis",
        AxisRange::new(0.0, 200.0),
        AxisRange::new(0.0, 200.0),
        AxisRange::new(0.0, 150.0),
    );

    let report = check_compatibility(&toolpath, &caps);
    assert!(!report.compatible);
    assert!(report.findings.iter().any(|f| f.code == "OUT_OF_BOUNDS_X"));
}

#[test]
fn test_compatibility_engine_passes_valid_toolpath() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(10.0),
        z: Some(0.0),
    });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(10.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    let caps = MachineCapabilities::new(
        "Standard-Mill-3Axis",
        AxisRange::new(0.0, 200.0),
        AxisRange::new(0.0, 200.0),
        AxisRange::new(0.0, 150.0),
    );

    let report = check_compatibility(&toolpath, &caps);
    assert!(report.compatible);
    assert!(report.findings.is_empty());
}
