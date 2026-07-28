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
    http::StatusCode,
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
use std::sync::Arc;
use std::time::Duration;
use tokio_util::io::StreamReader;
use tower_http::limit::RequestBodyLimitLayer;

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

/// Shared server state: just the reqwest client (connection-pooled, reused across requests).
#[derive(Clone)]
pub struct AppState {
    http: reqwest::Client,
}

impl AppState {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(PROFILE_FETCH_TIMEOUT)
            .build()
            .expect("reqwest client with rustls-tls backend builds");
        AppState { http }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the axum router. Shared by `main.rs` (bound to a real socket) and the integration tests
/// (driven in-process via `tower::ServiceExt::oneshot`).
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/verify", post(verify_handler))
        // NOT `axum::extract::DefaultBodyLimit`: that only caps `Bytes`-based extractors, and
        // `verify_handler` reads the raw body itself (see step 1 below). `RequestBodyLimitLayer`
        // wraps the body unconditionally, so the cap applies no matter how it's consumed.
        .layer(RequestBodyLimitLayer::new(effective_max_body_bytes()))
        .with_state(Arc::new(state))
}

async fn healthz() -> impl IntoResponse {
    Json(serde_json::json!({ "ok": true }))
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

async fn verify_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VerifyParams>,
    request: Request,
) -> Response {
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
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Stage::EngineError,
                format!("cannot create tempfile: {e}"),
            )
        }
    };
    let write_fd = match named_file.reopen() {
        Ok(f) => f,
        Err(e) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                Stage::EngineError,
                format!("cannot reopen tempfile for write: {e}"),
            )
        }
    };
    let mut sink = tokio::fs::File::from_std(write_fd);
    let data_stream = request
        .into_body()
        .into_data_stream()
        .map_err(io::Error::other);
    let mut reader = StreamReader::new(data_stream);
    if let Err(e) = tokio::io::copy(&mut reader, &mut sink).await {
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
            return error_response(StatusCode::BAD_GATEWAY, Stage::ProfileUnavailable, message)
        }
    };

    // 3. Import + verify on a blocking thread (dry-core is synchronous, CPU-bound, and can take
    // ~1s/50MB per the spike's native-baseline numbers — keep it off the async reactor).
    let path = named_file.path().to_path_buf();
    let outcome = tokio::task::spawn_blocking(move || run_verify(&path, &profile)).await;
    drop(named_file); // deletes the tempfile now that reading is done.

    match outcome {
        Ok(Ok(report_bytes)) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            report_bytes,
        )
            .into_response(),
        Ok(Err((stage, message))) => {
            let status = match stage {
                Stage::InputInvalid => StatusCode::UNPROCESSABLE_ENTITY,
                Stage::EngineError => StatusCode::INTERNAL_SERVER_ERROR,
                Stage::ProfileUnavailable => StatusCode::BAD_GATEWAY,
            };
            error_response(status, stage, message)
        }
        Err(join_error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            Stage::EngineError,
            format!("verify worker task failed: {join_error}"),
        ),
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
fn run_verify(path: &Path, profile: &Profile) -> Result<Vec<u8>, (Stage, String)> {
    let file = std::fs::File::open(path).map_err(|e| {
        (
            Stage::EngineError,
            format!("cannot reopen tempfile for read: {e}"),
        )
    })?;

    let params = profile.gcode_import_params();

    let imported = import_gcode_reader_with_map(file, &params)
        .map_err(|e| (Stage::InputInvalid, e.to_string()))?;

    let contracts = profile.contracts();
    let report = catch_unwind(AssertUnwindSafe(|| verify(&imported.toolpath, &contracts)))
        .map_err(|_| (Stage::EngineError, "dry-core verify panicked".to_string()))?;

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| (Stage::EngineError, format!("cannot serialize report: {e}")))?;
    Ok(format!("{json}\n").into_bytes())
}
