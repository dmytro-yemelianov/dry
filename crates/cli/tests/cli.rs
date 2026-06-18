//! End-to-end CLI tests: run the `dry` binary on a conformance fixture and check its output.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dry")
}

fn fixture(corpus: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../conformance/{corpus}/{name}.json"))
}

#[test]
fn emit_reproduces_the_fixture_gcode() {
    let path = fixture("gcode", "square");
    let out = Command::new(bin()).arg("emit").arg(&path).output().unwrap();
    assert!(out.status.success());
    let got: Vec<String> =
        String::from_utf8(out.stdout).unwrap().lines().map(String::from).collect();

    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let want: Vec<String> = doc["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, want, "`dry emit` output must match the fixture g-code");
}

#[test]
fn simulate_json_is_valid_and_matches_the_metrics() {
    let path = fixture("simulate", "square");
    let out = Command::new(bin()).args(["simulate", path.to_str().unwrap(), "--json"]).output().unwrap();
    assert!(out.status.success());
    let metrics: Value = serde_json::from_slice(&out.stdout).expect("valid JSON metrics");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(metrics["segment_count"], doc["expected"]["segment_count"]);
    assert!((metrics["total_time_s"].as_f64().unwrap()
        - doc["expected"]["total_time_s"].as_f64().unwrap())
    .abs()
        < 1e-9);
}

#[test]
fn inspect_runs_and_reports() {
    let path = fixture("gcode", "stack3");
    let out = Command::new(bin()).arg("inspect").arg(&path).output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("segments:") && text.contains("bbox:") && text.contains("peak flow:"));
}

#[test]
fn missing_file_exits_nonzero() {
    let out = Command::new(bin()).args(["emit", "/no/such/file.json"]).output().unwrap();
    assert!(!out.status.success());
}
