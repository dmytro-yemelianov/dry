# Dry Commercial License Product Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the licensed-CLI product per `docs/superpowers/specs/2026-07-28-commercial-license-product-design.md`: offline Ed25519 license verification in the binary, eval mode, a Cloudflare Worker issuer wired to Lemon Squeezy, and a self-serve docs surface — so a stranger can buy and self-onboard in under an hour.

**Architecture:** New `crates/license` (verify-only crypto, no network). CLI gains `dry license` subcommands, `DRY_LICENSE` env resolution, report stamping via an optional field (goldens untouched), and a license requirement on `upload`. A TypeScript Worker (`tools/license-issuer/`) signs tokens with WebCrypto Ed25519 (RFC 8032 — cross-verifiable by ed25519-dalek), triggered by Lemon Squeezy webhooks. Docs pages ride the existing VitePress public build.

**Tech Stack:** Rust (ed25519-dalek 2, base64 0.22, dirs 5), clap derive (existing), Cloudflare Workers + D1 + `send_email` binding, `@cloudflare/vitest-pool-workers`, Lemon Squeezy (merchant of record), VitePress (existing docs site).

**Spec refinement made here:** the token payload carries `expires_unix`/`issued_unix` (u64 seconds) as the authoritative times plus an informational `expires` display string — avoids pulling a date-parsing dependency into the license crate.

## Global Constraints

- Token format EXACTLY: `DRY-LICENSE-V1.<base64url-nopad(payload JSON)>.<base64url-nopad(64-byte Ed25519 sig over the raw base64url payload BYTES)>`.
- Payload JSON fields exactly: `id` (string), `licensee` (string), `email` (string), `tier` (`"solo"|"team"|"pilot"`), `machines` (u32), `issued` (string, display), `expires` (string, display), `issued_unix` (u64), `expires_unix` (u64), `key_id` (string).
- Grace period: 14 days (1_209_600 s) after `expires_unix`. States: `Valid`, `Grace`, `Expired`. Tampered/malformed/unknown-key ⇒ treated as absent (eval), with a specific stderr warning — NEVER a hard exit from a report command.
- `DRY_LICENSE` env var takes precedence over the stored file (`<config_dir>/dry/license.token`).
- Enforcement lives ONLY in `crates/cli` + passive data in `crates/core` (`LicenseStamp`). `dry-core` gains NO crypto, NO deps. SDKs/wasm/py untouched.
- Report stamping: `#[serde(skip_serializing_if = "Option::is_none")] pub license: Option<LicenseStamp>` — goldens are built in core tests with `None` and MUST NOT change (drift gate `crates/core/tests/report_goldens.rs` proves it). Schema: add `license` to `properties` of the five report types in `spec/dry-reports-v1.schema.json`, NOT to any `required` array.
- Exit-code convention (existing): 0 success, 1 gate/verify failure, 2 usage/IO (`die()` at `crates/cli/src/main.rs:466-469`). Follow it; no anyhow.
- Eval mode = full function + stamps + human-output banner `EVALUATION — not for production gating`; `dry upload` refuses without a Valid/Grace license (exit 2 with a pointer to /pricing) BEFORE any network call.
- No real network in tests: no Lemon Squeezy calls, no emails, no Moonraker. Cross-stack tests use the committed TEST keypair (explicitly non-secret); production keys are generated in the key ceremony (Task 10) and never committed.
- Repo conventions: single `main.rs` CLI (add ~150 lines max there; logic lives in `crates/license`); integration tests via `std::process::Command` + `env!("CARGO_BIN_EXE_dry")` (`crates/cli/tests/cli.rs:29-36`); commit after every task.
- Prices/tiers verbatim from spec: Solo $990/yr (1 user, 3 machines), Team $4,990/yr (10 users, 25 machines), Pilot $1.5k–5k manual.
- gh pushes to this repo require the account dance: `gh auth switch -u dmytro-yemelianov` → push → `gh auth switch -u miwaniza`.

---

### Task 1: `crates/license` — token parsing and verification (TDD)

**Files:**
- Create: `crates/license/Cargo.toml`, `crates/license/src/lib.rs`
- Modify: `Cargo.toml:3` (workspace members: add `"crates/license"`)

