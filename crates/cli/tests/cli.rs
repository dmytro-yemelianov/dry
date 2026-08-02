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
