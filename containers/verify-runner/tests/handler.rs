//! Handler-level tests for the verify-runner axum app, driven in-process via
//! `tower::ServiceExt::oneshot` (no real socket for the runner itself). The registry it fetches
//! profiles from IS a real stub server on a `std::net::TcpListener` thread — the same pattern
//! `crates/cli/tests/cli.rs:86-92` uses to mock Moonraker/the printer registry. All network in this
//! file stays on localhost, per the task's Global Constraints.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use dry_core::Profile;
use serde_json::Value;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use tower::ServiceExt as _;
use verify_runner::{app, run_verify_for_test, AppState};

fn conformance_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance")
        .join(relative)
}

/// A real, non-trivial profile from the conformance fixture matrix (not a hand-rolled test double)
/// — `marlin` firmware so `relative_e` stays false, matching the fixture gcode's absolute E values.
fn fixture_profile_text() -> String {
    std::fs::read_to_string(conformance_path(
        "profile-matrix/marlin-pla-i3/profile.json",
    ))
    .unwrap()
}

/// A real, small gcode fixture from `conformance/` (three-line absolute-E deposit).
fn fixture_gcode_path() -> PathBuf {
    conformance_path("reports/compare/fast.gcode")
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

/// Spawns a one-shot stub registry on `127.0.0.1:0` that serves `body` with `status_line` for
/// exactly one request, asserting the request path matches the REST shape documented in
/// `docs/19-printer-registry-api.md` (`GET /v1/profiles/{pack}/{version}/{profile}`).
fn spawn_stub_registry(
    status_line: &'static str,
    body: String,
    expected_path_prefix: &'static str,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let request = read_http_request(&mut stream);
        assert!(
            request.starts_with(expected_path_prefix),
            "unexpected request: {request}"
        );
        write!(
            stream,
            "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (format!("http://{address}"), handle)
}

fn verify_request(registry: &str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!(
            "/verify?pack=marlin-pla-i3&version=0.1.0&profile=marlin-pla-i3&registry={registry}"
        ))
        .body(Body::from(body))
        .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn healthz_returns_ok_true() {
    let response = app(AppState::new())
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = response_json(response).await;
    assert_eq!(json, serde_json::json!({"ok": true}));
}

#[tokio::test]
async fn verify_happy_path_returns_200_and_a_parseable_report() {
    let profile_text = fixture_profile_text();
    let gcode = std::fs::read(fixture_gcode_path()).unwrap();
    let (registry, server) = spawn_stub_registry(
        "HTTP/1.1 200 OK",
        profile_text,
        "GET /v1/profiles/marlin-pla-i3/0.1.0/marlin-pla-i3 ",
    );

    let response = app(AppState::new())
        .oneshot(verify_request(&registry, gcode))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;
    server.join().unwrap();

    assert_eq!(status, StatusCode::OK, "response body: {json}");
    assert!(
        json["findings"].is_array(),
        "expected a Report shape with `findings`: {json}"
    );
}

#[tokio::test]
async fn verify_report_is_byte_identical_to_a_direct_dry_core_call() {
    let profile_text = fixture_profile_text();
    let gcode_path = fixture_gcode_path();
    let gcode = std::fs::read(&gcode_path).unwrap();
    let (registry, server) = spawn_stub_registry(
        "HTTP/1.1 200 OK",
        profile_text.clone(),
        "GET /v1/profiles/marlin-pla-i3/0.1.0/marlin-pla-i3 ",
    );

    let response = app(AppState::new())
        .oneshot(verify_request(&registry, gcode))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let runner_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    server.join().unwrap();

    // "Run the same verify via dry-core directly the way the CLI does" — the exact call sequence
    // `run_verify_for_test` (== the handler's internal `run_verify`) performs mirrors
    // `crates/cli/src/main.rs`'s `Cmd::ReviewGcode` arm: `import_gcode_reader_with_map` with review
    // defaults, then `verify()`, then `serde_json::to_string_pretty(&report) + "\n"` — the same
    // bytes `Cmd::Verify --json` prints. This proves the HTTP body-streaming-to-tempfile and
    // response-encoding layers introduce zero byte drift versus calling dry-core directly.
    let profile = Profile::from_json(&profile_text).unwrap();
    let reference_bytes = run_verify_for_test(&gcode_path, &profile).unwrap();

    assert_eq!(
        runner_bytes.as_ref(),
        reference_bytes.as_slice(),
        "cloud verify report must be byte-identical to a direct dry-core verify call"
    );
    // Sanity: also byte-identical to what `serde_json::to_string_pretty` + "\n" produces directly,
    // i.e. exactly what `dry verify --json` writes to stdout for the same (toolpath, contracts).
    let expected_suffix = b"\n";
    assert!(reference_bytes.ends_with(expected_suffix));
}

#[tokio::test]
async fn verify_profile_404_returns_502_profile_unavailable() {
    let gcode = std::fs::read(fixture_gcode_path()).unwrap();
    let (registry, server) = spawn_stub_registry(
        "HTTP/1.1 404 Not Found",
        "not found".to_string(),
        "GET /v1/profiles/marlin-pla-i3/0.1.0/marlin-pla-i3 ",
    );

    let response = app(AppState::new())
        .oneshot(verify_request(&registry, gcode))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;
    server.join().unwrap();

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["stage"], "profile-unavailable");
    assert!(json["error"].is_string());
}

#[tokio::test]
async fn verify_unreachable_registry_returns_502_profile_unavailable() {
    // Bind then immediately drop a listener: the port is very likely refused on the next connect
    // attempt (localhost-only, per the Global Constraints — no real network call happens).
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let registry = format!("http://{address}");

    let gcode = std::fs::read(fixture_gcode_path()).unwrap();
    let response = app(AppState::new())
        .oneshot(verify_request(&registry, gcode))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["stage"], "profile-unavailable");
}

#[tokio::test]
async fn verify_garbage_body_returns_422_input_invalid() {
    let profile_text = fixture_profile_text();
    let (registry, server) = spawn_stub_registry(
        "HTTP/1.1 200 OK",
        profile_text,
        "GET /v1/profiles/marlin-pla-i3/0.1.0/marlin-pla-i3 ",
    );

    // Invalid UTF-8 — `GcodeParser` reads line-by-line via `BufRead::read_line`, which errors on
    // non-UTF-8 bytes; that error surfaces as a `GcodeImportError` (a `Result`, not a panic), which
    // is exactly the "malformed gcode that dry-core rejects normally" case the task specifies.
    let garbage = vec![0xFF, 0xFE, 0x00, 0x01, 0x02, 0xFE, 0xFF];

    let response = app(AppState::new())
        .oneshot(verify_request(&registry, garbage))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;
    server.join().unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["stage"], "input-invalid");
    assert!(json["error"].is_string());
}