**Interfaces:**
- Produces (consumed by Tasks 3/5):
  - `pub struct LicensePayload { pub id: String, pub licensee: String, pub email: String, pub tier: Tier, pub machines: u32, pub issued: String, pub expires: String, pub issued_unix: u64, pub expires_unix: u64, pub key_id: String }`
  - `pub enum Tier { Solo, Team, Pilot }` (serde lowercase)
  - `pub enum LicenseState { Valid, Grace { days_left: u64 }, Expired }`
  - `pub struct VerifiedLicense { pub payload: LicensePayload, pub state: LicenseState }`
  - `pub enum LicenseError { Malformed(String), UnknownKeyId(String), BadSignature }` (impl Display)
  - `pub fn verify_token(token: &str, keys: &[(&str, [u8; 32])], now_unix: u64) -> Result<VerifiedLicense, LicenseError>`
  - `pub const GRACE_SECS: u64 = 14 * 24 * 60 * 60;`
  - `pub const TEST_KEY_ID: &str = "test-1";` + `pub const TEST_VERIFYING_KEY: [u8; 32]` + `#[cfg(feature = "test-signing")] pub fn sign_token(payload_json: &str, signing_key_bytes: &[u8; 32], ...) -> String` (behind a feature so production builds carry no signing code).

- [ ] **Step 1: Crate scaffold + failing tests**

`crates/license/Cargo.toml`:
```toml
[package]
name = "dry-license"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
ed25519-dalek = { version = "2", default-features = false, features = ["std"] }

[features]
test-signing = ["ed25519-dalek/rand_core"]

[dev-dependencies]
ed25519-dalek = { version = "2", features = ["rand_core"] }
rand = "0.8"
```

Tests in `crates/license/src/lib.rs` `#[cfg(test)] mod tests` (write FIRST; each generates a keypair with `SigningKey::generate(&mut rand::rngs::OsRng)` and builds tokens via a local `make_token` helper):

```rust
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
        format!("DRY-LICENSE-V1.{p}.{}", URL_SAFE_NO_PAD.encode(sig.to_bytes()))
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
        assert!(matches!(verify_token(&forged_tok, &[("k1", vk)], NOW), Err(LicenseError::BadSignature)));
    }

    #[test]
    fn wrong_key_fails_signature() {
        let (sk, _) = keypair();
        let (_, other_vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW + 1000, "k1"));
        assert!(matches!(verify_token(&tok, &[("k1", other_vk)], NOW), Err(LicenseError::BadSignature)));
    }

    #[test]
    fn unknown_key_id_is_reported() {
        let (sk, vk) = keypair();
        let tok = make_token(&sk, &payload_json(NOW + 1000, "nope"));
        assert!(matches!(verify_token(&tok, &[("k1", vk)], NOW), Err(LicenseError::UnknownKeyId(_))));
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
        for bad in ["", "DRY-LICENSE-V1.only-two", "WRONG-PREFIX.a.b",
                    "DRY-LICENSE-V1.!!!.!!!", "DRY-LICENSE-V1.bm90anNvbg.c2ln"] {
            assert!(matches!(verify_token(bad, &[("k1", vk)], NOW), Err(LicenseError::Malformed(_))),
                    "expected Malformed for {bad:?}");
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p dry-license`
Expected: FAIL — types/functions not defined.

- [ ] **Step 3: Implement**

`crates/license/src/lib.rs` (above the tests module):
```rust
//! Offline Ed25519 license-token verification. No network, no clock source of
//! its own (callers pass `now_unix`), no signing in production builds.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

pub const TOKEN_PREFIX: &str = "DRY-LICENSE-V1";
pub const GRACE_SECS: u64 = 14 * 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier { Solo, Team, Pilot }

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Tier::Solo => "solo", Tier::Team => "team", Tier::Pilot => "pilot" })
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
pub enum LicenseState { Valid, Grace { days_left: u64 }, Expired }

#[derive(Debug, Clone)]
pub struct VerifiedLicense { pub payload: LicensePayload, pub state: LicenseState }

#[derive(Debug)]
pub enum LicenseError { Malformed(String), UnknownKeyId(String), BadSignature }

impl std::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseError::Malformed(m) => write!(f, "malformed license token: {m}"),
            LicenseError::UnknownKeyId(k) => write!(f, "license signed with unknown key id '{k}' (upgrade dry?)"),
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
        _ => return Err(LicenseError::Malformed("expected three dot-separated parts".into())),
    };
    if prefix != TOKEN_PREFIX {
        return Err(LicenseError::Malformed(format!("unknown prefix '{prefix}'")));
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
```
Add `"crates/license"` to the workspace `members` array in the root `Cargo.toml:3`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p dry-license` — Expected: 8/8 PASS. Also `cargo clippy -p dry-license -- -D warnings` clean.

- [ ] **Step 5: Commit**

```bash
git add crates/license Cargo.toml Cargo.lock
git commit -m "feat(license): offline Ed25519 license token verification crate"
```

---

### Task 2: Committed test keypair + JS↔Rust cross-verification fixture

**Files:**
- Create: `tools/license-issuer/scripts/keygen.mjs`, `crates/license/tests/fixtures/test-signing-key.json`, `crates/license/tests/cross_stack.rs`
- Modify: `crates/license/src/lib.rs` (add the TEST key constants)

**Interfaces:**
- Produces: `keygen.mjs` (Node ≥22, WebCrypto Ed25519) prints `{ key_id, verifying_key_hex, signing_key_pkcs8_b64 }`; the committed TEST keypair (key_id `test-1`, EXPLICITLY non-secret, used by CLI tests in Task 6); a fixture token signed by the JS side and verified by Rust — proving RFC 8032 cross-compatibility before the Worker exists.

- [ ] **Step 1: keygen script**

`tools/license-issuer/scripts/keygen.mjs`:
```js
// Generates an Ed25519 keypair for license signing.
// PRODUCTION USE: run once, store signing_key as a Cloudflare Worker secret +
// offline backup; paste verifying_key_hex + key_id into crates/license/src/lib.rs.
// NEVER commit a production signing key.
import { webcrypto } from 'node:crypto';

