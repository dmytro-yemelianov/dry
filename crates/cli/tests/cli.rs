//! End-to-end CLI tests: run the `dry` binary on a conformance fixture and check its output.

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn compare_json_reports_a_time_delta() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/reports/compare");
    let out = Command::new(bin())
        .args([
            "compare",
            dir.join("slow.gcode").to_str().unwrap(),
            dir.join("fast.gcode").to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let delta: Value = serde_json::from_slice(&out.stdout).expect("valid CompareDelta JSON");
    // fast is quicker → after < before on total time.
    assert!(
        delta["time"]["total"]["after"].as_f64().unwrap()
            < delta["time"]["total"]["before"].as_f64().unwrap()
    );
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dry")
}

fn fixture(corpus: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../conformance/{corpus}/{name}.json"))
}

#[cfg(feature = "moonraker")]
fn read_http_request(stream: &mut std::net::TcpStream) -> String {
    use std::io::Read as _;

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().unwrap())
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
    String::from_utf8(request).unwrap()
}

#[cfg(feature = "moonraker")]
fn write_json_response(stream: &mut std::net::TcpStream, body: &str) {
    use std::io::Write as _;

    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

#[cfg(feature = "moonraker")]
#[test]
fn upload_print_exits_nonzero_when_moonraker_does_not_start_the_job() {
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::thread;
    use std::time::{Duration, Instant};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut served = 0;
        while served < 2 && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => panic!("mock Moonraker accept failed: {error}"),
            };
            let request = read_http_request(&mut stream);
            match served {
                0 => {
                    assert!(request.starts_with("POST /server/files/upload "));
                    write_json_response(&mut stream, r#"{"item":{"path":"expected.gcode"}}"#);
                }
                1 => {
                    assert!(request.starts_with("POST /printer/print/start "));
                    assert!(request.contains(r#"{"filename":"expected.gcode"}"#));
                    write_json_response(&mut stream, r#"{"result":false}"#);
                }
                _ => unreachable!(),
            }
            served += 1;
        }
        assert_eq!(served, 2, "CLI did not make both Moonraker requests");
    });

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/vectors/minimal_line/expected.gcode");
    let out = Command::new(bin())
        .args([
            "upload",
            path.to_str().unwrap(),
            "--moonraker",
            &format!("http://{address}"),
            "--print",
            "--force",
            "--json",
        ])
        .output()
        .unwrap();
    server.join().unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr)
        .contains("Moonraker did not start printing expected.gcode"));
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
fn emit_rotary_axes_flag_and_legacy_kinematics_alias_are_equivalent() {
    // The rotary-axes selector was renamed `--kinematics` → `--rotary-axes`; the old name stays a
    // visible alias. Both must be accepted and produce byte-identical g-code for the same value.
    let path = fixture("gcode", "square");
    let run = |flag: &str| {
        let out = Command::new(bin())
            .args(["emit", path.to_str().unwrap(), flag, "ac"])
            .output()
            .unwrap();
        assert!(out.status.success(), "`dry emit {flag} ac` must succeed");
        String::from_utf8(out.stdout).unwrap()
    };
    assert_eq!(
        run("--rotary-axes"),
        run("--kinematics"),
        "`--kinematics` must remain a working alias of `--rotary-axes`"
    );
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
fn ir_command_on_raw_gcode_gives_actionable_hint() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "dry-cli-gcode-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "; a slicer file\nG1 X10 Y0 Z0.2 E0.5 F1200\nG1 X20 E0.5\n",
    )
    .unwrap();

    // IR commands handed raw G-code must fail with a hint pointing at import/review (covering both the
    // eager `load` path and the streaming `load_streaming` path), not a raw JSON parse error.
    for cmd in ["emit", "simulate", "verify"] {
        let out = Command::new(bin()).arg(cmd).arg(&path).output().unwrap();
        assert!(!out.status.success(), "{cmd} on g-code should fail");
        let err = String::from_utf8(out.stderr).unwrap();
        assert!(
            err.contains("looks like raw G-code") && err.contains("import-gcode"),
            "{cmd} error was not actionable: {err}"
        );
    }
    let _ = std::fs::remove_file(&path);
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

#[test]
fn review_gcode_max_retraction_distance_flag_fires_rule() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-retract-{}-{stamp}.gcode",
        std::process::id()
    ));
    // Relative E (M83): the `G1 E-5` is a pure retraction of 5 mm.
    std::fs::write(
        &input,
        "M83\nG1 X0 Y0 Z0.2 F1800\nG1 X10 E0.5 F1200\nG1 E-5 F1800\nG1 X20 F9000\n",
    )
    .unwrap();

    // Without the flag the 5 mm retraction is unconstrained → OK.
    let ok = Command::new(bin())
        .args(["review-gcode", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "no retraction contract → no finding: {}",
        String::from_utf8_lossy(&ok.stdout)
    );

    // With a 1 mm limit the 5 mm retraction trips the retraction-distance rule (non-zero exit).
    let bad = Command::new(bin())
        .args([
            "review-gcode",
            input.to_str().unwrap(),
            "--max-retraction-distance",
            "1",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        !bad.status.success(),
        "--max-retraction-distance 1 should fail the 5 mm retraction"
    );
    let text = String::from_utf8(bad.stderr).unwrap() + &String::from_utf8(bad.stdout).unwrap();
    assert!(
        text.contains("retraction-distance"),
        "expected a retraction-distance finding, got: {text}"
    );
}

#[test]
fn forensics_gcode_detects_slicer_and_attributes_features() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-forensics-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(
        &input,
        ";Generated with Cura_SteamEngine 5.0\nM83\n;LAYER:0\nG1 Z0.2 F600\n;TYPE:WALL-OUTER\n\
         G1 X0 Y0 F9000\nG1 X20 Y0 E0.8 F1200\n;TYPE:FILL\nG1 X2 Y2 F9000\nG1 X18 Y18 E0.6 F1800\n",
    )
    .unwrap();

    let out = Command::new(bin())
        .args(["forensics-gcode", input.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "forensics-gcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("valid forensics JSON");
    assert_eq!(report["slicer"], "Cura");
    let features: Vec<&str> = report["features"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["feature"].as_str().unwrap())
        .collect();
    assert!(features.contains(&"outer-wall"), "{features:?}");
    assert!(features.contains(&"infill"), "{features:?}");
    assert_eq!(report["features"][0]["source"], "from-comment");
}

#[test]
fn import_printer_cfg_matches_golden() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/profiles");
    let out = Command::new(bin())
        .args([
            "import-printer-cfg",
            dir.join("klipper_voron.cfg").to_str().unwrap(),
            "--name",
            "voron",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "import-printer-cfg failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let got: Value = serde_json::from_slice(&out.stdout).expect("valid profile JSON");
    let want: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("klipper_voron.expected.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(got, want, "imported profile must match the golden");
    assert_eq!(
        got["machine"]["kinematics"]["max_acceleration_mm_s2"],
        3000.0
    ); // sanity vs the fixture's max_accel
}
