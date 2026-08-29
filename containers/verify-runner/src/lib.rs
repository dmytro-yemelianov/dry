//! Native `dry-core` verify shim — the compute engine cloud verify jobs dispatch to (the Worker,
//! Task R3, calls `POST /verify` on this container; see `docs/superpowers/plans/` for the wider
//! cloud MVP shape).
//!
//! # Byte-identity invariant
//!
//! A cloud verify report must be byte-identical to local `dry import-gcode <file> --profile <p> -o
//! <ir>` followed by `dry verify <ir> --profile <p> --json` for the same profile+input — i.e. the
//! **plain** import path (`gcode_import_params` in `crates/cli/src/main.rs:1779-1798`), NOT the
//! `review-gcode` path (`gcode_review_params`, `crates/cli/src/main.rs:1800-1810`), which forces
//! `line_width`/`layer_height` to `0.45`/`0.2` when the profile omits them. This crate does not
//! implement its own verification logic — it mirrors the *exact* `dry-core` call sequence
//! `Cmd::ImportGcode` + `Cmd::Verify` use in composition: `import_gcode_reader_with_map` with
//! plain, unforced import defaults, then the raw `verify()` call, then
//! `serde_json::to_string_pretty(&report) + "\n"` — the same bytes `Cmd::Verify` in
//! `crates/cli/src/main.rs` prints for `--json`. No logic is duplicated from `crates/cli` or
//! `crates/core`; both are used as published dependencies (`dry-core` is a path dependency;
//! nothing in `crates/cli` or `crates/core` was modified for this task). `tests/handler.rs`
//! enforces this by shelling out to the real, compiled `dry` binary and byte-comparing its stdout
//! against this crate's HTTP response for the same profile+gcode — see
//! `verify_report_is_byte_identical_to_the_real_cli` and its "profile omits process defaults"
//! sibling test, which pins exactly the divergence point the old forced-defaults code masked.
//!
//! # Profile-selection decision
//!
//! `docs/19-printer-registry-api.md` documents the REST artifact route as
//! `GET /v1/profiles/{printer-id}/{version}/{profile-id}` — but a pack version can carry more than
//! one resolved profile (one per material/nozzle combination; see the GraphQL `profiles(materialId:
//! ..., nozzleDiameterMm: ...)` field in that doc). The doc does not say how a caller that only has
//! `pack`+`version` (no GraphQL client) should pick which `profile-id` to fetch — resolving that
//! requires the GraphQL search a full registry client would do, which is out of scope for this
//! container. **Decision:** `POST /verify` takes an explicit `profile=<profileId>` query parameter
//! in addition to `pack`/`version`/`registry`; the runner does zero profile resolution of its own
//! and fetches exactly `GET {registry}/v1/profiles/{pack}/{version}/{profile}`. Task R3 (the Worker)
//! is expected to resolve the `profile-id` (via its own registry client) and pass it through
//! unchanged.

