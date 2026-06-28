//! End-to-end CLI tests: run the `dry` binary on a conformance fixture and check its output.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dry")
}

fn fixture(corpus: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../conformance/{corpus}/{name}.json"))
}

#[test]
fn emit_reproduces_the_fixture_gcode() {
    let path = fixture("gcode", "square");
    let out = Command::new(bin()).arg("emit").arg(&path).output().unwrap();
    assert!(out.status.success());
    let got: Vec<String> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();

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
    let out = Command::new(bin())
        .args(["simulate", path.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let metrics: Value = serde_json::from_slice(&out.stdout).expect("valid JSON metrics");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(metrics["segment_count"], doc["expected"]["segment_count"]);
    assert!(
        (metrics["total_time_s"].as_f64().unwrap()
            - doc["expected"]["total_time_s"].as_f64().unwrap())
        .abs()
            < 1e-9
    );
}

#[test]
fn pack_writes_chunked_binary_that_simulate_streams() {
    let path = fixture("simulate", "square");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let packed =
        std::env::temp_dir().join(format!("dry-cli-pack-{}-{stamp}.dry", std::process::id()));

    let out = Command::new(bin())
        .args([
            "pack",
            path.to_str().unwrap(),
            "-o",
            packed.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "pack failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let bytes = std::fs::read(&packed).unwrap();
    assert_eq!(&bytes[..4], b"DRY1");

    let out = Command::new(bin())
        .args(["simulate", packed.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&packed);
    assert!(
        out.status.success(),
        "simulate failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let metrics: Value = serde_json::from_slice(&out.stdout).expect("valid JSON metrics");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(metrics["segment_count"], doc["expected"]["segment_count"]);
    assert!(
        (metrics["total_time_s"].as_f64().unwrap()
            - doc["expected"]["total_time_s"].as_f64().unwrap())
        .abs()
            < 1e-9
    );
}

#[test]
fn import_gcode_writes_dry_ir_json() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-import-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E1.5 F1200\n").unwrap();

    let out = Command::new(bin())
        .args([
            "import-gcode",
            input.to_str().unwrap(),
            "--line-width",
            "0.45",
            "--layer-height",
            "0.2",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "import-gcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let ir: Value = serde_json::from_slice(&out.stdout).expect("valid Dry IR JSON");
    assert_eq!(ir["meta"]["generator"], "dry gcode importer");
    assert_eq!(ir["segments"].as_array().unwrap().len(), 2);
    assert_eq!(ir["segments"][0]["travel"], true);
    assert_eq!(ir["segments"][1]["travel"], false);
    assert_eq!(ir["segments"][1]["end"][0], 10.0);
    assert_eq!(ir["segments"][1]["filament"], 1.5);
    assert_eq!(ir["segments"][1]["width"], 0.45);
    assert_eq!(ir["segments"][1]["height"], 0.2);
}

#[test]
fn review_gcode_reports_findings_with_source_lines() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-review-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "; header\nM83\nG1 X0 Y0 Z0.2 F9000\nM104 S210\nG1 X10 E1.5 F1200\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "review-gcode",
            input.to_str().unwrap(),
            "--bounds",
            "0,5,0,5,0,1",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(!out.status.success(), "review-gcode should fail bounds");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("bounds"), "{text}");
    assert!(text.contains("line 5"), "{text}");
    assert!(text.contains("seg 1"), "{text}");
}

#[test]
fn review_gcode_uses_profile_contracts_and_import_defaults() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-profile-review-{}-{stamp}.gcode",
        std::process::id()
    ));
    let profile = std::env::temp_dir().join(format!(
        "dry-cli-profile-review-{}-{stamp}.json",
        std::process::id()
    ));
    std::fs::write(&input, "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E0.1 F1200\n").unwrap();
    std::fs::write(
        &profile,
        r#"{
          "version": 1,
          "name": "bench-profile",
          "firmware": {"flavor": "klipper"},
          "machine": {
            "build_volume": [[0, 5], [0, 5], [0, 1]],
            "feedrate_range": [1, 5000]
          },
          "material": {
            "filament_diameter": 1.75,
            "max_volumetric_flow_mm3_s": 100
          },
          "process": {
            "line_width": 0.48,
            "layer_height": 0.2
          }
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "review-gcode",
            input.to_str().unwrap(),
            "--profile",
            profile.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&profile);
    assert!(!out.status.success(), "profile bounds should fail");
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("profile:   bench-profile"), "{text}");
    assert!(text.contains("bounds"), "{text}");
    assert!(text.contains("line 3"), "{text}");
}

#[test]
fn review_gcode_cli_limits_override_profile_limits() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-profile-override-{}-{stamp}.gcode",
        std::process::id()
    ));
    let profile = std::env::temp_dir().join(format!(
        "dry-cli-profile-override-{}-{stamp}.json",
        std::process::id()
    ));
    std::fs::write(&input, "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E0.1 F1200\n").unwrap();
    std::fs::write(
        &profile,
        r#"{
          "version": 1,
          "name": "flow-test",
          "material": {
            "filament_diameter": 1.75,
            "max_volumetric_flow_mm3_s": 0.001
          },
          "process": {
            "line_width": 0.45,
            "layer_height": 0.2
          }
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "review-gcode",
            input.to_str().unwrap(),
            "--profile",
            profile.to_str().unwrap(),
            "--max-flow",
            "999",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&profile);
    assert!(
        out.status.success(),
        "explicit max-flow should override profile: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn trace_gcode_outputs_windowed_source_mapped_json() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-trace-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "M83\nG1 X0 Y0 F600\nG1 X100 E1 F600\n").unwrap();

    let out = Command::new(bin())
        .args([
            "trace-gcode",
            input.to_str().unwrap(),
            "--line-width",
            "0.45",
            "--layer-height",
            "0.2",
            "--window-s",
            "5",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "trace-gcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let json: Value = serde_json::from_slice(&out.stdout).expect("valid trace JSON");
    let windows = json["trace"]["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0]["source_line_start"], 2);
    assert_eq!(windows[0]["source_line_end"], 3);
    assert_eq!(windows[1]["source_line_start"], 3);
    assert!((json["trace"]["total_time_s"].as_f64().unwrap() - 10.0).abs() < 1e-9);
}

#[test]
fn rewrite_gcode_preserves_non_motion_source_lines() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-rewrite-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "; header\nM83\nG1 X0 Y0 Z0.2 F9000 ; move\nM104 S210\nG1 X10 E1.5 F1200\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args(["rewrite-gcode", input.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "rewrite-gcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lines: Vec<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines[0], "; header");
    assert_eq!(lines[1], "M83");
    assert_eq!(lines[2], "G21");
    assert_eq!(lines[3], "G90");
    assert_eq!(lines[4], "M83");
    assert!(lines[5].starts_with("G0 "));
    assert_eq!(lines[6], "M104 S210");
    assert_eq!(lines[7], "G21");
    assert_eq!(lines[8], "G90");
    assert_eq!(lines[9], "M83");
    assert!(lines[10].starts_with("G1 "));
}

#[test]
fn rewrite_gcode_normalizes_relative_xyz_before_rewritten_motion() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-rewrite-relative-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "G91\nM83\nG1 X10 E1 F1200\nG1 X10 E1 F1200\n").unwrap();

    let out = Command::new(bin())
        .args(["rewrite-gcode", input.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "rewrite-gcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lines: Vec<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines[0], "G91");
    assert_eq!(lines[2], "G21");
    assert_eq!(lines[3], "G90");
    assert!(lines.iter().any(|line| line == "G1 X20 E1"), "{lines:?}");
}

#[test]
fn rewrite_gcode_resets_preserved_flow_multiplier() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-rewrite-flow-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "M221 S90\nM83\nG1 X10 E1 F1200\n").unwrap();

    let out = Command::new(bin())
        .args(["rewrite-gcode", input.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "rewrite-gcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lines: Vec<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines[0], "M221 S90");
    assert!(lines.iter().any(|line| line == "M221 S100"), "{lines:?}");
    assert!(
        lines.iter().any(|line| line == "G1 F1200 X10 E0.9"),
        "{lines:?}"
    );
}

#[test]
fn rewrite_gcode_absolute_e_realigns_after_preserved_g92() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-rewrite-absolute-e-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "M82\nG1 X10 E1 F1200\nG92 E0\nG1 X20 E1 F1200\n").unwrap();

    let out = Command::new(bin())
        .args(["rewrite-gcode", input.to_str().unwrap(), "--absolute-e"])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "rewrite-gcode --absolute-e failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lines: Vec<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(lines.iter().any(|line| line == "G92 E1"), "{lines:?}");
    assert!(lines.iter().any(|line| line == "G1 X20 E2"), "{lines:?}");
}

