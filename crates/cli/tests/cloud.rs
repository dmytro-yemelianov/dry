//! End-to-end tests for the opt-in Dry Cloud CLI surface. Every network call
//! terminates at a per-test localhost `TcpListener`; existing offline commands
//! must remain network-independent.

use serde_json::json;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dry")
}

fn fixture(corpus: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../conformance/{corpus}/{name}.json"))
}

struct TempConfig {
    path: PathBuf,
}

impl TempConfig {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dry-cloud-test-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn token_path(&self) -> PathBuf {
        self.path.join("dry").join("cloud-token")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(bin());
        command
            .env("XDG_CONFIG_HOME", &self.path)
            .env_remove("DRY_TOKEN")
            .env_remove("DRY_CLOUD_URL");
        command
    }

    fn write_token(&self, token: &str) {
        let path = self.token_path();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("{token}\n")).unwrap();
    }
}

impl Drop for TempConfig {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
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
    request
}

fn request_headers(request: &[u8]) -> String {
    let header_end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    String::from_utf8_lossy(&request[..header_end]).into_owned()
}

fn request_body(request: &[u8]) -> &[u8] {
    let header_end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap();
    &request[header_end + 4..]
}

fn write_json_response(stream: &mut TcpStream, status: &str, body: &serde_json::Value) {
    let body = body.to_string();
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
}

