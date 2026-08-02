//! Offline Ed25519 license-token verification. No network, no clock source of
//! its own (callers pass `now_unix`), no signing in production builds.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const TOKEN_PREFIX: &str = "DRY-LICENSE-V1";
pub const GRACE_SECS: u64 = 14 * 24 * 60 * 60;

/// TEST keypair id — the matching signing key is COMMITTED at
/// crates/license/tests/fixtures/ and is deliberately non-secret.
pub const TEST_KEY_ID: &str = "test-1";
pub const TEST_VERIFYING_KEY: [u8; 32] = [
    0xe3, 0xd3, 0x92, 0x0c, 0x08, 0xe7, 0x04, 0xcc, 0xa8, 0x18, 0x3d, 0xf6, 0x1d, 0xfe, 0x4b, 0x98,
    0x24, 0xc4, 0x43, 0xb6, 0xab, 0x23, 0x0c, 0x20, 0x52, 0x24, 0x43, 0x20, 0x66, 0xe2, 0x44, 0x60,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Solo,
    Team,
    Pilot,
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Tier::Solo => "solo",
            Tier::Team => "team",
            Tier::Pilot => "pilot",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicensePayload {
    pub id: String,
    pub licensee: String,
    pub email: String,
    pub tier: Tier,
    pub machines: u32,
    pub issued: String,
    pub expires: String,
    pub issued_unix: u64,
    pub expires_unix: u64,
    pub key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseState {
    Valid,
    Grace { days_left: u64 },
    Expired,
}

#[derive(Debug, Clone)]
pub struct VerifiedLicense {
    pub payload: LicensePayload,
    pub state: LicenseState,
}

#[derive(Debug)]
pub enum LicenseError {
    Malformed(String),
    UnknownKeyId(String),
    BadSignature,
}

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseError::Malformed(m) => write!(f, "malformed license token: {m}"),
            LicenseError::UnknownKeyId(k) => {
                write!(f, "license signed with unknown key id '{k}' (upgrade dry?)")
            }
            LicenseError::BadSignature => f.write_str("license signature verification failed"),
        }
    }
}