#[test]
fn rewrite_gcode_optimizes_each_motion_span_locally() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-rewrite-opt-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(
        &input,
        concat!(
            "; header\n",
            "G1 X0 Y0 Z0.2 F1000\n",
            "G1 X1 Y0 Z0.2\n",
            "G1 X2 Y0 Z0.2\n",
            "M104 S210\n",
            "G1 X2 Y1 Z0.2\n",
            "G1 X2 Y2 Z0.2\n",
        ),
    )
    .unwrap();

    let out = Command::new(bin())
        .args(["rewrite-gcode", input.to_str().unwrap(), "--optimize"])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "rewrite-gcode --optimize failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lines: Vec<_> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines[0], "; header");
    assert!(lines.iter().any(|line| line == "M104 S210"));
    let motion_lines: Vec<_> = lines
        .iter()
        .filter(|line| {
            line.starts_with("G0 ")
                || line.starts_with("G1 ")
                || line.starts_with("G2 ")
                || line.starts_with("G3 ")
        })
        .collect();
    assert!(
        motion_lines.len() < 5,
        "span-local optimize should reduce motion lines: {lines:?}"
    );
}

#[test]
fn inspect_runs_and_reports() {
    let path = fixture("gcode", "stack3");
    let out = Command::new(bin())
        .arg("inspect")
        .arg(&path)
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("segments:") && text.contains("bbox:") && text.contains("peak flow:"));
}

