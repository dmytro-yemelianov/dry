use dry_core::PassRole;

#[test]
fn test_pass_role_metadata_and_colors() {
    assert_eq!(PassRole::Roughing.as_str(), "roughing");
    assert_eq!(PassRole::Finishing.as_str(), "finishing");
    assert_eq!(PassRole::Perimeter.as_str(), "perimeter");
    assert_eq!(PassRole::Infill.as_str(), "infill");

    assert_eq!(PassRole::Roughing.default_color(), "#2563eb");
    assert_eq!(PassRole::Finishing.default_color(), "#16a34a");
    assert_eq!(PassRole::Travel.default_color(), "#ef4444");

    let json = serde_json::to_string(&PassRole::Roughing).expect("must serialize pass role");
    assert_eq!(json, "\"roughing\"");

    let deserialized: PassRole = serde_json::from_str(&json).expect("must deserialize pass role");
    assert_eq!(deserialized, PassRole::Roughing);
}
