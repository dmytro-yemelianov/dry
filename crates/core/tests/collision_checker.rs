use dry_core::{check_tool_holder_collision, resolve, Design, Op, ResolveParams, ToolHolder};

#[test]
fn test_tool_holder_deep_plunge_collision() {
    let mut design = Design::default();
    // Rapid move over stock
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(10.0),
    });
    // Plunge deep into stock to Z = -35.0 (depth = 35mm from stock top Z = 0)
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 1200.0 });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(-35.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    let stock_bounds = [0.0, 100.0, 0.0, 100.0, -50.0, 0.0];

    // Holder with 20mm stickout (cannot reach 35mm depth without collet collision)
    let holder_short = ToolHolder {
        holder_diameter: 50.0,
        stickout_length: 20.0,
        collet_diameter: 40.0,
        collet_length: 30.0,
        sections: None,
    };
    let findings1 = check_tool_holder_collision(&toolpath, &holder_short, stock_bounds);
    assert_eq!(findings1.len(), 1);
    assert_eq!(findings1[0].code, "TOOL_HOLDER_COLLISION");
    assert_eq!(findings1[0].plunge_depth, 35.0);

    // Long holder with 50mm stickout (safely reaches 35mm depth)
    let holder_long = ToolHolder {
        holder_diameter: 50.0,
        stickout_length: 50.0,
        collet_diameter: 40.0,
        collet_length: 30.0,
        sections: None,
    };
    let findings2 = check_tool_holder_collision(&toolpath, &holder_long, stock_bounds);
    assert_eq!(findings2.len(), 0);
}

/// The rule exists to catch the holder fouling the stock, and the holder is wider than the cutter.
/// Testing the tip against the raw stock footprint missed a plunge just outside the stock edge
/// whose 50mm holder still overhangs it — the declared holder dimensions were never read.
#[test]
fn test_holder_overhang_outside_stock_footprint_is_detected() {
    let mut design = Design::default();
    // Plunge 35mm deep, 10mm clear of the stock's +X edge. The tip never enters the stock
    // footprint, but a 50mm-diameter holder reaches 25mm past the tip in every direction.
    design.ops.push(Op::Move {
        x: Some(110.0),
        y: Some(50.0),
        z: Some(10.0),
    });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 1200.0 });
    design.ops.push(Op::Move {
        x: Some(110.0),
        y: Some(50.0),
        z: Some(-35.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());
    let stock_bounds = [0.0, 100.0, 0.0, 100.0, -50.0, 0.0];

    let wide = ToolHolder {
        holder_diameter: 50.0,
        stickout_length: 20.0,
        collet_diameter: 40.0,
        collet_length: 30.0,
        sections: None,
    };
    let findings = check_tool_holder_collision(&toolpath, &wide, stock_bounds);
    assert_eq!(
        findings.len(),
        1,
        "a 25mm holder radius overhangs a tip 10mm clear of the edge"
    );
    assert_eq!(findings[0].code, "TOOL_HOLDER_COLLISION");

    // A slim holder at the same point clears the stock and must not be reported.
    let slim = ToolHolder {
        holder_diameter: 6.0,
        stickout_length: 20.0,
        collet_diameter: 6.0,
        collet_length: 30.0,
        sections: None,
    };
    assert!(
        check_tool_holder_collision(&toolpath, &slim, stock_bounds).is_empty(),
        "a 3mm holder radius does not reach the stock from 10mm away"
    );
}

#[test]
fn test_tilted_5axis_toolholder_collision() {
    let mut design = Design::default();
    // Tool tilted at 45 degrees
    design.ops.push(Op::Orient {
        i: std::f64::consts::FRAC_1_SQRT_2,
        j: 0.0,
        k: std::f64::consts::FRAC_1_SQRT_2,
    });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(-10.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());
    let stock_bounds = [0.0, 100.0, 0.0, 100.0, -50.0, 0.0];

    let holder = ToolHolder {
        holder_diameter: 50.0,
        stickout_length: 15.0,
        collet_diameter: 40.0,
        collet_length: 30.0,
        sections: None,
    };

    let findings = check_tool_holder_collision(&toolpath, &holder, stock_bounds);
    assert!(!findings.is_empty());
    assert_eq!(findings[0].code, "TOOL_HOLDER_5AXIS_COLLISION");
}

#[test]
fn test_stepped_tapered_toolholder_collision() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(-25.0),
    });

    let toolpath = resolve(&design, &ResolveParams::default());
    let stock_bounds = [0.0, 100.0, 0.0, 100.0, -50.0, 0.0];

    // Stepped holder with 15mm slim neck (diameter 12mm), then 40mm wide body (diameter 50mm)
    let holder_stepped = ToolHolder {
        holder_diameter: 50.0,
        stickout_length: 15.0,
        collet_diameter: 12.0,
        collet_length: 20.0,
        sections: Some(vec![
            dry_core::ToolHolderSection {
                diameter: 12.0,
                length: 5.0,
            },
            dry_core::ToolHolderSection {
                diameter: 50.0,
                length: 30.0,
            },
        ]),
    };

    let findings = check_tool_holder_collision(&toolpath, &holder_stepped, stock_bounds);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "TOOL_HOLDER_COLLISION");
}