use axum::{
    extract::{Query, Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use dry_core::{import_gcode_reader_with_map, verify, Profile};
use futures_util::TryStreamExt;
use serde::Deserialize;
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::io::StreamReader;
use tower_http::limit::RequestBodyLimitLayer;

use std::collections::HashMap;
use std::sync::Mutex;

/// Header name used for end-to-end distributed request tracing.
pub static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

/// In-memory sliding-window rate limiter per client key.
#[derive(Default, Debug)]
pub struct RateLimiter {
    clients: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn check_and_record(&self, key: &str, limit_per_minute: usize, now: Instant) -> bool {
        let mut map = self.clients.lock().unwrap_or_else(|p| p.into_inner());
        let timestamps = map.entry(key.to_string()).or_default();
        let window = Duration::from_secs(60);
        timestamps.retain(|&t| now.duration_since(t) < window);
        if timestamps.len() >= limit_per_minute {
            false
        } else {
            timestamps.push(now);
            true
        }
    }
}

/// Production public keys trusted by this runner.
pub const PRODUCTION_KEYS: &[(&str, [u8; 32])] = &[
    (
        "prod-1",
        [
            0x4c, 0x0b, 0x77, 0xdc, 0x2f, 0x2d, 0xb6, 0x9f, 0xc5, 0xdf, 0xb5, 0xef, 0xf8, 0x41, 0x60,
            0x76, 0xfd, 0x5c, 0xd0, 0xfa, 0x69, 0x3b, 0x24, 0x3a, 0x31, 0x59, 0x66, 0x03, 0x5f, 0x37,
            0x7b, 0xcd,
        ],
    ),
    (
        "dry-prod-2026-08",
        [
            0x4c, 0x0b, 0x77, 0xdc, 0x2f, 0x2d, 0xb6, 0x9f, 0xc5, 0xdf, 0xb5, 0xef, 0xf8, 0x41, 0x60,
            0x76, 0xfd, 0x5c, 0xd0, 0xfa, 0x69, 0x3b, 0x24, 0x3a, 0x31, 0x59, 0x66, 0x03, 0x5f, 0x37,
            0x7b, 0xcd,
        ],
    ),
];

/// The verification keys accepted: production keys always, plus test key in debug/opt-in mode.
pub fn license_keys() -> Vec<(&'static str, [u8; 32])> {
    let mut keys: Vec<(&'static str, [u8; 32])> = PRODUCTION_KEYS.to_vec();
    let allow_test = cfg!(test)
        || (cfg!(debug_assertions)
            && std::env::var("DRY_LICENSE_ALLOW_TEST_KEY").is_ok_and(|v| v == "1"));
    if allow_test {
        keys.push((dry_license::TEST_KEY_ID, dry_license::TEST_VERIFYING_KEY));
    }
    keys
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Server telemetry and runtime metrics tracker.
#[derive(Default, Debug)]
pub struct ServerMetrics {
    pub requests_total: AtomicU64,
    pub requests_success: AtomicU64,
    pub requests_error: AtomicU64,
    pub requests_error_profile_unavailable: AtomicU64,
    pub requests_error_input_invalid: AtomicU64,
    pub requests_error_engine_error: AtomicU64,
    pub requests_error_unauthorized: AtomicU64,
    pub requests_error_rate_limited: AtomicU64,
    pub segments_inspected_total: AtomicU64,
    pub active_requests: AtomicU64,
    pub request_duration_ms_total: AtomicU64,
}

/// Worker enforces a 100MB `Content-Length` cap upstream; this is deliberate headroom, not the
/// product limit (see the task's Global Constraints).
pub const MAX_BODY_BYTES: usize = 200 * 1024 * 1024;

/// The env var that overrides [`MAX_BODY_BYTES`], read once at router-build time (see [`app`]).
/// Lets a test install a tiny cap and assert the over-limit behaviour without recompiling.
const MAX_BODY_BYTES_ENV: &str = "MAX_BODY_BYTES";

/// Effective body-size cap: [`MAX_BODY_BYTES_ENV`] if set and parseable, else [`MAX_BODY_BYTES`].
fn effective_max_body_bytes() -> usize {
    std::env::var(MAX_BODY_BYTES_ENV)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(MAX_BODY_BYTES)
}

/// Timeout for the profile fetch against the registry.
const PROFILE_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// The only registry host the runner will fetch a profile from — fail closed when unset (see
/// [`fetch_profile`]).
const ALLOWED_REGISTRY_HOST_ENV: &str = "ALLOWED_REGISTRY_HOST";

/// Shared server state: connection-pooled reqwest client, rate limiter, and server metrics.
#[derive(Clone)]
pub struct AppState {
    http: reqwest::Client,
    metrics: Arc<ServerMetrics>,
    rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(PROFILE_FETCH_TIMEOUT)
            .build()
            .expect("reqwest client with rustls-tls backend builds");
        AppState {
            http,
            metrics: Arc::new(ServerMetrics::default()),
            rate_limiter: Arc::new(RateLimiter::default()),
        }
    }

    pub fn metrics(&self) -> &ServerMetrics {
        &self.metrics
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Middleware that extracts or assigns an `X-Request-ID` and attaches it to response headers.
async fn request_id_middleware(request: Request, next: axum::middleware::Next) -> Response {
    let req_id = request
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|val| val.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            static COUNTER: AtomicU64 = AtomicU64::new(1);
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros())
                .unwrap_or(0);
            let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
            format!("req-{ts:x}-{seq:x}")
        });

    let header_val = HeaderValue::from_str(&req_id).unwrap_or_else(|_| HeaderValue::from_static("unknown"));

    let mut response = next.run(request).await;
    response.headers_mut().insert(X_REQUEST_ID.clone(), header_val);
    response
}

/// Build the axum router. Shared by `main.rs` (bound to a real socket) and the integration tests
/// (driven in-process via `tower::ServiceExt::oneshot`).
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics_handler))
        .route("/verify", post(verify_handler))
        // NOT `axum::extract::DefaultBodyLimit`: that only caps `Bytes`-based extractors, and
        // `verify_handler` reads the raw body itself (see step 1 below). `RequestBodyLimitLayer`
        // wraps the body unconditionally, so the cap applies no matter how it's consumed.
        .layer(RequestBodyLimitLayer::new(effective_max_body_bytes()))
        // Fix round 2: `RequestBodyLimitLayer` itself short-circuits BEFORE `verify_handler` ever
        // runs whenever the request already carries a `Content-Length` header over the cap (it
        // reads that header directly — see `RequestBodyLimit::call` in tower-http's
        // `limit/service.rs`) and returns a bare `413 Payload Too Large` / plain-text "length limit
        // exceeded" body, bypassing our `{error, stage}` envelope entirely. This layer is added
        // AFTER (so it wraps, i.e. sits OUTSIDE) `RequestBodyLimitLayer` — it observes that 413 on
        // the way back out and rewrites it into the same 422 `input-invalid` envelope the
        // streaming-overrun path already uses (the `tokio::io::copy` error arm in `verify_handler`).
        // Any other status passes through unchanged.
        .layer(axum::middleware::map_response(map_body_limit_response))
        .layer(axum::middleware::from_fn(request_id_middleware))
        .with_state(Arc::new(state))
}

