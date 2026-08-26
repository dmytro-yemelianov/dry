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
    };
    let findings2 = check_tool_holder_collision(&toolpath, &holder_long, stock_bounds);
    assert_eq!(findings2.len(), 0);
}
