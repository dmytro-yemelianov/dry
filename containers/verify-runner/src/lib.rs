//! Native `dry-core` verify shim — the compute engine cloud verify jobs dispatch to (the Worker,
//! Task R3, calls `POST /verify` on this container; see `docs/superpowers/plans/` for the wider
//! cloud MVP shape).
//!
//! # Byte-identity invariant
//!
//! A cloud verify report must be byte-identical to local `dry verify --json` for the same
//! profile+input. This crate does not implement its own verification logic — it mirrors the
//! *exact* `dry-core` call sequence the CLI's `review-gcode` path uses (see
//! `crates/cli/src/main.rs`'s `Cmd::ReviewGcode` arm and `crates/cloud/src/lib.rs`'s spike, which
//! documents the same sequence): `import_gcode_reader_with_map` with review-mode import defaults,
//! then the raw `verify()` call, then `serde_json::to_string_pretty(&report) + "\n"` — the same
//! bytes `Cmd::Verify` in `crates/cli/src/main.rs` prints for `--json`. No logic is duplicated from
//! `crates/cli` or `crates/core`; both are used as published dependencies (`dry-core` is a path
//! dependency; nothing in `crates/cli` or `crates/core` was modified for this task).
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
    extract::{DefaultBodyLimit, Query, Request, State},
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

/// Worker enforces a 100MB `Content-Length` cap upstream; this is deliberate headroom, not the
/// product limit (see the task's Global Constraints).
pub const MAX_BODY_BYTES: usize = 200 * 1024 * 1024;

/// Timeout for the profile fetch against the registry.
const PROFILE_FETCH_TIMEOUT: Duration = Duration::from_secs(10);

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
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
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
        return error_response(
            StatusCode::BAD_REQUEST,
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

async fn fetch_profile(client: &reqwest::Client, url: &str) -> Result<Profile, String> {
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
    Profile::from_json(&text).map_err(|e| format!("invalid profile from {url}: {e}"))
}

/// Import the g-code at `path` and verify it against `profile`'s contracts, returning the exact
/// bytes `dry verify --json` would print for the resulting report:
/// `serde_json::to_string_pretty(&report) + "\n"`.
///
/// Mirrors `crates/cli/src/main.rs`'s `gcode_review_params` + `Cmd::ReviewGcode`'s
/// `import_gcode_reader_with_map` + `verify()` call sequence exactly (also documented and exercised
/// identically by the `crates/cloud` spike's `review_import_params`), except the profile is always
/// present here (the registry resolves it), so there are no CLI-flag overrides to apply.
fn run_verify(path: &Path, profile: &Profile) -> Result<Vec<u8>, (Stage, String)> {
    let file = std::fs::File::open(path).map_err(|e| {
        (
            Stage::EngineError,
            format!("cannot reopen tempfile for read: {e}"),
        )
    })?;

    let mut params = profile.gcode_import_params();
    // `gcode_review_params` in crates/cli/src/main.rs: the review path forces these defaults when
    // the profile doesn't already specify them (raw g-code has no way to carry line width/layer
    // height itself).
    params.line_width = params.line_width.or(Some(0.45));
    params.layer_height = params.layer_height.or(Some(0.2));

    let imported = import_gcode_reader_with_map(file, &params)
        .map_err(|e| (Stage::InputInvalid, e.to_string()))?;

    let contracts = profile.contracts();
    let report = catch_unwind(AssertUnwindSafe(|| verify(&imported.toolpath, &contracts)))
        .map_err(|_| (Stage::EngineError, "dry-core verify panicked".to_string()))?;

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| (Stage::EngineError, format!("cannot serialize report: {e}")))?;
    Ok(format!("{json}\n").into_bytes())
}

/// Exposed for the byte-identity test: runs the same import+verify sequence `verify_handler` runs,
/// directly against a file path (no HTTP, no tempfile relay) — "the same verify via dry-core
/// directly the way the CLI does."
#[doc(hidden)]
pub fn run_verify_for_test(path: &Path, profile: &Profile) -> Result<Vec<u8>, String> {
    run_verify(path, profile).map_err(|(stage, message)| format!("{}: {message}", stage.as_str()))
}
