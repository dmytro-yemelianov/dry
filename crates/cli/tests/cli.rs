//! End-to-end CLI tests: run the `dry` binary on a conformance fixture and check its output.

use dry_core::{
    import_gcode, parse_gcode_lines, GcodeImportParams, GcodeRecord, SegmentKind,
    REFERENCE_FIVE_AXIS_MACHINE,
};
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

/// A committed test-signed license token (team tier) — see `crates/license/tests/fixtures/`.
/// `upload` now refuses to run in evaluation mode, so tests that exercise it need a license.
fn team_token() -> &'static str {
    include_str!("../../license/tests/fixtures/js-signed-team.token")
}

fn fixture(corpus: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../conformance/{corpus}/{name}.json"))
}

/// The `PTP`/`LIN`/`CIRC` lines of a KRL module, in order.
///
/// Counted by prefix rather than by feeding the program to Dry's g-code parser, which is what these
/// tests used to do: a KRL module is not g-code and the parser refuses it outright. What that
/// round-trip demonstrated was only that Dry's emitter and Dry's parser agreed — see
/// `tools/krl_check.sh` for the check that involves a grammar nobody here wrote.
fn krl_motion_lines(program: &str) -> Vec<&str> {
    program
        .lines()
        .filter(|l| l.starts_with("  PTP ") || l.starts_with("  LIN ") || l.starts_with("  CIRC "))
        .collect()
}

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

fn write_json_response(stream: &mut std::net::TcpStream, body: &str) {
    use std::io::Write as _;

    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

#[test]
fn printer_search_sends_typed_graph_filters() {
    use std::net::TcpListener;
    use std::thread;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut stream);
        assert!(request.starts_with("POST /graphql "));
        assert!(request.contains(r#""text":"voron""#));
        assert!(request.contains(r#""firmware":["KLIPPER"]"#));
        assert!(request.contains(r#""material":["ABS"]"#));
        write_json_response(
            &mut stream,
            r#"{"data":{"printers":{"totalCount":1,"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[{"id":"voron","name":"Voron","vendor":"Voron Design","model":"2.4","variant":null,"kind":"PRINTER_CLASS","versions":[]}]}}}"#,
        );
    });

    let out = Command::new(bin())
        .args([
            "printer",
            "search",
            "voron",
            "--firmware",
            "klipper",
            "--material",
            "ABS",
            "--source",
            &format!("http://{address}"),
            "--json",
        ])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["totalCount"], 1);
    assert_eq!(result["nodes"][0]["id"], "voron");
}

#[test]
fn printer_resolve_downloads_and_hash_verifies_profile() {
    use sha2::{Digest, Sha256};
    use std::net::TcpListener;
    use std::thread;

    let profile = br#"{"version":1,"name":"ABS 0.4"}"#;
    let sha256 = format!("{:x}", Sha256::digest(profile));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let profile_url = format!("http://{address}/profile");
    let graph_body = serde_json::json!({
        "data": {
            "printer": {
                "versions": [{
                    "profiles": [{
                        "id": "abs-0.4",
                        "materialId": "dry:material/abs",
                        "filamentId": null,
                        "processPresetId": "abs",
                        "nozzleDiameterMm": 0.4,
                        "profileUrl": profile_url,
                        "sha256": sha256,
                    }]
                }]
            }
        }
    })
    .to_string();
    let server = thread::spawn(move || {
        let (mut graph_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut graph_stream);
        assert!(request.starts_with("POST /graphql "));
        assert!(request.contains(r#""version":"0.1.0""#));
        assert!(request.contains(r#""materialId":"dry:material/abs""#));
        write_json_response(&mut graph_stream, &graph_body);

        let (mut profile_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut profile_stream);
        assert!(request.starts_with("GET /profile "));
        use std::io::Write as _;
        write!(
            profile_stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            profile.len()
        )
        .unwrap();
        profile_stream.write_all(profile).unwrap();
    });

    let out = Command::new(bin())
        .args([
            "printer",
            "resolve",
            "voron",
            "--version",
            "0.1.0",
            "--material",
            "dry:material/abs",
            "--source",
            &format!("http://{address}"),
        ])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"{\"version\":1,\"name\":\"ABS 0.4\"}\n");
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
        .env("DRY_LICENSE", team_token().trim())
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output()
        .unwrap();
    server.join().unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr)
        .contains("Moonraker rejected the request: print start response reported failure"));
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
fn emit_five_axis_respects_profile_model_and_flag_override() {
    let path = fixture("gcode", "square");
    let profile_path = std::env::temp_dir().join(format!(
        "dry-cli-emit-profile-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &profile_path,
        r#"{
          "version": 1,
          "machine": {
            "five_axis": {
              "type": "bc",
              "pivot_offset": [0.0, 0.0, 0.0],
              "rotary_offset": [0.0, 0.0]
            }
          }
        }"#,
    )
    .unwrap();

    let from_profile = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--five-axis", "--profile"])
        .arg(profile_path.to_str().unwrap())
        .output()
        .unwrap();
    assert!(
        from_profile.status.success(),
        "dry emit --profile ... --five-axis should succeed"
    );
    let profile_out = String::from_utf8(from_profile.stdout).unwrap();

    let from_flag = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--five-axis",
            "--rotary-axes",
            "bc",
        ])
        .output()
        .unwrap();
    assert!(
        from_flag.status.success(),
        "dry emit --rotary-axes bc --five-axis should succeed"
    );
    assert_eq!(
        profile_out,
        String::from_utf8(from_flag.stdout).unwrap(),
        "profile machine.five_axis should be used when --five-axis is set"
    );

    let explicit = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--five-axis",
            "--profile",
            profile_path.to_str().unwrap(),
            "--rotary-axes",
            "ac",
        ])
        .output()
        .unwrap();
    assert!(
        explicit.status.success(),
        "explicit rotary-axes must still be accepted"
    );
    let explicit = String::from_utf8(explicit.stdout).unwrap();

    let ac_flag = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--five-axis",
            "--rotary-axes",
            "ac",
        ])
        .output()
        .unwrap();
    assert!(
        ac_flag.status.success(),
        "dry emit --rotary-axes ac --five-axis should succeed"
    );
    assert_eq!(
        explicit,
        String::from_utf8(ac_flag.stdout).unwrap(),
        "explicit --rotary-axes should override profile machine.five_axis"
    );

    let _ = std::fs::remove_file(profile_path);
}

