//! Integration tests for `dry license activate|status` and env/file resolution.
//!
//! Conventions follow `crates/cli/tests/cli.rs`: shell out to the built binary via
//! `std::process::Command` + `env!("CARGO_BIN_EXE_dry")`.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_dry"))
}

fn team_token() -> &'static str {
    include_str!("../../license/tests/fixtures/js-signed-team.token")
}

#[test]
fn license_status_without_license_reports_eval() {
    let out = bin()
        .args(["license", "status"])
        .env_remove("DRY_LICENSE")
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-no-license"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("evaluation"), "got: {s}");
}

#[test]
fn env_var_license_is_recognized() {
    let out = bin()
        .args(["license", "status"])
        .env("DRY_LICENSE", team_token().trim())
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output()
        .unwrap();
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
    let ok = bin()
        .args(["license", "activate", tok_file.to_str().unwrap()])
        .env("XDG_CONFIG_HOME", &cfg)
        .env_remove("DRY_LICENSE")
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output()
        .unwrap();
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    let st = bin()
        .args(["license", "status"])
        .env("XDG_CONFIG_HOME", &cfg)
        .env_remove("DRY_LICENSE")
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&st.stdout).contains("team"));
}

/// A real g-code fixture — reused from the conventions in `crates/cli/tests/cli.rs`.
fn gcode_fixture() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/vectors/minimal_line/expected.gcode")
}

#[test]
fn eval_review_report_is_stamped_evaluation() {
    let out = bin()
        .args(["review-gcode", gcode_fixture().to_str().unwrap(), "--json"])
        .env_remove("DRY_LICENSE")
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-no-license"))
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["license"]["mode"], "evaluation");
    assert!(String::from_utf8_lossy(&out.stderr).contains("EVALUATION"));
}

#[test]
fn licensed_review_report_is_stamped_with_licensee() {
    let out = bin()
        .args(["review-gcode", gcode_fixture().to_str().unwrap(), "--json"])
        .env("DRY_LICENSE", team_token().trim())
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["license"]["mode"], "licensed");
    assert_eq!(v["license"]["tier"], "team");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("EVALUATION"));
}

#[cfg(feature = "moonraker")]
#[test]
fn upload_refuses_in_eval_before_any_network() {
    let out = bin()
        .args([
            "upload",
            gcode_fixture().to_str().unwrap(),
            "--moonraker",
            "http://127.0.0.1:1", // closed port: must NOT be contacted
        ])
        .env_remove("DRY_LICENSE")
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-no-license"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("requires a license"), "got: {err}");
    assert!(!err.contains("connection"), "network was attempted: {err}");
}

#[test]
fn garbage_token_activate_fails_cleanly() {
    let out = bin()
        .args(["license", "activate", "not-a-token"])
        .env("XDG_CONFIG_HOME", std::env::temp_dir().join("dry-garbage-license"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("malformed"));
}