const keyId = process.argv[2] ?? `key-${new Date().toISOString().slice(0, 10)}`;
const pair = await webcrypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
const rawPub = Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey));
const pkcs8 = Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey));
console.log(JSON.stringify({
  key_id: keyId,
  verifying_key_hex: rawPub.toString('hex'),
  signing_key_pkcs8_b64: pkcs8.toString('base64'),
}, null, 2));
```

- [ ] **Step 2: Generate and commit the TEST pair + a JS-signed fixture token**

```bash
node tools/license-issuer/scripts/keygen.mjs test-1 > crates/license/tests/fixtures/test-signing-key.json
```
Then a one-off sign step (add `scripts/sign.mjs` — reused by the Worker tests later):
```js
// tools/license-issuer/scripts/sign.mjs <key-file> <payload-json-file>
import { webcrypto } from 'node:crypto';
import { readFileSync } from 'node:fs';
const key = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const payload = readFileSync(process.argv[3], 'utf8').trim();
const b64url = (buf) => Buffer.from(buf).toString('base64url');
const sk = await webcrypto.subtle.importKey(
  'pkcs8', Buffer.from(key.signing_key_pkcs8_b64, 'base64'), 'Ed25519', false, ['sign']);
const p = b64url(payload);
const sig = await webcrypto.subtle.sign('Ed25519', sk, Buffer.from(p));
console.log(`DRY-LICENSE-V1.${p}.${b64url(sig)}`);
```
Create a payload file with key_id `test-1`, tier `team`, expires_unix far future (e.g. `4102444800` = 2100-01-01), sign it, save the token to `crates/license/tests/fixtures/js-signed-team.token`.

- [ ] **Step 3: Rust cross-stack test**

`crates/license/tests/cross_stack.rs`:
```rust
use dry_license::{verify_token, LicenseState, Tier};

fn test_vk() -> [u8; 32] {
    let j: serde_json::Value = serde_json::from_str(include_str!("fixtures/test-signing-key.json")).unwrap();
    let hex = j["verifying_key_hex"].as_str().unwrap();
    let bytes: Vec<u8> = (0..hex.len()).step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    bytes.try_into().unwrap()
}

#[test]
fn js_webcrypto_signed_token_verifies_in_rust() {
    let token = include_str!("fixtures/js-signed-team.token");
    let v = verify_token(token, &[("test-1", test_vk())], 1_790_000_000).unwrap();
    assert!(matches!(v.state, LicenseState::Valid));
    assert_eq!(v.payload.tier, Tier::Team);
}
```
Also add to `crates/license/src/lib.rs`:
```rust
/// TEST keypair id — the matching signing key is COMMITTED at
/// crates/license/tests/fixtures/ and is deliberately non-secret.
pub const TEST_KEY_ID: &str = "test-1";
pub const TEST_VERIFYING_KEY: [u8; 32] = [ /* paste bytes from verifying_key_hex */ ];
```
(`dev`/test builds of the CLI accept `test-1`; Task 5 wires production key acceptance so that the test key is honored ONLY when `cfg!(debug_assertions)` or env `DRY_LICENSE_ALLOW_TEST_KEY=1` — keeping release binaries from trusting it silently.)

- [ ] **Step 4: Verify** — `cargo test -p dry-license` all green (incl. cross_stack).

- [ ] **Step 5: Commit**

```bash
git add tools/license-issuer/scripts crates/license
git commit -m "feat(license): JS keygen/sign scripts and cross-stack verification fixture"
```

---

### Task 3: CLI `dry license activate|status` + resolution (env > file)

**Files:**
- Modify: `crates/cli/Cargo.toml` (add `dry-license` path dep + `dirs = "5"`), `crates/cli/src/main.rs`
- Test: `crates/cli/tests/license.rs`

**Interfaces:**
- Consumes: `dry_license::{verify_token, VerifiedLicense, LicenseState, LicenseError, TEST_KEY_ID, TEST_VERIFYING_KEY}`.
- Produces (used by Task 5): `fn resolve_license() -> LicenseResolution` in main.rs where `enum LicenseResolution { Licensed(VerifiedLicense), Eval { warning: Option<String> } }` — env `DRY_LICENSE` first, then `<dirs::config_dir()>/dry/license.token`; any error ⇒ `Eval` with the error text as warning (NEVER exits). `PRODUCTION_KEYS: &[(&str, [u8;32])]` const (placeholder `prod-1` key of zeros until Task 10's ceremony; zeros never verify, which is correct pre-ceremony).

- [ ] **Step 1: Failing integration tests**

`crates/cli/tests/license.rs` (conventions from `crates/cli/tests/cli.rs:29-36` — `std::process::Command`, `env!("CARGO_BIN_EXE_dry")`):
```rust
use std::process::Command;