pub fn verify_token(
    token: &str,
    keys: &[(&str, [u8; 32])],
    now_unix: u64,
) -> Result<VerifiedLicense, LicenseError> {
    let mut parts = token.trim().splitn(3, '.');
    let (prefix, payload_b64, sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(p), Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => (p, a, b),
        _ => {
            return Err(LicenseError::Malformed(
                "expected three dot-separated parts".into(),
            ))
        }
    };
    if prefix != TOKEN_PREFIX {
        return Err(LicenseError::Malformed(format!(
            "unknown prefix '{prefix}'"
        )));
    }
    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| LicenseError::Malformed(format!("payload base64: {e}")))?;
    let payload: LicensePayload = serde_json::from_slice(&payload_bytes)
        .map_err(|e| LicenseError::Malformed(format!("payload json: {e}")))?;
    let sig_bytes = URL_SAFE_NO_PAD
        .decode(sig_b64)
        .map_err(|e| LicenseError::Malformed(format!("signature base64: {e}")))?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| LicenseError::Malformed("signature must be 64 bytes".into()))?;
    let vk_bytes = keys
        .iter()
        .find(|(id, _)| *id == payload.key_id)
        .map(|(_, k)| *k)
        .ok_or_else(|| LicenseError::UnknownKeyId(payload.key_id.clone()))?;
    let vk = VerifyingKey::from_bytes(&vk_bytes)
        .map_err(|_| LicenseError::Malformed("embedded verifying key invalid".into()))?;
    // The signature covers the raw base64url payload characters, not the decoded JSON.
    vk.verify_strict(payload_b64.as_bytes(), &Signature::from_bytes(&sig_arr))
        .map_err(|_| LicenseError::BadSignature)?;
    let state = if now_unix <= payload.expires_unix {
        LicenseState::Valid
    } else if now_unix <= payload.expires_unix + GRACE_SECS {
        let days_left = (payload.expires_unix + GRACE_SECS - now_unix) / 86_400;
        LicenseState::Grace { days_left }
    } else {
        LicenseState::Expired
    };
    Ok(VerifiedLicense { payload, state })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair() -> (SigningKey, [u8; 32]) {
        let sk = SigningKey::generate(&mut rand::rngs::OsRng);
        let vk = sk.verifying_key().to_bytes();
        (sk, vk)
    }

    fn payload_json(expires_unix: u64, key_id: &str) -> String {
        format!(
            r#"{{"id":"01TEST","licensee":"Test Co","email":"t@example.com","tier":"team","machines":25,"issued":"2026-07-28","expires":"2027-07-28","issued_unix":1785000000,"expires_unix":{expires_unix},"key_id":"{key_id}"}}"#
        )
    }

    fn make_token(sk: &SigningKey, payload_json: &str) -> String {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let p = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let sig = sk.sign(p.as_bytes());
        format!(
            "DRY-LICENSE-V1.{p}.{}",
            URL_SAFE_NO_PAD.encode(sig.to_bytes())
        )
    }

    const NOW: u64 = 1_790_000_000;

    #[test]
    fn valid_token_verifies() {
        let (sk, vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW + 1000, "k1"));
        let v = verify_token(&tok, &[("k1", vk)], NOW).unwrap();
        assert!(matches!(v.state, LicenseState::Valid));
        assert_eq!(v.payload.tier, Tier::Team);
        assert_eq!(v.payload.machines, 25);
    }

    #[test]
    fn expired_within_grace_reports_grace_with_days() {
        let (sk, vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW - 3 * 86_400, "k1"));
        let v = verify_token(&tok, &[("k1", vk)], NOW).unwrap();
        assert!(matches!(v.state, LicenseState::Grace { days_left: 11 }));
    }

    #[test]
    fn expired_past_grace_is_expired() {
        let (sk, vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW - GRACE_SECS - 1, "k1"));
        let v = verify_token(&tok, &[("k1", vk)], NOW).unwrap();
        assert!(matches!(v.state, LicenseState::Expired));
    }

    #[test]
    fn tampered_payload_fails_signature() {
        let (sk, vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW + 1000, "k1"));
        let mut parts: Vec<&str> = tok.splitn(3, '.').collect();
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let forged = URL_SAFE_NO_PAD.encode(payload_json(NOW + 999_999_999, "k1").as_bytes());
        parts[1] = &forged;
        let forged_tok = parts.join(".");
        assert!(matches!(
            verify_token(&forged_tok, &[("k1", vk)], NOW),
            Err(LicenseError::BadSignature)
        ));
    }

    #[test]
    fn wrong_key_fails_signature() {
        let (sk, _) = keypair();
        let (_, other_vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW + 1000, "k1"));
        assert!(matches!(
            verify_token(&tok, &[("k1", other_vk)], NOW),
            Err(LicenseError::BadSignature)
        ));
    }

    #[test]
    fn unknown_key_id_is_reported() {
        let (sk, vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW + 1000, "nope"));
        assert!(matches!(
            verify_token(&tok, &[("k1", vk)], NOW),
            Err(LicenseError::UnknownKeyId(_))
        ));
    }

    #[test]
    fn rotation_old_key_still_validates() {
        let (sk1, vk1) = keypair();
        let (_sk2, vk2) = keypair();
        let tok = make_token(&sk1, &payload_json(NOW + 1000, "k1"));
        let keys = [("k1", vk1), ("k2", vk2)];
        assert!(verify_token(&tok, &keys, NOW).is_ok());
    }

    #[test]
    fn malformed_tokens_are_malformed_not_panic() {
        let (_, vk) = keypair();
        for bad in [
            "",
            "DRY-LICENSE-V1.only-two",
            "WRONG-PREFIX.a.b",
            "DRY-LICENSE-V1.!!!.!!!",
            "DRY-LICENSE-V1.bm90anNvbg.c2ln",
        ] {
            assert!(
                matches!(
                    verify_token(bad, &[("k1", vk)], NOW),
                    Err(LicenseError::Malformed(_))
                ),
                "expected Malformed for {bad:?}"
            );
        }
    }
}
