use dry_core::{ToolDefinition, ToolKind, ToolRegistry};

#[test]
fn test_tool_definition_and_registry_lookups() {
    let mut registry = ToolRegistry::new();
    assert!(registry.is_empty());

    let mut endmill = ToolDefinition::new(
        "flat_6mm",
        1,
        "6mm Flat Endmill",
        ToolKind::EndMill,
        6.0,
    );
    endmill.flute_length = Some(20.0);
    endmill.flute_count = Some(3);
    endmill.max_rpm = Some(18000.0);

    assert!(registry.register(endmill).is_ok());

    let drill = ToolDefinition::new(
        "spot_drill_90deg",
        2,
        "90° Spot Drill",
        ToolKind::Drill,
        4.0,
    );
    assert!(registry.register(drill).is_ok());

    assert_eq!(registry.len(), 2);

    let t1 = registry.get("flat_6mm").expect("must find tool 1 by id");
    assert_eq!(t1.number, 1);
    assert_eq!(t1.diameter, 6.0);

    let t2 = registry.get_by_number(2).expect("must find tool 2 by number");
    assert_eq!(t2.id, "spot_drill_90deg");

    // Test tool change emission
    let gcode = t1.emit_tool_change();
    assert_eq!(gcode[0], "T01 M06 ; Tool Change: 6mm Flat Endmill");
    assert_eq!(gcode[1], "G43 H01 ; Tool Length Offset");
}

#[test]
fn test_invalid_tool_definition_fails_validation() {
    let invalid_tool = ToolDefinition::new(
        "bad_tool",
        1,
        "Bad Tool",
        ToolKind::EndMill,
        -5.0, // negative diameter
    );
    assert!(invalid_tool.validate().is_err());
}