fn bin() -> Command { Command::new(env!("CARGO_BIN_EXE_dry")) }

fn team_token() -> &'static str {
    include_str!("../../license/tests/fixtures/js-signed-team.token")
}

#[test]
fn license_status_without_license_reports_eval() {
    let out = bin().args(["license", "status"])
        .env_remove("DRY_LICENSE")
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-no-license"))
        .output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("evaluation"), "got: {s}");
}

#[test]
fn env_var_license_is_recognized() {
    let out = bin().args(["license", "status"])
        .env("DRY_LICENSE", team_token().trim())
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output().unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(s.contains("team") && s.contains("Test"), "got: {s}");
}

#[test]
fn activate_stores_and_status_reads_back() {
    let cfg = std::env::temp_dir().join(format!("dry-lic-{}", std::process::id()));
    let tok_file = cfg.join("in.token");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(&tok_file, team_token()).unwrap();
    let ok = bin().args(["license", "activate", tok_file.to_str().unwrap()])
        .env("XDG_CONFIG_HOME", &cfg).env_remove("DRY_LICENSE")
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output().unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    let st = bin().args(["license", "status"])
        .env("XDG_CONFIG_HOME", &cfg).env_remove("DRY_LICENSE")
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output().unwrap();
    assert!(String::from_utf8_lossy(&st.stdout).contains("team"));
}