fn cloud_origin(listener: &TcpListener) -> String {
    format!("http://{}", listener.local_addr().unwrap())
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn auth_login_completes_device_flow_and_stores_token_with_private_permissions() {
    let config = TempConfig::new("login");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = cloud_origin(&listener);
    let server_origin = origin.clone();
    let server = thread::spawn(move || {
        let (mut device_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut device_stream);
        assert!(request_headers(&request).starts_with("POST /v1/auth/device HTTP/1.1"));
        write_json_response(
            &mut device_stream,
            "200 OK",
            &json!({
                "device_code": "device-1",
                "user_code": "ABCD-EFGH",
                "verification_uri": format!("{server_origin}/activate"),
                "verification_uri_complete": format!("{server_origin}/activate?user_code=ABCD-EFGH"),
                "expires_in": 60,
                "interval": 0
            }),
        );

        let (mut token_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut token_stream);
        let headers = request_headers(&request);
        assert!(headers.starts_with("POST /v1/auth/token HTTP/1.1"));
        let body = String::from_utf8_lossy(request_body(&request));
        assert!(body.contains("device_code=device-1"));
        assert!(body.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
        write_json_response(
            &mut token_stream,
            "200 OK",
            &json!({"access_token":"dry-access-token","token_type":"Bearer"}),
        );
    });

    let output = config
        .command()
        .args(["auth", "login", "--cloud-url", &origin])
        .output()
        .unwrap();
    server.join().unwrap();
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Code: ABCD-EFGH"));
    assert!(stdout.contains("/activate?user_code=ABCD-EFGH"));
    assert_eq!(
        fs::read_to_string(config.token_path()).unwrap(),
        "dry-access-token\n"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(config.token_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn auth_login_honors_slow_down_before_polling_again() {
    let config = TempConfig::new("slow-down");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = cloud_origin(&listener);
    let server_origin = origin.clone();
    let server = thread::spawn(move || {
        let (mut device_stream, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut device_stream);
        write_json_response(
            &mut device_stream,
            "200 OK",
            &json!({
                "device_code": "device-slow",
                "user_code": "SLOW-DOWN",
                "verification_uri": format!("{server_origin}/activate"),
                "verification_uri_complete": format!("{server_origin}/activate?user_code=SLOW-DOWN"),
                "expires_in": 30,
                "interval": 0
            }),
        );

        let (mut slow_stream, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut slow_stream);
        write_json_response(
            &mut slow_stream,
            "400 Bad Request",
            &json!({"error":"slow_down"}),
        );

        let before_second_poll = Instant::now();
        let (mut token_stream, _) = listener.accept().unwrap();
        let elapsed = before_second_poll.elapsed();
        assert!(
            elapsed >= Duration::from_millis(4_800),
            "second poll arrived too soon after slow_down: {elapsed:?}"
        );
        let _ = read_http_request(&mut token_stream);
        write_json_response(
            &mut token_stream,
            "200 OK",
            &json!({"access_token":"slow-token","token_type":"Bearer"}),
        );
    });

    let output = config
        .command()
        .args(["auth", "login", "--cloud-url", &origin])
        .output()
        .unwrap();
    server.join().unwrap();
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(config.token_path()).unwrap(),
        "slow-token\n"
    );
}

#[test]
fn auth_status_prefers_dry_token_and_prints_account_usage() {
    let config = TempConfig::new("status");
    config.write_token("file-token");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = cloud_origin(&listener);
    let server = thread::spawn(move || {
        let (mut me_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut me_stream);
        let headers = request_headers(&request);
        assert!(headers.starts_with("GET /v1/me HTTP/1.1"));
        assert!(headers.contains("Authorization: Bearer env-token"));
        write_json_response(
            &mut me_stream,
            "200 OK",
            &json!({
                "account_id":"account-1",
                "email":"maker@example.com",
                "created_at":"2026-07-28 12:00:00"
            }),
        );

        let (mut usage_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut usage_stream);
        let headers = request_headers(&request);
        assert!(headers.starts_with("GET /v1/usage HTTP/1.1"));
        assert!(headers.contains("Authorization: Bearer env-token"));
        write_json_response(
            &mut usage_stream,
            "200 OK",
            &json!({
                "month":{"jobs":3,"bytes":12345},
                "quotas":{"jobs_per_month":20,"keys":1}
            }),
        );
    });

    let output = config
        .command()
        .env("DRY_TOKEN", "env-token")
        .env("DRY_CLOUD_URL", &origin)
        .args(["auth", "status"])
        .output()
        .unwrap();
    server.join().unwrap();
    assert_success(&output);
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("maker@example.com (account-1) — 3/20 jobs, 12345 bytes this month"));
}

#[test]
fn auth_logout_removes_the_token_without_network_io() {
    let config = TempConfig::new("logout");
    config.write_token("file-token");

    let output = config
        .command()
        .env("DRY_CLOUD_URL", "http://127.0.0.1:1")
        .args(["auth", "logout"])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(!config.token_path().exists());
}

#[test]
fn cloud_verify_without_pack_version_submits_polls_and_exits_one_for_error_findings() {
    let config = TempConfig::new("verify");
    config.write_token("cloud-token");
    let gcode = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/reports/compare/fast.gcode");
    let expected_gcode = fs::read(&gcode).unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = cloud_origin(&listener);
    let server = thread::spawn(move || {
        let (mut submit_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut submit_stream);
        let headers = request_headers(&request);
        assert!(headers.starts_with("POST /v1/jobs/verify?pack=dry%3Aprinter%2Fvoron HTTP/1.1"));
        assert!(headers.contains("Authorization: Bearer cloud-token"));
        assert_eq!(request_body(&request), expected_gcode);
        write_json_response(
            &mut submit_stream,
            "202 Accepted",
            &json!({"id":"job-1","status_url":"/v1/jobs/job-1"}),
        );

        let (mut status_stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut status_stream);
        let headers = request_headers(&request);
        assert!(headers.starts_with("GET /v1/jobs/job-1 HTTP/1.1"));
        assert!(headers.contains("Authorization: Bearer cloud-token"));
        write_json_response(
            &mut status_stream,
            "200 OK",
            &json!({"id":"job-1","status":"queued"}),
        );

        let before_second_poll = Instant::now();
        let (mut status_stream, _) = listener.accept().unwrap();
        assert!(
            before_second_poll.elapsed() >= Duration::from_millis(900),
            "job was polled again without the one-second initial backoff"
        );
        let request = read_http_request(&mut status_stream);
        let headers = request_headers(&request);
        assert!(headers.starts_with("GET /v1/jobs/job-1 HTTP/1.1"));
        assert!(headers.contains("Authorization: Bearer cloud-token"));
        write_json_response(
            &mut status_stream,
            "200 OK",
            &json!({
                "id":"job-1",
                "status":"done",
                "report":{
                    "findings":[{
                        "rule":"bounds",
                        "severity":"error",
                        "segment":0,
                        "message":"outside build volume"
                    }]
                }
            }),
        );
    });

    let output = config
        .command()
        .env("DRY_CLOUD_URL", &origin)
        .args([
            "cloud",
            "verify",
            gcode.to_str().unwrap(),
            "--printer",
            "dry:printer/voron",
            "--json",
        ])
        .output()
        .unwrap();
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["findings"][0]["severity"], "error");
}

#[test]
fn cloud_command_without_token_has_the_actionable_exit_two_error() {
    let config = TempConfig::new("no-token");
    let gcode = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/reports/compare/fast.gcode");
    let output = config
        .command()
        .args([
            "cloud",
            "verify",
            gcode.to_str().unwrap(),
            "--printer",
            "demo",
            "--pack-version",
            "0.1.0",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "error: not logged in — run `dry auth login`\n"
    );
}

#[test]
fn local_verify_stays_offline_when_cloud_url_is_unreachable() {
    let config = TempConfig::new("offline");
    let output = config
        .command()
        .env("DRY_CLOUD_URL", "http://127.0.0.1:1")
        .args([
            "verify",
            fixture("vectors/minimal_line", "input").to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["findings"].is_array());
}

#[test]
fn auth_login_cloud_url_flag_overrides_dry_cloud_url() {
    let config = TempConfig::new("url-precedence");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = cloud_origin(&listener);
    let server_origin = origin.clone();
    let server = thread::spawn(move || {
        let (mut device_stream, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut device_stream);
        write_json_response(
            &mut device_stream,
            "200 OK",
            &json!({
                "device_code":"device-url",
                "user_code":"URL-TEST",
                "verification_uri":format!("{server_origin}/activate"),
                "verification_uri_complete":format!("{server_origin}/activate?user_code=URL-TEST"),
                "expires_in":60,
                "interval":0
            }),
        );
        let (mut token_stream, _) = listener.accept().unwrap();
        let _ = read_http_request(&mut token_stream);
        write_json_response(
            &mut token_stream,
            "200 OK",
            &json!({"access_token":"url-token","token_type":"Bearer"}),
        );
    });

    let output = config
        .command()
        .env("DRY_CLOUD_URL", "http://127.0.0.1:1")
        .args(["auth", "login", "--cloud-url", &origin])
        .output()
        .unwrap();
    server.join().unwrap();
    assert_success(&output);
}