/// Rewrites a bare `413 Payload Too Large` from `RequestBodyLimitLayer` (upstream in the layer
/// stack — see [`app`]) into the contract's `422 {"error", "stage": "input-invalid"}` envelope.
/// Every other status is returned unchanged: nothing else in this router's stack ever produces a
/// 413 on its own, so this only ever fires for the body-limit layer's upfront rejection.
async fn map_body_limit_response(response: Response) -> Response {
    if response.status() != StatusCode::PAYLOAD_TOO_LARGE {
        return response;
    }
    error_response(
        StatusCode::UNPROCESSABLE_ENTITY,
        Stage::InputInvalid,
        "request body exceeds the configured limit",
    )
}

/// Liveness probe: returns basic status ok.
async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
}

/// Readiness probe: reports whether the runner is configured and ready to accept verification requests.
async fn readyz() -> impl IntoResponse {
    let allowed_host = std::env::var(ALLOWED_REGISTRY_HOST_ENV).ok();
    let is_ready = allowed_host.is_some();
    Json(serde_json::json!({
        "ready": is_ready,
        "allowed_registry_host": allowed_host,
        "max_body_bytes": effective_max_body_bytes(),
        "service": "dry-verify-runner",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Prometheus telemetry metrics handler.
async fn metrics_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let total = state.metrics.requests_total.load(Ordering::Relaxed);
    let success = state.metrics.requests_success.load(Ordering::Relaxed);
    let error = state.metrics.requests_error.load(Ordering::Relaxed);
    let err_profile = state.metrics.requests_error_profile_unavailable.load(Ordering::Relaxed);
    let err_input = state.metrics.requests_error_input_invalid.load(Ordering::Relaxed);
    let err_engine = state.metrics.requests_error_engine_error.load(Ordering::Relaxed);
    let err_auth = state.metrics.requests_error_unauthorized.load(Ordering::Relaxed);
    let err_ratelimit = state.metrics.requests_error_rate_limited.load(Ordering::Relaxed);
    let active = state.metrics.active_requests.load(Ordering::Relaxed);
    let segments = state.metrics.segments_inspected_total.load(Ordering::Relaxed);
    let duration_ms = state.metrics.request_duration_ms_total.load(Ordering::Relaxed);

    let prometheus_text = format!(
        "# HELP dry_verify_requests_total Total number of verify requests processed\n\
         # TYPE dry_verify_requests_total counter\n\
         dry_verify_requests_total{{status=\"total\"}} {total}\n\
         dry_verify_requests_total{{status=\"success\"}} {success}\n\
         dry_verify_requests_total{{status=\"error\"}} {error}\n\
         # HELP dry_verify_errors_total Total errors partitioned by rejection reason\n\
         # TYPE dry_verify_errors_total counter\n\
         dry_verify_errors_total{{stage=\"profile_unavailable\"}} {err_profile}\n\
         dry_verify_errors_total{{stage=\"input_invalid\"}} {err_input}\n\
         dry_verify_errors_total{{stage=\"engine_error\"}} {err_engine}\n\
         dry_verify_errors_total{{stage=\"unauthorized\"}} {err_auth}\n\
         dry_verify_errors_total{{stage=\"rate_limited\"}} {err_ratelimit}\n\
         # HELP dry_verify_active_requests Current active in-flight requests\n\
         # TYPE dry_verify_active_requests gauge\n\
         dry_verify_active_requests {active}\n\
         # HELP dry_verify_segments_inspected_total Total G-code segments verified\n\
         # TYPE dry_verify_segments_inspected_total counter\n\
         dry_verify_segments_inspected_total {segments}\n\
         # HELP dry_verify_duration_ms_total Cumulative verification execution duration in ms\n\
         # TYPE dry_verify_duration_ms_total counter\n\
         dry_verify_duration_ms_total {duration_ms}\n"
    );

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        prometheus_text,
    )
}

#[derive(Debug, Deserialize)]
pub struct VerifyParams {
    pub pack: String,
    pub version: String,
    pub profile: String,
    pub registry: String,
}

/// The three error stages the task's Global Constraints define. Any other failure (e.g. a
/// malformed/missing query parameter) falls back to axum's default extractor rejection, which is
/// intentionally NOT one of these three stages.
#[derive(Debug)]
enum Stage {
    ProfileUnavailable,
    InputInvalid,
    EngineError,
}

impl Stage {
    fn as_str(&self) -> &'static str {
        match self {
            Stage::ProfileUnavailable => "profile-unavailable",
            Stage::InputInvalid => "input-invalid",
            Stage::EngineError => "engine-error",
        }
    }
}

fn error_response(status: StatusCode, stage: Stage, message: impl std::fmt::Display) -> Response {
    let body = serde_json::json!({
        "error": message.to_string(),
        "stage": stage.as_str(),
    });
    (status, Json(body)).into_response()
}

/// RAII guard for the temporary g-code file uploaded during verification.
/// Ensures the file is unlinked immediately when dropped, even on error or panic.
pub struct EphemeralGcodeFile {
    inner: Option<tempfile::NamedTempFile>,
}

impl EphemeralGcodeFile {
    pub fn new(file: tempfile::NamedTempFile) -> Self {
        Self { inner: Some(file) }
    }

    pub fn path(&self) -> Option<&Path> {
        self.inner.as_ref().map(|f| f.path())
    }

    pub fn close(mut self) {
        if let Some(f) = self.inner.take() {
            let path = f.path().to_path_buf();
            let _ = f.close();
            tracing::debug!(path = %path.display(), "ephemeral g-code file securely unlinked from disk");
        }
    }
}

impl Drop for EphemeralGcodeFile {
    fn drop(&mut self) {
        if let Some(f) = self.inner.take() {
            let path = f.path().to_path_buf();
            let _ = f.close();
            tracing::debug!(path = %path.display(), "ephemeral g-code file dropped and unlinked");
        }
    }
}

async fn verify_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VerifyParams>,
    request: Request,
) -> Response {
    let start_time = Instant::now();
    state.metrics.requests_total.fetch_add(1, Ordering::Relaxed);
    state.metrics.active_requests.fetch_add(1, Ordering::Relaxed);

    struct ActiveGuard(Arc<ServerMetrics>);
    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            self.0.active_requests.fetch_sub(1, Ordering::Relaxed);
        }
    }
    let _active_guard = ActiveGuard(state.metrics.clone());

    // 0. Authenticate & Resolve License Mode
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let revoked_env = std::env::var("DRY_LICENSE_REVOKED_IDS").unwrap_or_default();
    let revoked_ids: Vec<&str> = revoked_env
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let (license_stamp, client_key, is_licensed, rpm_limit) = match auth_header {
        Some(header) if header.starts_with("Bearer ") || header.starts_with("bearer ") => {
            let token = header[7..].trim();
            let keys = license_keys();
            match dry_license::verify_token_with_revocation(token, &keys, now_unix(), &revoked_ids) {
                Ok(verified) => match verified.state {
                    dry_license::LicenseState::Expired => {
                        state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
                        state.metrics.requests_error_unauthorized.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(licensee = %verified.payload.licensee, "license expired past grace period");
                        return error_response(
                            StatusCode::UNAUTHORIZED,
                            Stage::InputInvalid,
                            format!("license for {} expired and is past grace period", verified.payload.licensee),
                        );
                    }
                    _ => {
                        let stamp = dry_core::LicenseStamp {
                            mode: "licensed".to_string(),
                            licensee: Some(verified.payload.licensee.clone()),
                            tier: Some(verified.payload.tier.to_string()),
                        };
                        let key = format!("lic:{}", verified.payload.licensee);
                        let limit = verified.payload.tier.rate_limit_per_minute();
                        (Some(stamp), key, true, limit)
                    }
                },
                Err(err) => {
                    state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
                    state.metrics.requests_error_unauthorized.fetch_add(1, Ordering::Relaxed);
                    tracing::warn!(error = %err, "invalid or revoked license token");
                    return error_response(
                        StatusCode::UNAUTHORIZED,
                        Stage::InputInvalid,
                        format!("invalid license token: {err}"),
                    );
                }
            }
        }
        Some(_) => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            state.metrics.requests_error_unauthorized.fetch_add(1, Ordering::Relaxed);
            return error_response(
                StatusCode::UNAUTHORIZED,
                Stage::InputInvalid,
                "malformed Authorization header; expected 'Bearer <token>'",
            );
        }
        None => {
            // Unauthenticated / Evaluation Mode
            (None, "anonymous".to_string(), false, 120)
        }
    };

    // 0.1 Rate Limiting Check
    if !state.rate_limiter.check_and_record(&client_key, rpm_limit, Instant::now()) {
        state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
        state.metrics.requests_error_rate_limited.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(client = %client_key, limit = rpm_limit, "rate limit exceeded");
        let mut resp = error_response(
            StatusCode::TOO_MANY_REQUESTS,
            Stage::InputInvalid,
            "rate limit exceeded; try again later",
        );
        resp.headers_mut().insert(
            axum::http::header::RETRY_AFTER,
            axum::http::HeaderValue::from_static("60"),
        );
        return resp;
    }

    tracing::info!(
        pack = %params.pack,
        version = %params.version,
        profile = %params.profile,
        licensed = is_licensed,
        "starting verify request"
    );

    // 1. Stream the raw g-code body to a tempfile under /tmp — never buffer the whole upload in
    // one `Vec<u8>`. The spike (docs/superpowers/specs/2026-07-28-cloud-spike-findings.md) found
    // that holding the raw body in memory *in addition to* dry-core's own ~43-50x import blowup is
    // exactly what exhausted a 128MB Workers isolate at 10-50MB inputs; this container has 6GiB, so
    // the import-time blowup itself is fine, but there is no reason to pay the extra body-copy cost
    // when a streaming write avoids it for free.
    let named_file = match tempfile::Builder::new()
        .prefix("dry-verify-")
        .tempfile_in("/tmp")
    {
        Ok(f) => f,
        Err(e) => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            tracing::error!(error = %e, "cannot create tempfile");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Stage::EngineError,
                format!("cannot create tempfile: {e}"),
            );
        }
    };
    let write_fd = match named_file.reopen() {
        Ok(f) => f,
        Err(e) => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            tracing::error!(error = %e, "cannot reopen tempfile for write");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Stage::EngineError,
                format!("cannot reopen tempfile for write: {e}"),
            );
        }
    };
    let mut sink = tokio::fs::File::from_std(write_fd);
    let data_stream = request
        .into_body()
        .into_data_stream()
        .map_err(io::Error::other);
    let mut reader = StreamReader::new(data_stream);
    if let Err(e) = tokio::io::copy(&mut reader, &mut sink).await {
        state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
        state.metrics.requests_error_input_invalid.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(error = %e, "failed reading request body");
        // Covers both a genuinely malformed/truncated stream AND `RequestBodyLimitLayer` rejecting
        // an over-cap body (surfaced here as a stream read error, not a separate rejection type) —
        // both are `input-invalid`, so both get the same 422 every other `Stage::InputInvalid`
        // response uses (see the status-code match in `verify_handler`'s outcome handling below).
        return error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            Stage::InputInvalid,
            format!("failed reading request body: {e}"),
        );
    }
    drop(sink);

    let ephemeral = EphemeralGcodeFile::new(named_file);

    // 2. Fetch the resolved profile from the registry (see the module doc for the profile=<id>
    // decision). Any failure — network error, non-2xx, unparseable body — is profile-unavailable.
    let profile_url = format!(
        "{}/v1/profiles/{}/{}/{}",
        params.registry.trim_end_matches('/'),
        params.pack,
        params.version,
        params.profile
    );
    let profile = match fetch_profile(&state.http, &profile_url).await {
        Ok(profile) => profile,
        Err(message) => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            state.metrics.requests_error_profile_unavailable.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(url = %profile_url, error = %message, "failed to fetch profile");
            return error_response(StatusCode::BAD_GATEWAY, Stage::ProfileUnavailable, message);
        }
    };

    // 3. Import + verify on a blocking thread (dry-core is synchronous, CPU-bound, and can take
    // ~1s/50MB per the spike's native-baseline numbers — keep it off the async reactor).
    let path = match ephemeral.path() {
        Some(p) => p.to_path_buf(),
        None => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            state.metrics.requests_error_engine_error.fetch_add(1, Ordering::Relaxed);
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Stage::EngineError,
                "ephemeral tempfile path missing",
            );
        }
    };
    let outcome = tokio::task::spawn_blocking(move || run_verify(&path, &profile, license_stamp)).await;
    ephemeral.close(); // Explicitly delete the ephemeral file as soon as reading finishes.

    match outcome {
        Ok(Ok((report_bytes, segment_count))) => {
            let elapsed = start_time.elapsed();
            state.metrics.requests_success.fetch_add(1, Ordering::Relaxed);
            state
                .metrics
                .segments_inspected_total
                .fetch_add(segment_count as u64, Ordering::Relaxed);
            state
                .metrics
                .request_duration_ms_total
                .fetch_add(elapsed.as_millis() as u64, Ordering::Relaxed);

            tracing::info!(
                pack = %params.pack,
                version = %params.version,
                profile = %params.profile,
                segments_inspected = segment_count,
                duration_ms = elapsed.as_millis(),
                "verify request completed successfully"
            );

            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                report_bytes,
            )
                .into_response()
        }
        Ok(Err((stage, message))) => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            match stage {
                Stage::InputInvalid => {
                    state.metrics.requests_error_input_invalid.fetch_add(1, Ordering::Relaxed);
                }
                Stage::EngineError => {
                    state.metrics.requests_error_engine_error.fetch_add(1, Ordering::Relaxed);
                }
                Stage::ProfileUnavailable => {
                    state.metrics.requests_error_profile_unavailable.fetch_add(1, Ordering::Relaxed);
                }
            }
            let status = match stage {
                Stage::InputInvalid => StatusCode::UNPROCESSABLE_ENTITY,
                Stage::EngineError => StatusCode::INTERNAL_SERVER_ERROR,
                Stage::ProfileUnavailable => StatusCode::BAD_GATEWAY,
            };
            tracing::warn!(
                stage = stage.as_str(),
                status = status.as_u16(),
                error = %message,
                "verify request failed"
            );
            error_response(status, stage, message)
        }
        Err(join_error) => {
            state.metrics.requests_error.fetch_add(1, Ordering::Relaxed);
            state.metrics.requests_error_engine_error.fetch_add(1, Ordering::Relaxed);
            tracing::error!(error = %join_error, "blocking verify task panicked");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Stage::EngineError,
                "blocking verify task panicked",
            )
        }
    }
}