#[test]
fn garbage_token_activate_fails_cleanly() {
    let out = bin().args(["license", "activate", "not-a-token"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("malformed"));
}
```
(Note: `dirs::config_dir()` honors `XDG_CONFIG_HOME` on Linux; on macOS it does not — the resolution helper must read `XDG_CONFIG_HOME` itself first for testability, then fall back to `dirs::config_dir()`. Document this in a comment.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p dry-cli --test license` fails (no such subcommand).

- [ ] **Step 3: Implement in main.rs**

Add to `enum Cmd` (pattern of `main.rs:71-464`):
```rust
/// Manage the commercial license (activate a token, show status)
License {
    #[command(subcommand)]
    action: LicenseAction,
},
```
```rust
#[derive(clap::Subcommand)]
enum LicenseAction {
    /// Verify and store a license token (argument: token string or a file containing it)
    Activate { token_or_file: String },
    /// Show the active license, its tier and expiry state
    Status,
}
```
Resolution + helpers (new section near the other helper fns):
```rust
const PRODUCTION_KEYS: &[(&str, [u8; 32])] = &[
    // "prod-1" is installed by the key ceremony (see the release runbook).
    ("prod-1", [0u8; 32]),
];

fn license_keys() -> Vec<(&'static str, [u8; 32])> {
    let mut keys: Vec<(&'static str, [u8; 32])> = PRODUCTION_KEYS.to_vec();
    let allow_test = cfg!(debug_assertions)
        || std::env::var("DRY_LICENSE_ALLOW_TEST_KEY").is_ok_and(|v| v == "1");
    if allow_test {
        keys.push((dry_license::TEST_KEY_ID, dry_license::TEST_VERIFYING_KEY));
    }
    keys
}

fn license_config_path() -> std::path::PathBuf {
    // XDG_CONFIG_HOME first (also makes macOS tests hermetic), then the platform dir.
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(dirs::config_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("dry").join("license.token")
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

enum LicenseResolution {
    Licensed(dry_license::VerifiedLicense),
    Eval { warning: Option<String> },
}

fn resolve_license() -> LicenseResolution {
    let token = match std::env::var("DRY_LICENSE") {
        Ok(t) if !t.trim().is_empty() => Some(t),
        _ => std::fs::read_to_string(license_config_path()).ok(),
    };
    let Some(token) = token else { return LicenseResolution::Eval { warning: None } };
    let keys = license_keys();
    match dry_license::verify_token(&token, &keys, now_unix()) {
        Ok(v) => match v.state {
            dry_license::LicenseState::Expired => LicenseResolution::Eval {
                warning: Some(format!(
                    "license for {} expired {} (past the 14-day grace) — running in evaluation mode",
                    v.payload.licensee, v.payload.expires
                )),
            },
            _ => LicenseResolution::Licensed(v),
        },
        Err(e) => LicenseResolution::Eval { warning: Some(format!("{e} — running in evaluation mode")) },
    }
}
```
Dispatch arm in `run()` (`main.rs:571` match): `Activate` reads the arg as a file if the path exists, else treats it as the token; verifies with `license_keys()` (any `LicenseError` ⇒ `die(...)` per the exit-2 convention at `main.rs:466-469`); on success `std::fs::create_dir_all` + write to `license_config_path()` and print licensee/tier/expiry. `Status` prints either the licensed details (licensee, tier, machines, expires, state incl. grace days) or `mode: evaluation` + purchase pointer, and surfaces the resolution warning if any. Both return `ExitCode::SUCCESS`.

`crates/cli/Cargo.toml` additions: `dry-license = { path = "../license" }`, `dirs = "5"` (regular deps — license handling is NOT feature-gated).

- [ ] **Step 4: Run to verify pass** — `cargo test -p dry-cli --test license` 4/4; `cargo test -p dry-cli` all green (existing suites unaffected); clippy clean.

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat(cli): dry license activate/status with env-first resolution"`

---

### Task 4: `LicenseStamp` on report structs (goldens must NOT drift)

**Files:**
- Modify: `crates/core/src/report.rs` (ReviewReport `:43`, RewriteReport `:132`, new struct), `crates/core/src/verify.rs` (Report `:307`), `crates/core/src/explain.rs` (ExplainBundle `:26`), `crates/core/src/compare.rs` (CompareDelta `:70`), `spec/dry-reports-v1.schema.json`
- Test: extend `crates/core/tests/report_goldens.rs` usage (no golden regeneration!)

**Interfaces:**
- Produces: `pub struct LicenseStamp { pub mode: String, #[serde(skip_serializing_if = "Option::is_none")] pub licensee: Option<String>, #[serde(skip_serializing_if = "Option::is_none")] pub tier: Option<String> }` in `crates/core/src/report.rs`, re-exported from core's root; each of the five structs gains `#[serde(skip_serializing_if = "Option::is_none")] #[serde(default)] pub license: Option<LicenseStamp>`; all `::build` constructors set `license: None` (CLI stamps post-build in Task 5).

- [ ] **Step 1: Write the drift-proof test FIRST**

Add to `crates/core/tests/report_goldens.rs` a test asserting a stamped report round-trips and an unstamped one serializes WITHOUT the key:
```rust
#[test]
fn license_stamp_is_absent_when_none() {
    // Any golden case works; goldens are built without a stamp, so committed
    // bytes must remain valid — this is the no-drift guarantee.
    let sample = std::fs::read_to_string(reports_dir().join("structural/review.json")).unwrap();
    assert!(!sample.contains("\"license\""));
    let parsed: dry_core::ReviewReport = serde_json::from_str(&sample).unwrap();
    assert!(parsed.license.is_none());
}
```
(Adjust the golden path/type to a real existing case file — check `conformance/reports/structural/` contents first.)

- [ ] **Step 2: Run to verify failure** — field doesn't exist yet; compile error is the RED.

- [ ] **Step 3: Implement**

Add `LicenseStamp` + the optional field to the five structs; update every constructor/builder (`ReviewReport::build` `report.rs:58`, `RewriteReport` builder `:161`, `build_explain_bundle` `explain.rs:45`, `compare_reports` `compare.rs:84`, verify `Report` construction sites) to set `license: None`.

Schema `spec/dry-reports-v1.schema.json`: add a `LicenseStamp` definition (`mode` string required; `licensee`, `tier` optional strings; `additionalProperties: false`) and a `license` property (NOT in `required`) to the five report type definitions.

- [ ] **Step 4: Verify no drift + full gates**

Run: `cargo test -p dry-core` (goldens must pass UNCHANGED — if `report_goldens.rs` reports drift, the serde attrs are wrong; fix the code, do not regenerate), then `python3 tools/validate_reports.py` (all goldens still schema-valid), then `cargo test --workspace`.

- [ ] **Step 5: Commit** — `git commit -am "feat(core): optional LicenseStamp on report envelopes (golden-stable)"`

---

### Task 5: Eval/licensed wiring — stamps, banners, upload gate

**Files:**
- Modify: `crates/cli/src/main.rs`
- Test: `crates/cli/tests/license.rs` (extend)

**Interfaces:**
- Consumes: `resolve_license()` (Task 3), `LicenseStamp` (Task 4), upload flow at `main.rs:1892-2080`.
- Behavior contract:
  - `fn license_stamp(res: &LicenseResolution) -> dry_core::LicenseStamp` — `mode: "licensed"` + licensee/tier, or `mode: "evaluation"`.
  - The five report-producing paths (`review-gcode`, `verify`, `compare`, `explain`, `rewrite-gcode`) set `report.license = Some(stamp)` right before serialization/printing (JSON and human paths both).
  - Human (non-`--json`) output in eval mode prints exactly one stderr banner: `EVALUATION — not for production gating. https://dry-public-docs.pages.dev/pricing` (single constant; page URL updated in Task 9 if the domain changes).
  - Grace state prints a stderr warning with days left on every licensed run.
  - `dry upload`: immediately after arg parsing in `run_upload` (`main.rs:1898` area), `Eval` ⇒ `die("dry upload requires a license — see https://…/pricing")` (exit 2, BEFORE any Moonraker contact). `Licensed`/`Grace` proceeds.

- [ ] **Step 1: Failing tests** (extend `crates/cli/tests/license.rs`)

```rust
#[test]
fn eval_review_report_is_stamped_evaluation() {
    // conformance fixture path convention per crates/cli/tests/cli.rs fixture()
    let out = bin().args(["review-gcode", fixture("gcode/minimal.gcode"), "--json"])
        .env_remove("DRY_LICENSE")
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-no-license"))
        .output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["license"]["mode"], "evaluation");
    assert!(String::from_utf8_lossy(&out.stderr).contains("EVALUATION"));
}

#[test]
fn licensed_review_report_is_stamped_with_licensee() {
    let out = bin().args(["review-gcode", fixture("gcode/minimal.gcode"), "--json"])
        .env("DRY_LICENSE", team_token().trim())
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["license"]["mode"], "licensed");
    assert_eq!(v["license"]["tier"], "team");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("EVALUATION"));
}

#[cfg(feature = "moonraker")]
#[test]
fn upload_refuses_in_eval_before_any_network() {
    let out = bin().args(["upload", fixture("gcode/minimal.gcode"),
                          "--moonraker", "http://127.0.0.1:1"]) // closed port: must NOT be contacted
        .env_remove("DRY_LICENSE")
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-no-license"))
        .output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("requires a license"), "got: {err}");
    assert!(!err.contains("connection"), "network was attempted: {err}");
}
```
(Adapt `fixture()` names to real files in `conformance/` — check what `tests/cli.rs` uses and reuse the same inputs.)

- [ ] **Step 2: RED** — `cargo test -p dry-cli --test license` fails on the three new tests.

- [ ] **Step 3: Implement** per the behavior contract above. Call `resolve_license()` ONCE per invocation (top of `run()`), pass the resolution into the arms that need it. Keep total main.rs additions across Tasks 3+5 under ~150 lines by leaning on the license crate.

- [ ] **Step 4: GREEN** — license tests all pass; `cargo test --workspace` green (the existing upload tests at `tests/cli.rs:86-92` now need a license env — set `DRY_LICENSE` + `DRY_LICENSE_ALLOW_TEST_KEY=1` in those tests; that modification is in-scope here).

- [ ] **Step 5: Commit** — `git commit -am "feat(cli): eval/licensed modes — report stamps, banners, upload license gate"`

---

### Task 6: Issuer Worker — scaffold, webhook, signing, email, D1

**Files:**
- Create: `tools/license-issuer/{package.json,wrangler.jsonc,tsconfig.json,src/index.ts,src/sign.ts,src/token.ts,schema.sql,vitest.config.ts,test/issuer.test.ts,.gitignore}`

**Interfaces:**
- `POST /webhook/lemonsqueezy` — verifies `X-Signature` (HMAC-SHA256 of raw body with `LS_WEBHOOK_SECRET`); handles `order_created` + `subscription_payment_success`; maps LS variant → tier via `TIER_BY_VARIANT` env JSON (`{"<variant_id>":"solo",...}`); builds payload (id: `crypto.randomUUID()`, licensee/email from LS payload, machines by tier {solo:3,team:25,pilot:25}, issued/expires display + unix — 1 year + 3 days), signs (Ed25519 WebCrypto, `SIGNING_KEY_PKCS8_B64` secret, `KEY_ID` var), INSERTs into D1 `licenses`, emails the token via `send_email` binding, 200.
- `POST /admin/issue` — `Authorization: Bearer ADMIN_TOKEN` (secret); JSON body `{licensee,email,tier,machines,days}` → same sign+log+email path; returns the token in the response too (for manual delivery).
- All other routes 404. Every response includes the same security-headers pattern used in the yemelianov-dev portfolio Worker functions.
- `schema.sql`: `CREATE TABLE licenses (id TEXT PRIMARY KEY, email TEXT NOT NULL, licensee TEXT NOT NULL, tier TEXT NOT NULL, expires_unix INTEGER NOT NULL, order_id TEXT, revoked INTEGER DEFAULT 0, created_at TEXT DEFAULT (datetime('now')));`
- Refund/`subscription_expired` events: `UPDATE licenses SET revoked = 1` (log-only; offline keys uncallable — spec-accepted).

Implementation notes: `src/token.ts` exports `buildPayload(...)` and `formatToken(payloadB64, sigB64)` mirroring the Rust format EXACTLY (base64url no-pad; signature over the payload base64url BYTES). `src/sign.ts` wraps WebCrypto import/sign (same code path as `scripts/sign.mjs` — extract shared logic so the fixture and Worker cannot drift). wrangler.jsonc: D1 binding `DB`, `send_email` binding `EMAIL` (from `license@` sender identity — Task 10 decides the domain), secrets `LS_WEBHOOK_SECRET`, `SIGNING_KEY_PKCS8_B64`, `ADMIN_TOKEN`; vars `KEY_ID`, `TIER_BY_VARIANT`, `MAIL_FROM`, `MAIL_TO_BCC` (owner copy).

- [ ] **Step 1: Failing tests** — `test/issuer.test.ts` under `@cloudflare/vitest-pool-workers` (config pattern from the portfolio's `vitest.workers.config.ts`: `cloudflareTest()` plugin, miniflare with a real D1). Cases: (1) webhook with bad HMAC → 401, no D1 row, no email; (2) `order_created` with valid HMAC (compute in-test with the same secret) → 200, D1 row exists, email-send stub called once, and the emailed token **verifies against the TEST public key with correct tier/expiry** (import the fixture keypair from `crates/license/tests/fixtures/test-signing-key.json` as the signing secret in tests); (3) `/admin/issue` without bearer → 401; with bearer → 200 + valid token; (4) refund event → row `revoked=1`; (5) unknown route → 404. Email stubbing: bind `EMAIL` to a recording stub in miniflare options — no real sends.
- [ ] **Step 2: RED** — `cd tools/license-issuer && npm i && npx vitest run` fails.
- [ ] **Step 3: Implement** `src/token.ts`, `src/sign.ts`, `src/index.ts` per the interface block. HMAC verify: `crypto.subtle.importKey('raw', secret, {name:'HMAC', hash:'SHA-256'}, ...)` + `verify` against the hex `X-Signature` — timing-safe by construction.
- [ ] **Step 4: GREEN** — all issuer tests pass. Also run one manual local loop: `npx wrangler dev` + curl the admin endpoint with the test signing key configured → paste the returned token into `DRY_LICENSE_ALLOW_TEST_KEY=1 dry license status` → shows licensed. Record the output.
- [ ] **Step 5: Commit** — `git add tools/license-issuer && git commit -m "feat(issuer): license-signing Worker with LS webhook, admin issue, D1 audit log"`

---

### Task 7: Docs surface — pricing, activation, quickstart

**Files:**
- Create: `docs/site/pricing.md`, `docs/site/activate.md`, `docs/site/guide/ci-gate-quickstart.md`
- Modify: `docs/site/licensing.md` (from "contact the owner" to the real flow), `docs/site/.vitepress/config.ts` (`nav` `:96-102`: add Pricing; sidebar `/guide/` gets the quickstart), `docs/site/scripts/check-public-boundary.mjs` (`allowedPublicSourceFiles` `:38-53`: add `docs/site/pricing.md`, `docs/site/activate.md`)

Content requirements (write real copy, not placeholders):
- **pricing.md**: the three tiers with the spec's exact prices/limits; what's in eval mode vs licensed (table); honest support terms ("email, best-effort; priority for Team; no SLA pre-1.0" — consistent with `docs/16-support-matrix.md:88-90`); checkout buttons as plain links to `https://dry.lemonsqueezy.com/checkout/buy/<VARIANT_UUID_SOLO>` / `<VARIANT_UUID_TEAM>` placeholders wired in Task 9; pilot CTA = mailto; one paragraph positioning against the competitive landscape (deterministic gate + advisory LLM; cite the OOPSLA/LLM-ADAM research line as third-party problem validation, per the spec appendix).
- **activate.md**: `dry license activate` + `DRY_LICENSE` env setup (GitHub Actions snippet with `secrets.DRY_LICENSE`), grace/renewal behavior, air-gapped FAQ, "what we collect: nothing — verification is offline".
- **ci-gate-quickstart.md** ("60 minutes to a gated pipeline"): install from GitHub Releases (curl + tar for linux x64; brew-less), eval run on the reader's own G-code (`dry review-gcode`), reading findings, buy → secret → the full GitHub Actions job YAML gating on exit code 1, `dry upload --moonraker` as the print-side gate.

- [ ] **Step 1: Write pages + nav/sidebar/boundary edits.**
- [ ] **Step 2: Verify** — `cd docs/site && DRY_DOCS_MODE=public bash build.sh` green (boundary check passes with the two new allowlisted files); links resolve in the built output.
- [ ] **Step 3: Commit** — `git commit -am "docs(site): pricing, activation, CI-gate quickstart pages"`

---

### Task 8: v0.5.0 release prep

**Files:**
- Modify: `CHANGELOG.md` (new `## [0.5.0]` from Unreleased: license subcommands, eval mode, report stamps, upload gate), `Cargo.toml:11` + `py/pyproject.toml` + `py/Cargo.toml` + `crates/wasm/Cargo.toml` + `sdk/ts/package.json` + `package-lock` (all to `0.5.0` — `scripts/check-version.sh v0.5.0` must pass), `docs/16-support-matrix.md` (add license/eval row)

- [ ] **Step 1: Version bumps + changelog; run `bash scripts/check-version.sh v0.5.0` → all ok.**
- [ ] **Step 2: `cargo test --workspace` + the conformance validators green.**
- [ ] **Step 3: Commit** — `git commit -am "chore: prepare 0.5.0 (commercial licensing)"`. Do NOT tag yet — tag in Task 11 after the E2E purchase test.

---

### Task 9: USER — Lemon Squeezy + Cloudflare setup (checklist, controller-assisted)

No repo files. Present as a checklist and wait:
1. Create the Lemon Squeezy store; two products (Dry Solo $990/yr, Dry Team $4,990/yr, both "license length: 1 year, subscription"); note the two variant IDs.
2. Decide the sender/product domain (recommend `dry.yemelianov.dev` on the docs Pages project + `license@yemelianov.dev` sender — Email Sending domain onboarding for yemelianov.dev must be done if not already; this is the same open TODO as the portfolio contact form).
3. **Key ceremony:** run `node tools/license-issuer/scripts/keygen.mjs prod-1`; store `signing_key_pkcs8_b64` as (a) Worker secret, (b) an offline backup the owner keeps; paste `verifying_key_hex` bytes into `PRODUCTION_KEYS` in `crates/cli/src/main.rs` (replacing the zeros; commit). The private key never touches the repo.
4. Create D1 database (`wrangler d1 create dry-licenses` + apply `schema.sql`); set Worker secrets (`LS_WEBHOOK_SECRET` from the LS webhook config, `SIGNING_KEY_PKCS8_B64`, `ADMIN_TOKEN`); vars (`KEY_ID=prod-1`, `TIER_BY_VARIANT`, `MAIL_FROM`); `wrangler deploy`.
5. Point the LS webhook at the deployed Worker URL; enable `order_created`, `subscription_payment_success`, `refunded`, `subscription_expired`.
6. Fill the two variant UUIDs into `docs/site/pricing.md` checkout links; rebuild/redeploy docs.
7. **EULA**: lawyer-reviewed commercial terms replace/extend `LICENSE` references on the pricing page (structure drafted by the controller; BLOCKING for real-money launch, not for test mode).

---

### Task 10: End-to-end test-mode purchase + release

- [ ] **Step 1:** LS test mode ON → complete a test purchase of Solo → confirm: webhook 200 in Worker logs, D1 row, email arrives with token, `dry license activate` accepts it (against `prod-1`), `dry license status` shows solo/expiry, a `review-gcode --json` run is stamped licensed.
- [ ] **Step 2:** Test the refund path (LS test refund → D1 `revoked=1`).
- [ ] **Step 3:** Tag the release: `git tag v0.5.0 && git push origin main v0.5.0` (account dance); watch `release.yml` to completion; verify the GitHub Release contains the CLI artifacts and the quickstart's install command works against it.
- [ ] **Step 4:** LS live mode ON (after EULA sign-off from Task 9.7). Pricing page live = launched.

---

## Self-review notes

- Spec coverage: token format/fields ✓ (T1), grace/never-brick ✓ (T1/T3/T5), env-first ✓ (T3), stamps ✓ (T4/T5), eval banner + upload gate ✓ (T5), Worker endpoints/HMAC/D1/email ✓ (T6), docs pages incl. quickstart + honest support ✓ (T7), sequencing (v0.4.0 already cut separately; v0.5.0 carries licensing) ✓ (T8/T10), key ceremony/rotation ✓ (T2/T9), E2E test purchase ✓ (T10), EULA lawyer gate ✓ (T9.7), no real network in tests ✓ (constraints + T6).
- Type consistency: `LicenseResolution`/`LicenseStamp`/`VerifiedLicense` names used identically across T3/T4/T5; token format identical T1/T2/T6 (shared JS sign module prevents drift).
- Known judgment points left to implementers: exact golden case path in T4 Step 1 (verify against the real `conformance/reports/` tree); existing upload tests' env update in T5 Step 4; LS webhook payload field paths (verify against LS docs when writing T6 — their JSON nests under `data.attributes`).