#[test]
fn emit_five_axis_defaults_to_reference_bc_when_no_kinematics_provided() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-five-axis-default-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r#"{
          "version": 0,
          "segments": [
            {
              "start": [0.0, 0.0, 0.2],
              "end": [10.0, 0.0, 0.2],
              "travel": false,
              "speed": 1000.0,
              "length": 10.0,
              "volume": 0.5,
              "filament": 0.2,
              "kind": "line",
              "centre": null,
              "clockwise": false,
              "orientation": [1.0, 0.0, 0.0]
            }
          ]
        }"#,
    )
    .unwrap();

    let implicit = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--five-axis"])
        .output()
        .unwrap();
    assert!(
        implicit.status.success(),
        "dry emit --five-axis should default to reference machine kinematics"
    );
    let implicit = String::from_utf8(implicit.stdout).unwrap();

    let explicit_bc = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--five-axis",
            "--rotary-axes",
            "bc",
        ])
        .output()
        .unwrap();
    assert!(
        explicit_bc.status.success(),
        "dry emit --five-axis --rotary-axes bc should succeed"
    );
    let explicit_bc = String::from_utf8(explicit_bc.stdout).unwrap();

    assert_eq!(
        implicit, explicit_bc,
        "absent machine.five_axis and --rotary-axes should use the reference BC model"
    );
    assert!(
        implicit.contains("B90"),
        "reference BC model should emit B90 for +X orientation"
    );
    assert!(
        implicit.contains("C0"),
        "reference BC model should emit C0 for +X orientation"
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn emit_profile_linuxcnc_flavor_is_implicit_rs274_with_step_nc() {
    let path = fixture("vectors/five_axis", "input");
    let profile_path = std::env::temp_dir().join(format!(
        "dry-cli-emit-profile-linuxcnc-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &profile_path,
        r#"{
          "version": 1,
          "firmware": {
            "flavor": "linuxcnc"
          },
          "machine": {
            "five_axis": "bc"
          }
        }"#,
    )
    .unwrap();
    let step_nc = std::env::temp_dir().join(format!(
        "dry-cli-linuxcnc-step-nc-{}-{}.xml",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let out = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--profile",
            profile_path.to_str().unwrap(),
            "--five-axis",
            "--step-nc",
            step_nc.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "linuxcnc flavor should imply RS-274 emit (without --format)"
    );

    let gcode = String::from_utf8(out.stdout).unwrap();
    let parsed = parse_gcode_lines(&gcode).unwrap();
    let motion_count = parsed
        .iter()
        .filter(|line| matches!(line.record, GcodeRecord::Motion(_)))
        .count();
    assert_eq!(motion_count, 1);
    assert!(
        gcode.contains("G1") && !gcode.contains("LIN ") && !gcode.contains("WAIT"),
        "profile-driven RS-274 should not emit robot style words"
    );

    let imported = import_gcode(
        &gcode,
        &GcodeImportParams {
            relative_e: false,
            // `--five-axis` posts for REFERENCE_FIVE_AXIS_MACHINE, so import needs the same
            // model to read the rotary words back. Naming the constant rather than repeating
            // its literal keeps this correct if the reference machine ever changes.
            kinematics: Some(REFERENCE_FIVE_AXIS_MACHINE),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(imported.segments.len(), 1);

    let sidecar = std::fs::read_to_string(&step_nc).unwrap();
    assert!(sidecar.contains("<stepnc"));
    assert!(sidecar.contains("workingstep id=\"ws-0\""));

    let _ = std::fs::remove_file(profile_path);
    let _ = std::fs::remove_file(step_nc);
}

#[test]
fn emit_profile_robot_krl_flavor_emits_a_kuka_module() {
    let path = fixture("vectors/five_axis", "input");
    let profile_path = std::env::temp_dir().join(format!(
        "dry-cli-emit-profile-robot-krl-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &profile_path,
        r#"{
          "version": 1,
          "firmware": {
            "flavor": "robot-krl"
          },
          "machine": {
            "five_axis": "bc"
          }
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--profile",
            profile_path.to_str().unwrap(),
            "--five-axis",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "robot-krl profile should imply robot-motion emit (without --format)"
    );

    let program = String::from_utf8(out.stdout).unwrap();
    assert!(program.starts_with("DEF dry ( )\n"), "{program}");
    assert!(program.trim_end().ends_with("\nEND"), "{program}");
    assert_eq!(krl_motion_lines(&program).len(), 1, "{program}");
    assert!(
        !program.contains("G4 P") && !program.contains("G1 ") && !program.contains("G0 "),
        "robot profile should not emit RS-274 / GRBL words"
    );

    let _ = std::fs::remove_file(profile_path);
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
fn emit_grbl_flag_uses_grbl_output_mode() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-grbl-emit-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::write(
        &path,
        r#"{"version":0,"segments":[{"start":[null,null,null],"end":[null,null,null],"travel":false,"speed":1000.0,"length":0.0,"volume":0.0,"filament":0.0,"kind":"dwell","dwell_s":1.5}]}"#,
    )
    .unwrap();

    let rs274 = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--format", "rs274"])
        .output()
        .unwrap();
    assert!(
        rs274.status.success(),
        "dry emit --format rs274 should succeed"
    );

    let grbl = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--format", "grbl"])
        .output()
        .unwrap();
    assert!(
        grbl.status.success(),
        "dry emit --format grbl should succeed"
    );

    let rs274_stdout = String::from_utf8(rs274.stdout).unwrap();
    let grbl_stdout = String::from_utf8(grbl.stdout).unwrap();
    assert_eq!(
        rs274_stdout.trim(),
        "G4 S1.5",
        "rs274 output should use S dwell syntax"
    );
    assert_eq!(
        grbl_stdout.trim(),
        "G4 P1.5",
        "grbl output should use P dwell syntax"
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn emit_krl_flag_uses_krl_output_mode() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-krl-emit-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::write(
        &path,
        r#"{"version":0,"segments":[{"start":[null,null,null],"end":[10.0,null,null],"travel":true,"speed":1500.0,"length":10.0,"volume":0.0,"filament":0.0,"kind":"line","centre":null,"clockwise":false}]}"#,
    )
    .unwrap();

    let krl = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--format", "krl"])
        .output()
        .unwrap();
    assert!(krl.status.success(), "dry emit --format krl should succeed");

    // A whole module, not a bare motion line. The travel becomes a PTP, which carries no
    // `$VEL.CP` because that variable does not govern a joint move (crates/core/src/emit/krl.rs).
    assert_eq!(
        String::from_utf8(krl.stdout).unwrap(),
        concat!(
            "DEF dry ( )\n",
            ";  Emitted by dry: never run on a KUKA controller or simulator.\n",
            ";  The structure of THIS program has not been checked either -- dry emits KRL,\n",
            ";  it does not parse it. tools/krl_check.sh checks a file against an external\n",
            ";  KRL grammar that nobody here wrote.\n",
            ";  PTP speed is $VEL_AXIS[] (percent of maximum), which dry does not set.\n",
            "  $TOOL = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}\n",
            "  $BASE = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}\n",
            "  PTP {E6POS: X 10.0}\n",
            "END\n",
        ),
        "krl output should be a DEF/END module"
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn emit_krl_output_is_one_kuka_instruction_per_segment() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-krl-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r#"{
          "version": 0,
          "segments": [
            {
              "start": [null, null, null],
              "end": [20.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [20.0, 0.0, 0.0],
              "end": [20.0, 20.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [20.0, 20.0, 0.0],
              "end": [0.0, 20.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [0.0, 20.0, 0.0],
              "end": [0.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [0.0, 0.0, 0.0],
              "end": [0.0, 0.0, 0.0],
              "travel": true,
              "speed": 1200.0,
              "length": 0.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "dwell",
              "centre": null,
              "clockwise": false,
              "dwell_s": 0.5
            }
          ]
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--format", "krl"])
        .output()
        .unwrap();
    assert!(out.status.success(), "emit --format krl should succeed");

    let program = String::from_utf8(out.stdout).unwrap();
    // Four moves and a dwell. The dwell is a `WAIT SEC`, not a motion instruction, so it is
    // counted separately rather than folded into the motion count the way g-code lets it be.
    assert_eq!(krl_motion_lines(&program).len(), 4, "{program}");
    assert!(program.contains("  WAIT SEC 0.5\n"), "{program}");

    let _ = std::fs::remove_file(path);
}

#[test]
fn emit_krl_five_axis_writes_zyx_euler_angles_into_every_pose() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-krl-five-axis-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    std::fs::write(
        &path,
        r#"{
          "version": 0,
          "segments": [
            {
              "start": [null, null, null],
              "end": [10.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 10.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false,
              "orientation": [1.0, 0.0, 0.0]
            },
            {
              "start": [10.0, 0.0, 0.0],
              "end": [10.0, 10.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 10.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false,
              "orientation": [0.0, 1.0, 0.0]
            }
          ]
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--format",
            "krl",
            "--five-axis",
            "--rotary-axes",
            "bc",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "dry emit --format krl --five-axis should succeed"
    );

    let program = String::from_utf8(out.stdout).unwrap();
    // Tool axis along +X, then along +Y. KUKA A is the rotation about Z and B the tilt from +Z,
    // so the poses are (A 0, B 90) then (A 90, B 90); C is the pinned 180 deg roll. Under
    // `--five-axis` every axis the segment names is restated, exactly as the g-code renderer
    // restates them (crates/core/src/emit/gcode.rs), so the second pose repeats X and Z.
    assert_eq!(
        krl_motion_lines(&program),
        [
            "  LIN {E6POS: X 10.0, Y 0.0, Z 0.0, A 0.0, B 90.0, C 180.0}",
            "  LIN {E6POS: X 10.0, Y 10.0, Z 0.0, A 90.0}",
        ],
        "{program}"
    );

    let _ = std::fs::remove_file(path);
}

#[test]
fn emit_rs274_flag_uses_rs274_output_mode() {
    let path = fixture("gcode", "square");
    let default = Command::new(bin()).arg("emit").arg(&path).output().unwrap();
    assert!(default.status.success());

    let rs274 = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--format", "rs274"])
        .output()
        .unwrap();
    assert!(
        rs274.status.success(),
        "dry emit --format rs274 should succeed"
    );

    let rs274_out = String::from_utf8(rs274.stdout).unwrap();
    let default_out = String::from_utf8(default.stdout).unwrap();

    // RS-274 controllers have no E axis: the motion is the same, the extruder words are gone.
    assert!(
        default_out.contains('E'),
        "the FFF fixture should carry extruder words to make this test meaningful"
    );
    assert!(
        !rs274_out.contains('E'),
        "rs274 output must carry no extruder word: {rs274_out}"
    );
    let strip_e = |s: &str| {
        s.lines()
            .map(|l| {
                l.split_whitespace()
                    .filter(|t| !t.starts_with('E'))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        rs274_out.trim_end(),
        strip_e(&default_out).trim_end(),
        "rs274 motion should otherwise match the default target"
    );
}

#[test]
fn emit_rs274_output_is_parseable_and_step_nc_is_written() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-rs274-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let step_nc = std::env::temp_dir().join(format!(
        "dry-cli-rs274-step-nc-{}-{}.xml",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r#"{
          "version": 0,
          "segments": [
            {
              "start": [null, null, null],
              "end": [20.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [20.0, 0.0, 0.0],
              "end": [20.0, 20.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [20.0, 20.0, 0.0],
              "end": [0.0, 20.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [0.0, 20.0, 0.0],
              "end": [0.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [0.0, 0.0, 0.0],
              "end": [0.0, 0.0, 0.0],
              "travel": true,
              "speed": 1200.0,
              "length": 0.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "dwell",
              "centre": null,
              "clockwise": false,
              "dwell_s": 0.5
            }
          ]
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--format",
            "rs274",
            "--step-nc",
            step_nc.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "emit --format rs274 --step-nc should succeed"
    );

    let gcode = String::from_utf8(out.stdout).unwrap();
    let parsed = parse_gcode_lines(&gcode).unwrap();
    let motion_count = parsed
        .iter()
        .filter(|line| matches!(line.record, GcodeRecord::Motion(_)))
        .count();
    assert_eq!(
        motion_count, 5,
        "RS-274 emit should produce one motion record per segment"
    );
    let imported = import_gcode(
        &gcode,
        &GcodeImportParams {
            relative_e: false,
            ..GcodeImportParams::default()
        },
    )
    .unwrap();
    assert_eq!(imported.segments.len(), 5);
    assert_eq!(imported.segments.last().unwrap().kind, SegmentKind::Dwell);

    let sidecar = std::fs::read_to_string(&step_nc).unwrap();
    assert!(sidecar.contains("<stepnc"));
    assert!(sidecar.contains("workingstep id=\"ws-4\""));

    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(step_nc);
}

#[test]
fn emit_rs274_five_axis_with_step_nc_is_parseable_and_tracks_toolframe() {
    let out_path = fixture("vectors/five_axis", "input");
    let step_nc = std::env::temp_dir().join(format!(
        "dry-cli-rs274-five-axis-step-nc-{}-{}.xml",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));

    let out = Command::new(bin())
        .args([
            "emit",
            out_path.to_str().unwrap(),
            "--format",
            "rs274",
            "--five-axis",
            "--rotary-axes",
            "bc",
            "--step-nc",
            step_nc.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "emit --format rs274 --five-axis should succeed for 5-axis fixture"
    );

    let gcode = String::from_utf8(out.stdout).unwrap();
    let parsed = parse_gcode_lines(&gcode).unwrap();
    let motion_count = parsed
        .iter()
        .filter(|line| matches!(line.record, GcodeRecord::Motion(_)))
        .count();
    assert_eq!(
        motion_count, 1,
        "rs274 five-axis output should contain one motion line for the fixture"
    );

    let imported = import_gcode(
        &gcode,
        &GcodeImportParams {
            relative_e: false,
            // `--five-axis` posts for REFERENCE_FIVE_AXIS_MACHINE, so import needs the same
            // model to read the rotary words back. Naming the constant rather than repeating
            // its literal keeps this correct if the reference machine ever changes.
            kinematics: Some(REFERENCE_FIVE_AXIS_MACHINE),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(imported.segments.len(), 1);

    let sidecar = std::fs::read_to_string(&step_nc).unwrap();
    assert!(sidecar.contains("<toolframe"));
    assert!(sidecar.contains("workingstep id=\"ws-0\""));

    let _ = std::fs::remove_file(step_nc);
}

#[test]
fn emit_grbl_output_is_parseable() {
    let path = std::env::temp_dir().join(format!(
        "dry-cli-grbl-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(
        &path,
        r#"{
          "version": 0,
          "segments": [
            {
              "start": [null, null, null],
              "end": [20.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [20.0, 0.0, 0.0],
              "end": [20.0, 20.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [20.0, 20.0, 0.0],
              "end": [0.0, 20.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "volume": 0.0,
              "length": 20.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [0.0, 20.0, 0.0],
              "end": [0.0, 0.0, 0.0],
              "travel": false,
              "speed": 1200.0,
              "length": 20.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "line",
              "centre": null,
              "clockwise": false
            },
            {
              "start": [0.0, 0.0, 0.0],
              "end": [0.0, 0.0, 0.0],
              "travel": true,
              "speed": 1200.0,
              "length": 0.0,
              "volume": 0.0,
              "filament": 0.0,
              "kind": "dwell",
              "centre": null,
              "clockwise": false,
              "dwell_s": 0.5
            }
          ]
        }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--format", "grbl"])
        .output()
        .unwrap();
    assert!(out.status.success(), "emit --format grbl should succeed");

    let gcode = String::from_utf8(out.stdout).unwrap();
    let parsed = parse_gcode_lines(&gcode).unwrap();
    let motion_count = parsed
        .iter()
        .filter(|line| matches!(line.record, GcodeRecord::Motion(_)))
        .count();
    assert_eq!(
        motion_count, 5,
        "GRBL emit should produce one motion record per segment"
    );
    let imported = import_gcode(
        &gcode,
        &GcodeImportParams {
            relative_e: false,
            ..GcodeImportParams::default()
        },
    )
    .unwrap();
    assert_eq!(imported.segments.len(), 5);
    assert_eq!(imported.segments.last().unwrap().kind, SegmentKind::Dwell);

    let _ = std::fs::remove_file(path);
}

#[test]
fn emit_with_step_nc_sidecar_writes_intent_file() {
    let path = fixture("gcode", "square");
    let tmp = std::env::temp_dir().join(format!(
        "dry-step-nc-{}-{}.xml",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let out = Command::new(bin())
        .args(["emit", path.to_str().unwrap(), "--step-nc"])
        .arg(&tmp)
        .output()
        .unwrap();
    assert!(out.status.success(), "emit --step-nc should succeed");

    let step_nc = std::fs::read_to_string(&tmp).unwrap();
    assert!(step_nc.starts_with("<?xml version=\"1.0\""));
    assert!(step_nc.contains("<stepnc"));

    std::fs::remove_file(tmp).unwrap();
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
fn review_gcode_reports_unmodeled_commands_in_json() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-unmodeled-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "G1 X1\nG28 X Y\nM84\n").unwrap();

    let out = Command::new(bin())
        .args(["review-gcode", input.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert!(
        out.status.success(),
        "warnings alone remain a successful review"
    );
    let report: Value = serde_json::from_slice(&out.stdout).expect("valid ReviewReport JSON");
    let findings = report["findings"].as_array().unwrap();
    assert!(findings.iter().any(|finding| {
        finding["rule"] == "unmodeled-gcode"
            && finding["source_line"] == 2
            && finding["message"].as_str().unwrap().contains("G28")
    }));
    assert!(findings.iter().any(|finding| {
        finding["rule"] == "unmodeled-gcode"
            && finding["source_line"] == 3
            && finding["message"].as_str().unwrap().contains("M84")
    }));
}

/// Stock OrcaSlicer output for a Bambu X1C or a Prusa MK4 used to abort `review-gcode` and
/// `trace-gcode` on the vendor macros in the machine start g-code — a hard exit before any report,
/// on 4/4 measured files. Both constructs are here, with the moves they used to hide.
#[test]
fn review_and_trace_gcode_survive_vendor_macros_in_slicer_start_gcode() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-slicer-dialect-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(
        &input,
        "M1002 set_gcode_claim_speed_level : 5\n\
         M221 S; push soft endstop status\n\
         M1006 A0 B10 L100 C37 D10 M60 E37 F10 N60\n\
         M862.3 P \"MK4\" ; printer model check\n\
         M862.6 P\"Input shaper\" ; FW feature check\n\
         M115 U5.0.0-RC+11963\n\
         M83\n\
         G1 X0 Y0 F600\n\
         G1 X100 E1 F600\n",
    )
    .unwrap();

    let review = Command::new(bin())
        .args(["review-gcode", input.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    let trace = Command::new(bin())
        .args([
            "trace-gcode",
            input.to_str().unwrap(),
            "--window-s",
            "5",
            "--line-width",
            "0.45",
            "--layer-height",
            "0.2",
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);

    assert!(
        review.status.success(),
        "review-gcode failed on slicer start g-code: {}",
        String::from_utf8_lossy(&review.stderr)
    );
    assert!(
        trace.status.success(),
        "trace-gcode failed on slicer start g-code: {}",
        String::from_utf8_lossy(&trace.stderr)
    );

    let report: Value = serde_json::from_slice(&review.stdout).expect("valid ReviewReport JSON");
    // The two real moves are recovered, and `M1006 ... L100 ... E37 ...` is not one of them.
    assert_eq!(report["segments"], 2);
    let unmodeled: Vec<(u64, String)> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|finding| finding["rule"] == "unmodeled-gcode")
        .map(|finding| {
            (
                finding["source_line"].as_u64().unwrap(),
                finding["message"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let expected = [
        (1, "M1002"),
        (2, "M221"),
        (3, "M1006"),
        (4, "M862.3"),
        (5, "M862.6"),
        (6, "M115"),
    ];
    assert_eq!(unmodeled.len(), expected.len(), "{unmodeled:?}");
    for ((line, message), (want_line, want_command)) in unmodeled.iter().zip(expected) {
        assert_eq!(*line, want_line);
        assert!(
            message.starts_with(&format!("{want_command} is preserved")),
            "line {line} should be reported as {want_command}: {message}"
        );
    }

    let trace: Value = serde_json::from_slice(&trace.stdout).expect("valid trace JSON");
    assert_eq!(trace["trace"]["segment_count"], 2);
}

#[test]
#[cfg(feature = "moonraker")]
fn upload_print_without_profile_is_blocked_before_network_io() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let input = std::env::temp_dir().join(format!(
        "dry-cli-upload-gate-{}-{stamp}.gcode",
        std::process::id()
    ));
    std::fs::write(&input, "G1 X1\n").unwrap();

    let out = Command::new(bin())
        .args([
            "upload",
            input.to_str().unwrap(),
            "--moonraker",
            "http://127.0.0.1:9",
            "--print",
        ])
        .env("DRY_LICENSE", team_token().trim())
        .env("DRY_LICENSE_ALLOW_TEST_KEY", "1")
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&input);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("auto-print requires --profile"), "{stderr}");
    assert!(
        !stderr.contains("network error reaching Moonraker"),
        "gate must reject before network I/O: {stderr}"
    );
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

fn sliced_sample() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/sliced-sample.gcode")
}

#[test]
fn trace_gcode_default_format_is_unchanged() {
    // The default invocation stays byte-identical to today's: no `layers`/`analytics` keys.
    let out = Command::new(bin())
        .args(["trace-gcode", sliced_sample().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["trace"]["layers"].as_array().unwrap().len(), 0);
    assert!(json["trace"].get("analytics").is_none());
}

#[test]
fn trace_gcode_analytics_flag_adds_layers_and_analytics() {
    let out = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--window-s",
            "1",
            "--analytics",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let layers = json["trace"]["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 2, "sliced-sample.gcode has two layers");
    assert!(json["trace"]["analytics"].is_object());
    assert!(json["trace"]["analytics"]["layer_stats"]["layer_count"] == 2);
}

#[test]
fn trace_gcode_format_csv_matches_to_csv_header() {
    let out = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--format",
            "csv",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "window_index,start_time_s,end_time_s,print_time_s,travel_time_s,dwell_time_s,extruding_distance_mm,travel_distance_mm,extruded_volume_mm3,filament_mm,max_feedrate_mm_min,max_flow_mm3_s"
    );
    assert!(lines.next().is_some(), "at least one window row");
}

/// The window CSV shows no analytics, so `--analytics` beside `--format csv` must not change a byte —
/// and the CLI skips the analytics pass entirely rather than computing and discarding it.
#[test]
fn trace_gcode_format_csv_is_unchanged_by_the_analytics_flag() {
    let plain = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--format",
            "csv",
        ])
        .output()
        .unwrap();
    let with = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--format",
            "csv",
            "--analytics",
        ])
        .output()
        .unwrap();
    assert!(plain.status.success() && with.status.success());
    assert_eq!(
        plain.stdout, with.stdout,
        "--format csv output must be byte-identical with and without --analytics"
    );
}

#[test]
fn trace_gcode_rejects_a_non_positive_flow_outlier_k() {
    let out = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--analytics",
            "--flow-outlier-k",
            "0",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("flow-outlier k"), "{stderr}");
}

#[test]
fn trace_gcode_format_layers_csv_implies_analytics() {
    // No `--analytics` flag — `--format layers-csv` is the only producer of rows and implies it.
    let out = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--format",
            "layers-csv",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "layer_index,z_mm,segment_start,segment_end,print_time_s,travel_time_s,dwell_time_s,extruding_distance_mm,travel_distance_mm,extruded_volume_mm3,filament_mm,max_feedrate_mm_min,max_flow_mm3_s"
    );
    assert_eq!(lines.count(), 2, "two layer rows");
}

#[test]
fn trace_gcode_flow_outlier_k_without_analytics_is_a_usage_error() {
    let out = Command::new(bin())
        .args([
            "trace-gcode",
            sliced_sample().to_str().unwrap(),
            "--flow-outlier-k",
            "3.0",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--flow-outlier-k"));
}

/// Fixtures for the `review-batch` tests: one clean file, one gating file (bounds violation via
/// a tiny profile, mirroring `review_gcode_uses_profile_contracts_and_import_defaults`), plus a
/// path that doesn't exist.
struct BatchFixtures {
    dir: PathBuf,
    clean: PathBuf,
    gating: PathBuf,
    missing: PathBuf,
    profile: PathBuf,
}

impl BatchFixtures {
    fn new(tag: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "dry-cli-review-batch-{tag}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let clean = dir.join("clean.gcode");
        std::fs::write(&clean, "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X1 Y0 E0.05 F1200\n").unwrap();

        let gating = dir.join("gating.gcode");
        std::fs::write(&gating, "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E0.1 F1200\n").unwrap();

        let profile = dir.join("profile.json");
        std::fs::write(
            &profile,
            r#"{
              "version": 1,
              "name": "batch-bench",
              "machine": {
                "build_volume": [[0, 5], [0, 5], [0, 1]],
                "feedrate_range": [1, 5000]
              },
              "material": {
                "filament_diameter": 1.75,
                "max_volumetric_flow_mm3_s": 100
              },
              "process": {
                "line_width": 0.45,
                "layer_height": 0.2
              }
            }"#,
        )
        .unwrap();

        let missing = dir.join("does-not-exist.gcode");

        BatchFixtures {
            dir,
            clean,
            gating,
            missing,
            profile,
        }
    }
}

impl Drop for BatchFixtures {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn review_batch_exit_2_when_a_file_cannot_be_inspected() {
    let f = BatchFixtures::new("exit2");
    let out = Command::new(bin())
        .args([
            "review-batch",
            f.clean.to_str().unwrap(),
            f.gating.to_str().unwrap(),
            f.missing.to_str().unwrap(),
            "--profile",
            f.profile.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unreadable file must win over a gating one"
    );

    let batch: Value = serde_json::from_slice(&out.stdout).expect("valid ReviewBatch JSON");
    assert_eq!(batch["files_total"], 3);
    assert_eq!(batch["files_passed"], 1);
    assert_eq!(batch["files_failed"], 1);
    assert_eq!(batch["files_errored"], 1);
    let results = batch["results"].as_array().unwrap();
    assert_eq!(results.len(), 3, "the missing file did not abort the run");
    assert_eq!(results[0]["status"], "passed");
    assert!(results[0]["review"].is_object());
    assert_eq!(results[1]["status"], "failed");
    assert_eq!(results[2]["status"], "errored");
    assert!(results[2]["error"]
        .as_str()
        .unwrap()
        .contains("does-not-exist.gcode"));
    assert!(results[2]["review"].is_null());
    assert!(!batch["findings_by_rule"].as_array().unwrap().is_empty());
}

#[test]
fn review_batch_exit_1_when_every_file_is_inspected_but_one_fails() {
    let f = BatchFixtures::new("exit1");
    let out = Command::new(bin())
        .args([
            "review-batch",
            f.clean.to_str().unwrap(),
            f.gating.to_str().unwrap(),
            "--profile",
            f.profile.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let batch: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(batch["files_errored"], 0);
    assert_eq!(batch["files_failed"], 1);
}

#[test]
fn review_batch_exit_0_when_every_file_passes() {
    let f = BatchFixtures::new("exit0");
    let out = Command::new(bin())
        .args([
            "review-batch",
            f.clean.to_str().unwrap(),
            "--profile",
            f.profile.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let batch: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(batch["files_total"], 1);
    assert_eq!(batch["files_passed"], 1);
}

#[test]
fn review_batch_files_from_stdin_matches_positional() {
    let f = BatchFixtures::new("filesfrom");
    let positional = Command::new(bin())
        .args([
            "review-batch",
            f.clean.to_str().unwrap(),
            "--profile",
            f.profile.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(positional.status.success());

    let mut child = Command::new(bin())
        .args([
            "review-batch",
            "--files-from",
            "-",
            "--profile",
            f.profile.to_str().unwrap(),
            "--json",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(f.clean.to_str().unwrap().as_bytes())
            .unwrap();
    }
    let via_stdin = child.wait_with_output().unwrap();
    assert!(
        via_stdin.status.success(),
        "{}",
        String::from_utf8_lossy(&via_stdin.stderr)
    );

    let a: Value = serde_json::from_slice(&positional.stdout).unwrap();
    let b: Value = serde_json::from_slice(&via_stdin.stdout).unwrap();
    assert_eq!(a["files_total"], b["files_total"]);
    assert_eq!(a["files_passed"], b["files_passed"]);
    assert_eq!(
        a["results"][0]["review"]["metrics"],
        b["results"][0]["review"]["metrics"]
    );
}

#[test]
fn review_batch_human_output_lists_each_file_and_a_rule_breakdown() {
    let f = BatchFixtures::new("human");
    let out = Command::new(bin())
        .args([
            "review-batch",
            f.clean.to_str().unwrap(),
            f.gating.to_str().unwrap(),
            "--profile",
            f.profile.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("review-batch: 2 file(s)"), "{text}");
    assert!(text.contains("PASS"), "{text}");
    assert!(text.contains("FAIL"), "{text}");
    assert!(text.contains("by rule:"), "{text}");
    assert!(text.contains("bounds"), "{text}");
}

#[test]
fn review_batch_out_writes_json_to_file() {
    let f = BatchFixtures::new("out");
    let out_file = f.dir.join("batch.json");
    let out = Command::new(bin())
        .args([
            "review-batch",
            f.clean.to_str().unwrap(),
            "--profile",
            f.profile.to_str().unwrap(),
            "--json",
            "--out",
            out_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stdout.is_empty(), "written to --out, not stdout");
    let text = std::fs::read_to_string(&out_file).unwrap();
    let batch: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(batch["files_total"], 1);
}

/// A batch with nothing in it is a usage error, not an empty pass: `0` from `review-batch` means
/// "every file was inspected and passed", and no file is not the same claim.
#[test]
fn review_batch_with_no_files_is_a_usage_error() {
    let out = Command::new(bin()).args(["review-batch"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no files given"), "{stderr}");
}

/// An unreadable `--files-from` is a usage error (exit 2), unlike an unreadable *input* file — which
/// becomes an `errored` result and never aborts the batch. The list itself is the run's instructions:
/// without it there is no batch to report on.
#[test]
fn review_batch_unreadable_files_from_is_a_usage_error() {
    let f = BatchFixtures::new("filesfrom-missing");
    let missing_list = f.dir.join("no-such-list.txt");
    let out = Command::new(bin())
        .args([
            "review-batch",
            "--files-from",
            missing_list.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("no-such-list.txt"), "{stderr}");
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

/// NEW-2: `--reorder-travel` runs the aggressive pipeline, which *grows* the segment count (`z_hop`
/// replaces one travel with three, `coasting` splits the tail off a run). The summary line subtracted
/// two `usize`s in the reduction direction, so the command panicked with "attempt to subtract with
/// overflow" on 20 of the 28 frozen gallery designs — including this one, 401 → 1201 segments.
#[test]
fn optimize_reorder_travel_survives_a_pipeline_that_grows_the_toolpath() {
    let path = fixture("gallery", "spiral_vase");
    let out = Command::new(bin())
        .args(["optimize", "--reorder-travel", path.to_str().unwrap()])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(out.status.success(), "{stderr}");
    let summary = stderr.lines().next().unwrap_or_default();
    // the delta is signed: growth reads `+800`, not a wrapped `18446744073709550816`.
    assert!(
        summary.contains(" segments (+"),
        "a growing pipeline must report a positive delta: {summary}"
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
    assert!(text.contains("OK (no findings"), "{text}");
    // A clean run must also say what it covered, so a vacuous pass is distinguishable without
    // --json. `--bounds` is supplied here, so `bounds` is in force on top of the structural set.
    assert!(text.contains("segment(s) inspected"), "{text}");
    assert!(!text.contains("0 segment(s) inspected"), "{text}");
    assert!(text.contains("12 rule(s) in force"), "{text}");

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
fn verify_rejects_inverted_contract_ranges() {
    let path = fixture("gcode", "square");

    for (flag, value, expected) in [
        (
            "--bounds",
            "100,0,0,100,0,50",
            "bounds x lower bound must be <= upper bound",
        ),
        (
            "--speed-range",
            "9000,300",
            "speed range lower bound must be <= upper bound",
        ),
    ] {
        let output = Command::new(bin())
            .args(["verify", path.to_str().unwrap(), flag, value])
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "{flag} accepted an inverted range"
        );
        let text =
            String::from_utf8(output.stderr).unwrap() + &String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(expected), "unexpected {flag} error: {text}");
    }
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

#[test]
fn generate_pocket_emits_a_framed_rs274_program() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let pid = std::process::id();
    let ir = std::env::temp_dir().join(format!("dry-cli-generate-pocket-{pid}-{stamp}.json"));
    let profile_path = std::env::temp_dir().join(format!(
        "dry-cli-generate-pocket-profile-{pid}-{stamp}.json"
    ));
    std::fs::write(
        &profile_path,
        r#"{
        "version": 1,
        "firmware": { "flavor": "rs274" },
        "machine": { "cnc": { "tool": 1, "spindle_rpm": 10000 } }
    }"#,
    )
    .unwrap();

    let out = Command::new(bin())
        .args([
            "generate",
            "pocket",
            "--shape",
            "rect",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "60",
            "--height",
            "40",
            "--tool-diameter",
            "6",
            "--depth",
            "5",
            "--depth-per-pass",
            "2.5",
            "-o",
            ir.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "generate pocket failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new(bin())
        .args([
            "emit",
            ir.to_str().unwrap(),
            "--format",
            "rs274",
            "--profile",
            profile_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&ir);
    let _ = std::fs::remove_file(&profile_path);
    assert!(
        out.status.success(),
        "emit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let gcode = String::from_utf8(out.stdout).unwrap();
    assert!(
        gcode.starts_with("G21 G17 G90\nG54\nT1 M6\nS10000 M3\n"),
        "got head: {}",
        &gcode[..gcode.len().min(120)]
    );
    assert!(gcode.trim_end().ends_with("M30"));
    assert!(!gcode.contains(" E"), "rs274 must carry no extruder words");
    assert!(gcode.contains("G0"), "safe-Z rapids must emit as G0");
}

#[test]
fn generate_pocket_circle_profile_mode() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let ring = std::env::temp_dir().join(format!(
        "dry-cli-generate-pocket-ring-{}-{stamp}.json",
        std::process::id()
    ));

    let out = Command::new(bin())
        .args([
            "generate",
            "pocket",
            "--shape",
            "circle",
            "--cx",
            "0",
            "--cy",
            "0",
            "--radius",
            "15",
            "--tool-diameter",
            "6",
            "--depth",
            "3",
            "--cut-mode",
            "profile",
            "-o",
            ring.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "generate pocket failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new(bin())
        .args(["emit", ring.to_str().unwrap(), "--format", "rs274"])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&ring);
    assert!(
        out.status.success(),
        "emit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let gcode = String::from_utf8(out.stdout).unwrap();
    assert!(
        gcode.contains("G3") || gcode.contains("G2"),
        "circle profile must emit arcs"
    );
}

#[test]
fn generate_pocket_rejects_oversized_tool() {
    let out = Command::new(bin())
        .args([
            "generate",
            "pocket",
            "--shape",
            "rect",
            "--x",
            "0",
            "--y",
            "0",
            "--width",
            "10",
            "--height",
            "10",
            "--tool-diameter",
            "12",
            "--depth",
            "1",
            "-o",
            "/dev/null",
        ])
        .output()
        .unwrap();
    // Must fail via the generator's own validation (die(), exit 2) and carry its real message —
    // not merely exit non-zero, which a panic (101) or a clap usage error would also satisfy and
    // which would hide a regression that turned validation into an unwrap() panic.
    assert_eq!(
        out.status.code(),
        Some(2),
        "expected the generator's validation error (exit 2), got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("tool_diameter") && stderr.contains("does not fit"),
        "expected the pocket generator's oversized-tool message, got: {stderr}"
    );
}

/// A unique path under the temp dir, in the style the other CLI tests use.
fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "dry-cli-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// An IR whose *last* segment the emitter refuses: an arc with no explicit endpoint, which is a
/// full 360° circle on RS-274 rather than the no-op the missing words suggest.
fn ir_refused_at_the_last_segment() -> String {
    let good = |from: [f64; 3], to: [f64; 3]| {
        format!(
            r#"{{"start":[{},{},{}],"end":[{},{},{}],"travel":false,"speed":1200.0,"length":1.0,
                "volume":0.12,"filament":0.05,"width":0.4,"height":0.2,"kind":"line","centre":null,
                "clockwise":false,"temperature":null,"fan":null,"flow":null,"tool":null,
                "dwell_s":null,"orientation":null}}"#,
            from[0], from[1], from[2], to[0], to[1], to[2]
        )
    };
    let endpointless_arc = r#"{"start":[2.0,0.0,0.2],"end":[null,null,0.2],"travel":false,
        "speed":1200.0,"length":1.0,"volume":0.12,"filament":0.05,"width":0.4,"height":0.2,
        "kind":"arc","centre":[-1.0,0.0],"clockwise":false,"temperature":null,"fan":null,
        "flow":null,"tool":null,"dwell_s":null,"orientation":null}"#;
    format!(
        r#"{{"version":0,"segments":[{},{},{}]}}"#,
        good([0.0, 0.0, 0.2], [1.0, 0.0, 0.2]),
        good([1.0, 0.0, 0.2], [2.0, 0.0, 0.2]),
        endpointless_arc
    )
}

/// `emit_stream_to_writer` streams, and the refusal only surfaces at the *last* segment — so
/// writing straight into `--out` left a truncated but syntactically valid g-code program on disk
/// (under RS-274, one that has also lost its `M9`/`M5`/`M30` postamble) and then exited 2.
#[test]
fn emit_leaves_no_file_behind_when_the_program_is_refused() {
    let ir = temp_path("refused-ir.json");
    std::fs::write(&ir, ir_refused_at_the_last_segment()).unwrap();
    let out = temp_path("refused-out.gcode");

    let run = Command::new(bin())
        .args(["emit", ir.to_str().unwrap(), "-o", out.to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(run.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&run.stderr).contains("arc segment needs an explicit end"));
    assert!(
        !out.exists(),
        "a refused program must not leave a truncated g-code file at {out:?}"
    );
    let partial = PathBuf::from(format!("{}.dry-partial", out.to_str().unwrap()));
    assert!(
        !partial.exists(),
        "the temporary program was not cleaned up"
    );

    let _ = std::fs::remove_file(&ir);
}

/// The `.stpnc` sidecar is a machining program too: it must not be written before the g-code
/// program it describes is known to be emittable.
#[test]
fn emit_step_nc_sidecar_is_not_written_for_a_refused_program() {
    let ir = temp_path("refused-step-nc-ir.json");
    std::fs::write(&ir, ir_refused_at_the_last_segment()).unwrap();
    let out = temp_path("refused-step-nc-out.gcode");
    let sidecar = temp_path("refused-sidecar.stpnc");

    let run = Command::new(bin())
        .args([
            "emit",
            ir.to_str().unwrap(),
            "--step-nc",
            sidecar.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(run.status.code(), Some(2));
    assert!(!out.exists(), "refused program left g-code at {out:?}");
    assert!(
        !sidecar.exists(),
        "refused program left a STEP-NC intent file at {sidecar:?}"
    );
    let sidecar_partial = PathBuf::from(format!("{}.dry-partial", sidecar.to_str().unwrap()));
    assert!(
        !sidecar_partial.exists(),
        "the sidecar's own temp file was not cleaned up"
    );
    let out_partial = PathBuf::from(format!("{}.dry-partial", out.to_str().unwrap()));
    assert!(
        !out_partial.exists(),
        "the g-code temp file was not cleaned up"
    );

    let _ = std::fs::remove_file(&ir);
}

/// The sidecar is staged to its own temp file *before* the g-code emits, so a failure to commit the
/// sidecar into place — after the g-code has already been committed — must not make it look like
/// nothing was written: the g-code at `--out` is complete and real, and exit 2 must say so.
#[test]
fn emit_step_nc_sidecar_commit_failure_reports_partial_success() {
    let path = fixture("gcode", "square");
    let out = temp_path("sidecar-atomic-out.gcode");
    // A directory at the sidecar's destination path makes the final `rename` fail deterministically
    // (EISDIR) without needing to simulate disk-full, while leaving the temp-file staging untouched.
    let sidecar_dir = temp_path("sidecar-atomic-blocked.stpnc");
    std::fs::create_dir(&sidecar_dir).unwrap();

    let run = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--step-nc",
            sidecar_dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(run.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains(&format!("already written to {}", out.to_str().unwrap())),
        "exit 2 must say a complete g-code program was already written: {stderr}"
    );
    assert!(
        out.exists(),
        "the g-code that already committed must remain on disk"
    );
    let sidecar_partial = PathBuf::from(format!("{}.dry-partial", sidecar_dir.to_str().unwrap()));
    assert!(
        !sidecar_partial.exists(),
        "the sidecar temp file was not cleaned up after the failed commit"
    );
    let out_partial = PathBuf::from(format!("{}.dry-partial", out.to_str().unwrap()));
    assert!(
        !out_partial.exists(),
        "the g-code temp file was not cleaned up"
    );

    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_dir(&sidecar_dir);
}

/// The successful path still writes the whole program, sidecar included.
#[test]
fn emit_to_a_file_writes_the_whole_program() {
    let path = fixture("gcode", "square");
    let out = temp_path("emit-ok-out.gcode");
    let sidecar = temp_path("emit-ok-sidecar.stpnc");

    let run = Command::new(bin())
        .args([
            "emit",
            path.to_str().unwrap(),
            "--step-nc",
            sidecar.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let want: Vec<String> = doc["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let got: Vec<String> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(String::from)
        .collect();
    assert_eq!(
        got, want,
        "`dry emit -o` must write the same program as stdout"
    );
    assert!(std::fs::read_to_string(&sidecar)
        .unwrap()
        .contains("<stepnc"));

    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&sidecar);
}

/// The committed `conformance/slicer-corpus/` files are genuine, unmodified OrcaSlicer output — see
/// `conformance/slicer-corpus/README.md` for why they carry no correctness authority. What *is* a
/// regression claim: they must keep importing without a hard parse error, with or without their
/// matching machine profile (`docs/25-slicer-corpus-baseline.md`'s basic regression claim). This test
/// needs no slicer binary — it runs `dry review-gcode` on files already sliced and committed to disk.
#[test]
fn slicer_corpus_files_import_cleanly() {
    let corpus_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/slicer-corpus");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(corpus_dir.join("MANIFEST.json")).unwrap())
            .expect("valid slicer-corpus MANIFEST.json");
    let files = manifest["files"].as_array().unwrap();
    assert!(!files.is_empty(), "MANIFEST.json lists no committed files");

    for entry in files {
        let name = entry["file"].as_str().unwrap();
        let path = corpus_dir.join(name);
        assert!(
            path.is_file(),
            "{name} listed in MANIFEST.json but missing on disk"
        );

        let out = Command::new(bin())
            .args(["review-gcode", path.to_str().unwrap(), "--json"])
            .output()
            .unwrap();
        // Not `out.status.success()`: exit code and "imported cleanly" are different
        // claims (the profiled branch below makes the same point) -- even with no
        // profile, `review-gcode` can in principle exit non-zero on findings while
        // still having produced a well-formed report. What this test actually
        // regresses is "still produces a report" (a hard parse failure writes no
        // JSON at all), so assert on the JSON structure directly.
        let report: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "{name} failed to import with no profile: {e}\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
        assert!(
            report["findings"].is_array(),
            "{name} with no profile: no findings array in report"
        );

        if let Some(profile) = entry["matching_dry_profile"].as_str() {
            let profile_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(profile);
            let out = Command::new(bin())
                .args([
                    "review-gcode",
                    path.to_str().unwrap(),
                    "--profile",
                    profile_path.to_str().unwrap(),
                    "--json",
                ])
                .output()
                .unwrap();
            // Not `out.status.success()`: a matched profile is expected to raise error-severity
            // findings against these deliberately conservative example profiles
            // (`docs/25-slicer-corpus-baseline.md`'s profile-mismatch classification), which makes
            // `review-gcode` exit non-zero on a *successful* import. The regression claim here is
            // "still imports and produces a report" (a hard parse failure writes no JSON at all —
            // see the prior probe this corpus fixed), not "raises no findings".
            let report: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
                panic!(
                    "{name} failed to import with profile {profile}: {e}\nstderr: {}",
                    String::from_utf8_lossy(&out.stderr)
                )
            });
            assert!(
                report["findings"].is_array(),
                "{name} with profile {profile}: no findings array in report"
            );
        }
    }
}

/// `MANIFEST.json`'s `sha256` field is a provenance/integrity claim ("this is the exact byte content
/// that was reviewed and classified in `docs/25-slicer-corpus-baseline.md`") that was never actually
/// checked by any test -- a corrupted or silently re-sliced file would pass
/// `slicer_corpus_files_import_cleanly` above as long as it still imported. This guards the claim
/// directly: hash every committed file and compare against its manifest entry.
#[test]
fn slicer_corpus_manifest_sha256_matches_committed_files() {
    use sha2::{Digest, Sha256};

    let corpus_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/slicer-corpus");
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(corpus_dir.join("MANIFEST.json")).unwrap())
            .expect("valid slicer-corpus MANIFEST.json");
    let files = manifest["files"].as_array().unwrap();
    assert!(!files.is_empty(), "MANIFEST.json lists no committed files");

    for entry in files {
        let name = entry["file"].as_str().unwrap();
        let expected = entry["sha256"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: MANIFEST.json entry has no sha256 field"));
        let path = corpus_dir.join(name);
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{name} listed in MANIFEST.json but unreadable: {e}"));
        let actual = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            actual, expected,
            "{name}: committed file's sha256 does not match MANIFEST.json (file was re-sliced, \
             corrupted, or the manifest is stale -- re-freeze deliberately if the content change \
             was intended)"
        );
    }
}

/// `dry check` must not report compatibility it did not measure.
///
/// `CompatibilityReport::compatible` is false only for Error-severity findings, and the two limits
/// this subcommand advertises — `--max-feedrate` and `--max-spindle-rpm` — only ever produce
/// Warnings. So the human-readable branch printed "fully compatible with machine limits" while the
/// `--json` branch beside it listed a feedrate thirty times the machine maximum.
mod check_reports_what_it_measured {
    use super::*;

    /// A single segment far outside any sane feedrate or spindle ceiling.
    fn excessive_ir() -> PathBuf {
        // Process id as well as the clock: two tests in the same binary can read the same
        // nanosecond on a coarse timer, and a shared path makes them delete each other's fixture.
        let path = std::env::temp_dir().join(format!(
            "dry-check-{}-{}.json",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            r#"{"segments":[{"kind":"line","start":[0.0,0.0,0.2],"end":[10.0,0.0,0.2],
               "speed":90000.0,"length":10.0,"volume":1.2,"filament":0.5,"travel":false,
               "power":40000.0}]}"#,
        )
        .unwrap();
        path
    }

    #[test]
    fn warning_findings_are_reported_not_reported_as_compatible() {
        let ir = excessive_ir();
        let out = Command::new(bin())
            .args([
                "check",
                ir.to_str().unwrap(),
                "--max-feedrate",
                "3000",
                "--max-spindle-rpm",
                "12000",
            ])
            .output()
            .expect("run dry check");

        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        assert!(
            !combined.contains("fully compatible"),
            "must not claim full compatibility while holding findings:\n{combined}"
        );
        assert!(
            combined.contains("EXCEEDS_MAX_FEEDRATE"),
            "the measured feedrate violation must be visible:\n{combined}"
        );
        assert!(
            combined.contains("EXCEEDS_MAX_SPINDLE_RPM"),
            "the measured spindle violation must be visible:\n{combined}"
        );
        // Exit status is deliberately unchanged: a warning is advisory, so callers gating on the
        // exit code keep their current behaviour. What changed is that it is no longer invisible.
        assert!(out.status.success(), "a warning must not fail the gate");

        let _ = std::fs::remove_file(&ir);
    }

    #[test]
    fn the_all_clear_names_only_the_limits_actually_supplied() {
        let ir = excessive_ir();

        // With no optional limits, only the envelope is checked — saying more would be the same
        // overclaim in a quieter form.
        let out = Command::new(bin())
            .args(["check", ir.to_str().unwrap()])
            .output()
            .expect("run dry check");
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(
            stdout.contains("envelope"),
            "must name the envelope: {stdout}"
        );
        assert!(
            !stdout.contains("max feedrate") && !stdout.contains("max spindle"),
            "must not imply it checked limits nobody supplied: {stdout}"
        );

        // Supplied and satisfied: both are named.
        let out = Command::new(bin())
            .args([
                "check",
                ir.to_str().unwrap(),
                "--max-feedrate",
                "200000",
                "--max-spindle-rpm",
                "60000",
            ])
            .output()
            .expect("run dry check");
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(stdout.starts_with("OK:"), "expected an all-clear: {stdout}");
        assert!(
            stdout.contains("max feedrate") && stdout.contains("max spindle rpm"),
            "{stdout}"
        );

        let _ = std::fs::remove_file(&ir);
    }
}

/// The CLI's 5-axis fallback differs from the SDKs', deliberately, and this pins that.
///
/// `dry emit --five-axis` falls back to the reference `Bc` machine — chosen on purpose in "harden
/// 5-axis defaults and add BC fallback regression" — while `Design.gcode({fiveAxis:true})` in both
/// SDKs falls back to `Kinematics::default()`, which is `Ab`. The `five_axis_drape` conformance
/// vector states `Ab` explicitly, so the CLI's default output does NOT match that golden.
///
/// This is a real divergence and the SDK README documents it. The test exists so it cannot drift
/// further unnoticed in either direction: reconciling the defaults should fail here and force the
/// documentation to be updated with the decision.
#[test]
fn five_axis_default_diverges_from_the_ab_golden_by_design() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/vectors/five_axis_drape");
    let vector: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("vector.json")).unwrap()).unwrap();
    let declared = vector["emit_params"]["kinematics"].as_str().unwrap();
    assert!(
        declared.starts_with("Ab"),
        "vector declares {declared}; this test reasons about the Ab/Bc split"
    );

    let default_out = Command::new(bin())
        .args([
            "emit",
            dir.join("input.json").to_str().unwrap(),
            "--five-axis",
        ])
        .output()
        .expect("run dry emit");
    assert!(default_out.status.success());
    let default_gcode = String::from_utf8_lossy(&default_out.stdout)
        .trim_end()
        .to_string();
    let golden = std::fs::read_to_string(dir.join("expected.gcode"))
        .unwrap()
        .trim_end()
        .to_string();

    assert_ne!(
        default_gcode, golden,
        "if the defaults have been reconciled, update sdk/ts/README.md and delete this test"
    );

    // Asking for the SDKs' model explicitly reproduces the golden exactly, which is what makes the
    // divergence a default choice rather than an engine difference.
    let explicit = Command::new(bin())
        .args([
            "emit",
            dir.join("input.json").to_str().unwrap(),
            "--five-axis",
            "--rotary-axes",
            "ab",
        ])
        .output()
        .expect("run dry emit --rotary-axes ab");
    assert_eq!(
        String::from_utf8_lossy(&explicit.stdout).trim_end(),
        golden,
        "the engine agrees across front-ends once the model is stated explicitly"
    );
}
