use dry_core::{pocket_stepped_ops, CutMode, PocketOptions, PocketShape};

#[test]
fn test_stepped_pocket_multi_level_generation() {
    let opts = PocketOptions {
        shape: PocketShape::Rect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
        },
        mode: CutMode::Pocket,
        tool_diameter: 6.0,
        stepover: Some(0.5),
        depth: 0.0, // overridden by stepped generator
        depth_per_pass: None,
        z_top: Some(0.0),
        safe_z: Some(5.0),
        cut_feed: Some(1500.0),
        plunge_feed: Some(500.0),
        helical_entry: None,
    };

    // Cut total depth 6mm with 2mm max stepdown -> 3 depth levels (-2, -4, -6)
    let ops = pocket_stepped_ops(&opts, 6.0, 2.0).expect("stepped pocket ops generated");
    assert!(!ops.is_empty());

    let json = serde_json::to_string(&ops).expect("serialize ops");
    // Should have moves reaching -2, -4, and -6
    assert!(json.contains("-2") || json.contains("-2.0"));
    assert!(json.contains("-4") || json.contains("-4.0"));
    assert!(json.contains("-6") || json.contains("-6.0"));
}