/// SSRF guard: the runner will only ever fetch a profile from a single operator-configured
/// registry host. `ALLOWED_REGISTRY_HOST` is required — unset means refuse every fetch (fail
/// closed), not "allow anything". The registry base URL must be `https://` and its host must equal
/// `ALLOWED_REGISTRY_HOST` exactly, EXCEPT `http://` is additionally allowed when the host is
/// `127.0.0.1` or `localhost` — a deliberate dev/test escape hatch so a local stub registry (which
/// doesn't terminate TLS) can still be exercised without weakening the production rule (any other
/// host must be `https`).
fn validate_registry_url(url: &str) -> Result<(), String> {
    let allowed_host = std::env::var(ALLOWED_REGISTRY_HOST_ENV).map_err(|_| {
        format!(
            "{ALLOWED_REGISTRY_HOST_ENV} is not configured; refusing all registry fetches \
             (fail closed)"
        )
    })?;

    let parsed =
        reqwest::Url::parse(url).map_err(|e| format!("invalid registry URL {url}: {e}"))?;
    let host = parsed.host_str().unwrap_or("");
    if host != allowed_host {
        return Err(format!(
            "registry host {host:?} is not the allowed registry host {allowed_host:?}"
        ));
    }

    let is_loopback_host = host == "127.0.0.1" || host == "localhost";
    match parsed.scheme() {
        "https" => Ok(()),
        "http" if is_loopback_host => Ok(()),
        scheme => Err(format!(
            "registry URL scheme {scheme:?} is not allowed for host {host:?} (https is required; \
             http is only permitted for 127.0.0.1/localhost)"
        )),
    }
}