#[test]
fn missing_file_exits_nonzero() {
    let out = Command::new(bin())
        .args(["emit", "/no/such/file.json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn verify_runs_and_reports_findings() {
    let path = fixture("gcode", "square");

    // clean path with bounds should succeed
    let out = Command::new(bin())
        .args([
            "verify",
            path.to_str().unwrap(),
            "--bounds",
            "0,100,0,100,0,50",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("OK (no findings)"));

    // out-of-bounds path should fail (non-zero exit code)
    let out_bad = Command::new(bin())
        .args(["verify", path.to_str().unwrap(), "--bounds", "0,5,0,5,0,5"])
        .output()
        .unwrap();
    assert!(!out_bad.status.success());
    let text_bad =
        String::from_utf8(out_bad.stderr).unwrap() + &String::from_utf8(out_bad.stdout).unwrap();
    assert!(text_bad.contains("bounds"));

    // speed-range bounds violation should fail
    let out_speed = Command::new(bin())
        .args([
            "verify",
            path.to_str().unwrap(),
            "--speed-range",
            "2000,5000",
        ])
        .output()
        .unwrap();
    assert!(!out_speed.status.success());
    let text_speed = String::from_utf8(out_speed.stderr).unwrap()
        + &String::from_utf8(out_speed.stdout).unwrap();
    assert!(text_speed.contains("speed"));
}
