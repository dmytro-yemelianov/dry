use dry_license::{verify_token, LicenseState, Tier};

fn test_vk() -> [u8; 32] {
    let j: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/test-signing-key.json")).unwrap();
    let hex = j["verifying_key_hex"].as_str().unwrap();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    bytes.try_into().unwrap()
}

#[test]
fn js_webcrypto_signed_token_verifies_in_rust() {
    let token = include_str!("fixtures/js-signed-team.token");
    let v = verify_token(token, &[("test-1", test_vk())], 1_790_000_000).unwrap();
    assert!(matches!(v.state, LicenseState::Valid));
    assert_eq!(v.payload.tier, Tier::Team);
}
