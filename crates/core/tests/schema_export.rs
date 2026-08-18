use dry_core::get_dialect_schema;

#[test]
fn test_get_dialect_schemas() {
    let intent_schema = get_dialect_schema("dry.intent/1").expect("intent schema exists");
    assert!(intent_schema.contains("DryIntentV1"));
    assert!(intent_schema.contains("dry.intent/1"));

    let path_schema = get_dialect_schema("dry.path/1").expect("path schema exists");
    assert!(path_schema.contains("DryPathV1"));

    let motion_schema = get_dialect_schema("dry.motion/1").expect("motion schema exists");
    assert!(motion_schema.contains("DryMotionV1"));

    let tool_schema = get_dialect_schema("dry.tool/1").expect("tool schema exists");
    assert!(tool_schema.contains("DryToolV1"));

    assert!(get_dialect_schema("unknown/99").is_none());
}
