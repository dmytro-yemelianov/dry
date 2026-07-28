//! Handler-level tests for the verify-runner axum app, driven in-process via
//! `tower::ServiceExt::oneshot` (no real socket for the runner itself). The registry it fetches
//! profiles from IS a real stub server on a `std::net::TcpListener` thread — the same pattern
//! `crates/cli/tests/cli.rs:86-92` uses to mock Moonraker/the printer registry. All network in this
//! file stays on localhost, per the task's Global Constraints.
//!
//! # Env-var tests
//!
//! Two runtime behaviours (`ALLOWED_REGISTRY_HOST`, `MAX_BODY_BYTES`) are configured via process
//! env vars read at request/router-build time. Env vars are process-global, not per-thread, and
//! `cargo test` runs test functions concurrently on separate OS threads by default, so every test
//! that reads or writes either var goes through `EnvVarGuard`, which serializes on a shared
//! `Mutex` for its entire lifetime (including across `.await` points) and restores the prior value
//! (or absence) on drop. Without this, tests would flip each other's env vars mid-flight.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, Once};
use std::thread;
use tower::ServiceExt as _;
use verify_runner::{app, AppState};

fn conformance_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance")
        .join(relative)
}

/// A real, non-trivial profile from the conformance fixture matrix (not a hand-rolled test double)
/// — `marlin` firmware so `relative_e` stays false, matching the fixture gcode's absolute E values.
/// Its `process.line_width`/`process.layer_height` happen to equal the OLD forced-default values
/// (0.45/0.2), which is exactly why this profile alone could not catch the forced-defaults bug —
/// see `fixture_profile_path_without_process_defaults` below for the profile that does.
fn fixture_profile_path() -> PathBuf {
    conformance_path("profile-matrix/marlin-pla-i3/profile.json")
}

fn fixture_profile_text() -> String {
    std::fs::read_to_string(fixture_profile_path()).unwrap()
}

/// A copy of `fixture_profile_path()` with `process.line_width`/`process.layer_height` stripped —
/// pins the previously-masked divergence point: the old runner forced these to `0.45`/`0.2` when a
/// profile omitted them, silently diverging from the real `dry import-gcode` -> `dry verify --json`
/// composition, which leaves them `None` (unwidth/unheighted beads, different `Bead` findings).
fn fixture_profile_path_without_process_defaults() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/marlin-pla-i3-no-process-defaults.json")
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