async fn fetch_profile(client: &reqwest::Client, url: &str) -> Result<Profile, String> {
    validate_registry_url(url)?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("registry request to {url} failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("registry returned {} for {url}", response.status()));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("reading profile response body: {e}"))?;
    // TODO(deferred): sha256 verification per docs/19 — the registry's artifact route documents a
    // sha256 alongside each resolved profile; this runner does not yet fetch or check it (see
    // `.superpowers/sdd/task-R2-report.md`'s "Concerns" section).
    Profile::from_json(&text).map_err(|e| format!("invalid profile from {url}: {e}"))
}

/// Import the g-code at `path` and verify it against `profile`'s contracts, returning the exact
/// bytes `dry verify --json` would print for the resulting report:
/// `serde_json::to_string_pretty(&report) + "\n"`.
///
/// Mirrors the **plain** `dry import-gcode --profile <p> -o <ir>` then `dry verify <ir> --profile
/// <p> --json` composition exactly: `gcode_import_params` in `crates/cli/src/main.rs:1779-1798`
/// (== `profile.gcode_import_params()` with no CLI-flag overrides — there are none to apply here,
/// since the registry always resolves a profile), NOT `gcode_review_params`
/// (`crates/cli/src/main.rs:1800-1810`), which forces `line_width`/`layer_height` to `0.45`/`0.2`
/// when the profile omits them. Absent profile process fields (`process.line_width`,
/// `process.layer_height`) are left `None` here exactly as they are in that CLI composition — no
/// forced defaults.
fn run_verify(
    path: &Path,
    profile: &Profile,
    license_stamp: Option<dry_core::LicenseStamp>,
) -> Result<(Vec<u8>, usize), (Stage, String)> {
    let file = std::fs::File::open(path).map_err(|e| {
        (
            Stage::EngineError,
            format!("cannot reopen tempfile for read: {e}"),
        )
    })?;

    let params = profile.gcode_import_params();

    let imported_res = catch_unwind(AssertUnwindSafe(|| import_gcode_reader_with_map(file, &params)))
        .map_err(|_| (Stage::EngineError, "dry-core import panicked".to_string()))?;
    let imported = imported_res.map_err(|e| (Stage::InputInvalid, e.to_string()))?;

    let segment_count = imported.toolpath.segments.len();
    let contracts = profile.contracts();
    let mut report = catch_unwind(AssertUnwindSafe(|| verify(&imported.toolpath, &contracts)))
        .map_err(|_| (Stage::EngineError, "dry-core verify panicked".to_string()))?;

    report.license = license_stamp.or_else(|| {
        Some(dry_core::LicenseStamp {
            mode: "evaluation".to_string(),
            licensee: None,
            tier: None,
        })
    });

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| (Stage::EngineError, format!("cannot serialize report: {e}")))?;
    Ok((format!("{json}\n").into_bytes(), segment_count))
}