// --- env-var test isolation --------------------------------------------------------------------

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Serializes access to process env vars across the whole test binary (env vars are process-
/// global, and `cargo test` runs tests concurrently on separate threads) and restores each
/// variable's prior value — or absence — on drop. Held for a test's entire body, including
/// `.await` points, so no other test's request can observe a var mid-flip.
///
/// A single guard can carry MULTIPLE variable changes (see `apply`) — `ENV_LOCK` is a plain,
/// non-reentrant `Mutex`, so a test that needs two vars set must go through one `apply`/`set_many`
/// call, not two separate `set` calls: calling `set` twice on the same thread would try to lock
/// `ENV_LOCK` a second time while the first guard still holds it and deadlock the entire suite
/// (every other test blocks forever waiting for a lock that thread will never release).
struct EnvVarGuard {
    _lock: MutexGuard<'static, ()>,
    restores: Vec<(&'static str, Option<String>)>,
}

impl EnvVarGuard {
    /// Sets `key` to `value` for the guard's lifetime.
    fn set(key: &'static str, value: &str) -> Self {
        Self::apply(&[(key, Some(value))])
    }

    /// Ensures `key` is ABSENT for the guard's lifetime — the "fail closed on unset" case.
    fn unset(key: &'static str) -> Self {
        Self::apply(&[(key, None)])
    }

    /// Applies every `(key, value)` change in `changes` under ONE `ENV_LOCK` acquisition —
    /// `Some(value)` sets it, `None` ensures it's absent — and restores all of them (in reverse
    /// order) on drop.
    fn apply(changes: &[(&'static str, Option<&str>)]) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let mut restores = Vec::with_capacity(changes.len());
        for &(key, value) in changes {
            restores.push((key, std::env::var(key).ok()));
            // SAFETY: `ENV_LOCK` serializes every env-var-touching test in this binary; no other
            // thread reads or writes process env vars while this guard is alive. (`set_var`/
            // `remove_var` became `unsafe fn` in Rust 1.82 because mutating the environment is
            // process-global, not thread-local — exactly the hazard this guard exists to rule out.)
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
        EnvVarGuard {
            _lock: lock,
            restores,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        for (key, previous) in self.restores.drain(..).rev() {
            // SAFETY: see `apply` above — still under `_lock`.
            match previous {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

/// The host every test's stub registry binds to; tests that need registry access to succeed set
/// `ALLOWED_REGISTRY_HOST` to this value.
const LOOPBACK_HOST: &str = "127.0.0.1";

// --- real-CLI byte-identity harness --------------------------------------------------------------

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cli_binary_path() -> PathBuf {
    repo_root().join("target/debug/dry")
}

static BUILD_CLI: Once = Once::new();

/// Builds the real `dry` CLI binary exactly once per test-binary run (a `cargo build -p dry-cli`
/// subprocess against the main engine workspace at the repo root — NOT this crate's own standalone
/// workspace). Subsequent calls from other tests are no-ops. This is what makes the byte-identity
/// tests below an honest end-to-end check instead of comparing the runner against itself.
fn ensure_cli_built() {
    BUILD_CLI.call_once(|| {
        let status = Command::new("cargo")
            .args(["build", "-p", "dry-cli", "--quiet"])
            .current_dir(repo_root())
            .status()
            .expect("failed to invoke `cargo build -p dry-cli`");
        assert!(status.success(), "cargo build -p dry-cli failed");
    });
    assert!(
        cli_binary_path().is_file(),
        "expected the dry CLI binary at {:?} after `cargo build -p dry-cli`",
        cli_binary_path()
    );
}

/// Runs the REAL, compiled `dry` binary as two subprocesses in a tempdir — `dry import-gcode
/// <gcode> --profile <profile> -o <ir>` then `dry verify <ir> --profile <profile> --json` — and
/// returns the second command's stdout bytes: the literal ground truth for "byte-identical to
/// local `dry verify --json`" (see `crates/cli/src/main.rs`'s `Cmd::ImportGcode`/`Cmd::Verify`
/// arms). No crates/cli or crates/core logic is reimplemented here — this just shells out.
fn real_cli_verify_json(gcode_path: &Path, profile_path: &Path) -> Vec<u8> {
    ensure_cli_built();
    let dir = tempfile::tempdir().unwrap();
    let ir_path = dir.path().join("ir.json");

    let import_status = Command::new(cli_binary_path())
        .arg("import-gcode")
        .arg(gcode_path)
        .arg("--profile")
        .arg(profile_path)
        .arg("-o")
        .arg(&ir_path)
        .status()
        .expect("failed to run `dry import-gcode`");
    assert!(import_status.success(), "dry import-gcode failed");

    let verify_output = Command::new(cli_binary_path())
        .arg("verify")
        .arg(&ir_path)
        .arg("--profile")
        .arg(profile_path)
        .arg("--json")
        .output()
        .expect("failed to run `dry verify --json`");
    // NOT `verify_output.status.success()`: `dry verify` deliberately exits `1` (not `0`) when the
    // report has findings (see `Cmd::Verify`'s `if report.ok() { SUCCESS } else { ExitCode::from(1)
    // }` in `crates/cli/src/main.rs`) — that is the expected, correct outcome for our fixtures
    // (e.g. a `cold-extrusion` finding), not a subprocess failure. A genuine crash/misuse instead
    // produces empty/malformed stdout, which the caller's byte-comparison assertion will catch.
    assert!(
        !verify_output.stdout.is_empty(),
        "dry verify --json produced no stdout (stderr: {})",
        String::from_utf8_lossy(&verify_output.stderr)
    );
    verify_output.stdout
}

/// Sends `gcode` + `profile_text` through the full runner HTTP stack (stub registry -> query
/// parsing -> tempfile streaming -> import/verify -> response encoding) and returns the response
/// status and raw body bytes.
async fn run_via_handler(gcode: Vec<u8>, profile_text: String) -> (StatusCode, axum::body::Bytes) {
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
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    server.join().unwrap();
    (status, bytes)
}

// --- tests ---------------------------------------------------------------------------------------

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
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
    let profile_text = fixture_profile_text();
    let gcode = std::fs::read(fixture_gcode_path()).unwrap();

    let (status, bytes) = run_via_handler(gcode, profile_text).await;
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(status, StatusCode::OK, "response body: {json}");
    assert!(
        json["findings"].is_array(),
        "expected a Report shape with `findings`: {json}"
    );
}

/// Fix 1: byte-identity is enforced against the REAL, compiled CLI binary — `dry import-gcode`
/// piped into `dry verify --json` — not against the runner's own `run_verify` function (the old
/// self-referential test could never catch a bug in `run_verify` itself, since it was comparing
/// that function to itself).
#[tokio::test]
async fn verify_report_is_byte_identical_to_the_real_cli() {
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
    let profile_path = fixture_profile_path();
    let profile_text = fixture_profile_text();
    let gcode_path = fixture_gcode_path();
    let gcode = std::fs::read(&gcode_path).unwrap();

    let (status, runner_bytes) = run_via_handler(gcode, profile_text).await;
    assert_eq!(status, StatusCode::OK);

    let reference_bytes = real_cli_verify_json(&gcode_path, &profile_path);

    assert_eq!(
        runner_bytes.as_ref(),
        reference_bytes.as_slice(),
        "cloud verify report must be byte-identical to `dry import-gcode` -> `dry verify --json`"
    );
}

/// The second, previously-impossible-to-catch byte-identity case: a profile that OMITS
/// `process.line_width`/`process.layer_height`. The old runner forced these to `0.45`/`0.2`
/// (mirroring `gcode_review_params`, the REVIEW path's forced defaults) even though the real
/// `dry import-gcode` -> `dry verify --json` composition (the plain `gcode_import_params` path)
/// leaves them `None`. Every other fixture profile in this repo happens to already set both
/// fields to exactly those forced-default values, so this divergence was invisible without a
/// profile fixture that omits them.
#[tokio::test]
async fn verify_report_is_byte_identical_to_the_real_cli_when_profile_omits_process_defaults() {
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
    let profile_path = fixture_profile_path_without_process_defaults();
    let profile_text = std::fs::read_to_string(&profile_path).unwrap();
    let gcode_path = fixture_gcode_path();
    let gcode = std::fs::read(&gcode_path).unwrap();

    let (status, runner_bytes) = run_via_handler(gcode, profile_text).await;
    assert_eq!(status, StatusCode::OK);

    let reference_bytes = real_cli_verify_json(&gcode_path, &profile_path);

    assert_eq!(
        runner_bytes.as_ref(),
        reference_bytes.as_slice(),
        "must stay byte-identical to the real CLI even when the profile omits \
         process.line_width/process.layer_height"
    );
}

#[tokio::test]
async fn verify_profile_404_returns_502_profile_unavailable() {
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
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
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
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
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
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

/// Fix 2: the body-stream/limit path must return the same 422 `input-invalid` envelope every other
/// `Stage::InputInvalid` failure uses, not a bare 400. `MAX_BODY_BYTES` is overridden via env var
/// (read at router-build time in `app()`) so this test can install a tiny cap without recompiling.
#[tokio::test]
async fn verify_body_over_configured_limit_returns_422_input_invalid() {
    // Only `MAX_BODY_BYTES` needs setting — the body-limit error fires before the profile fetch,
    // so `ALLOWED_REGISTRY_HOST` is never consulted here. (Also: `EnvVarGuard::apply` is how a test
    // would set both at once — see its doc comment for why two separate `set` calls would
    // deadlock.)
    let _cap = EnvVarGuard::set("MAX_BODY_BYTES", "8");

    let oversized_body = vec![b'G'; 4096];
    let response = app(AppState::new())
        .oneshot(verify_request("http://127.0.0.1:1", oversized_body))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["stage"], "input-invalid");
    assert!(json["error"].is_string());
}

/// Fix 3 (SSRF allowlist): a registry host that isn't `ALLOWED_REGISTRY_HOST` is refused before any
/// network call is attempted. `example.invalid` is an RFC 2606 reserved, guaranteed-unresolvable
/// hostname — if the runner ever tried to actually connect, this test would hang/timeout instead of
/// returning promptly, so a fast 502 here is itself evidence the check runs before the request.
#[tokio::test]
async fn verify_disallowed_registry_host_returns_502_profile_unavailable() {
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", LOOPBACK_HOST);
    let gcode = std::fs::read(fixture_gcode_path()).unwrap();

    let response = app(AppState::new())
        .oneshot(verify_request("http://example.invalid", gcode))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["stage"], "profile-unavailable");
    assert!(json["error"].is_string());
}

/// Fix 3 (SSRF allowlist): `http://` is only permitted for `127.0.0.1`/`localhost` (the dev/test
/// escape hatch); any other allowed host must be fetched over `https://`. Sets
/// `ALLOWED_REGISTRY_HOST` to a non-loopback name so the host check passes and the scheme check is
/// what's actually being exercised.
#[tokio::test]
async fn verify_http_scheme_for_non_loopback_host_returns_502_profile_unavailable() {
    let _allow = EnvVarGuard::set("ALLOWED_REGISTRY_HOST", "example.invalid");
    let gcode = std::fs::read(fixture_gcode_path()).unwrap();

    let response = app(AppState::new())
        .oneshot(verify_request("http://example.invalid", gcode))
        .await
        .unwrap();
    let status = response.status();
    let json = response_json(response).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["stage"], "profile-unavailable");
    assert!(json["error"].is_string());
}

/// Fix 3 (SSRF allowlist): no default fallback to accepting everything — an unset
/// `ALLOWED_REGISTRY_HOST` refuses ALL registry fetches, even a real, reachable, otherwise-valid
/// stub registry on loopback. The stub server here is never actually contacted (the allowlist
/// check runs before the network call), so its background thread is left parked in `accept()` and
/// intentionally not joined — it is dropped when the test process exits.
#[tokio::test]
async fn verify_missing_allowed_registry_host_env_returns_502_profile_unavailable() {
    let _allow = EnvVarGuard::unset("ALLOWED_REGISTRY_HOST");
    let profile_text = fixture_profile_text();
    let gcode = std::fs::read(fixture_gcode_path()).unwrap();
    let (registry, _server) = spawn_stub_registry(
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

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json["stage"], "profile-unavailable");
    assert!(json["error"].is_string());
}
