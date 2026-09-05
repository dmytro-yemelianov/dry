//! Task 0 feasibility spike — proves (or disproves) that `dry-core`'s gcode import+verify path can
//! run inside a Cloudflare Worker (workers-rs). This crate is NOT product code: it is the minimal
//! scaffold needed to measure wall time under `wrangler dev` and to compile-check a queue consumer
//! stub. See `docs/superpowers/specs/2026-07-28-cloud-spike-findings.md` for the findings this
//! scaffold produced — that doc, not this file, is the binding interface for later tasks.
//!
//! `POST /spike/verify` reads the request body as raw gcode text, runs the *same* dry-core entry
//! points `crates/cli/src/main.rs`'s `review-gcode` arm uses (`import_gcode_reader_with_map` →
//! `simulate` → `verify` → `ReviewReport::build`), and returns timing JSON:
//! `{bytes, parse_ms, verify_ms, total_ms, segments, findings}`.
//!
//! Contract simplification vs the CLI: the spike has no `--profile` support, so it always imports
//! with the review defaults (`line_width = 0.45mm`, `layer_height = 0.2mm`, see
//! `gcode_review_params` in `crates/cli/src/main.rs`) and verifies against `Contracts::default()`
//! (all contract-driven checks disabled; only the always-on structural checks run). That is enough
//! to measure engine throughput, which is the spike's only goal — profile plumbing is Task 2/6.

use dry_core::{import_gcode_reader_with_map, simulate, verify, Contracts, GcodeImportParams};
use worker::{event, Context, Date, Env, Request, Response, Result, Router};

/// Review-mode import defaults — mirrors `gcode_review_params(None, None, None, None)` in the CLI
/// (no `--profile`, no overrides): `filament_diameter` stays at the `GcodeImportParams` default
/// (1.75mm) and only `line_width`/`layer_height` are set, exactly as the CLI does when no profile is
/// supplied.
fn review_import_params() -> GcodeImportParams {
    GcodeImportParams {
        line_width: Some(0.45),
        layer_height: Some(0.2),
        ..GcodeImportParams::default()
    }
}

fn parse_contracts_header(raw: Option<&str>) -> serde_json::Result<Contracts> {
    match raw {
        Some(value) => serde_json::from_str(value),
        None => Ok(Contracts::default()),
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .post_async("/verify", worker_verify)
        .post_async("/spike/verify", spike_verify)
        .run(req, env)
        .await
}

async fn worker_verify(mut req: Request, _ctx: worker::RouteContext<()>) -> Result<Response> {
    let body: Vec<u8> = req.bytes().await?;
    let params = review_import_params();
    let imported = match import_gcode_reader_with_map(body.as_slice(), &params) {
        Ok(imported) => imported,
        Err(e) => {
            return Response::error(format!("import failed: {e}"), 422);
        }
    };
    let contracts_header = req.headers().get("X-Dry-Contracts")?;
    let contracts = match parse_contracts_header(contracts_header.as_deref()) {
        Ok(contracts) => contracts,
        // `Contracts::default()` disables every contract-driven check. Degrading to it on a
        // malformed header answered 200 with a clean-looking report for a program nobody
        // verified — the caller asked for contracts and got silence. `verify-runner` refuses
        // bad input with a 4xx; this route now does too.
        Err(e) => {
            return Response::error(format!("invalid X-Dry-Contracts header: {e}"), 400);
        }
    };
    let report = verify(&imported.toolpath, &contracts);
    Response::from_json(&report)
}

#[cfg(test)]
mod tests {
    use super::parse_contracts_header;

    #[test]
    fn malformed_contracts_header_is_rejected() {
        assert!(parse_contracts_header(Some("{")).is_err());
    }

    #[test]
    fn missing_contracts_header_uses_the_documented_default() {
        let contracts = parse_contracts_header(None).expect("missing header should use defaults");
        assert_eq!(
            serde_json::to_value(contracts).expect("default contracts serialize"),
            serde_json::json!({ "monotonic_z": false })
        );
    }

    #[test]
    fn valid_contracts_header_preserves_requested_checks() {
        let contracts = parse_contracts_header(Some(r#"{"max_flow":12.5}"#))
            .expect("valid contracts header should parse");
        assert_eq!(contracts.max_flow, Some(12.5));
    }
}

async fn spike_verify(mut req: Request, _ctx: worker::RouteContext<()>) -> Result<Response> {
    let t_request_start = Date::now().as_millis();

    let body: Vec<u8> = req.bytes().await?;
    let bytes = body.len();

    let t_parse_start = Date::now().as_millis();
    let params = review_import_params();
    let imported = match import_gcode_reader_with_map(body.as_slice(), &params) {
        Ok(imported) => imported,
        Err(e) => {
            return Response::error(format!("import failed: {e}"), 422);
        }
    };
    let t_parse_end = Date::now().as_millis();

    let metrics = simulate(&imported.toolpath);
    let contracts = Contracts::default();
    let report = verify(&imported.toolpath, &contracts);
    let t_verify_end = Date::now().as_millis();

    let body = serde_json::json!({
        "bytes": bytes,
        "segments": imported.toolpath.segments.len(),
        "findings": report.findings.len(),
        "errors": report.findings.iter().filter(|f| f.severity == dry_core::Severity::Error).count(),
        "print_time_s": metrics.total_time_s.value(),
        // Worker-side Date() deltas. Workers freeze Date() within a request except across true I/O
        // (the `req.bytes().await` above is the one await point before this handler's CPU-bound work
        // starts) — see FINDINGS for the cross-check against curl wall-clock, which is the
        // authoritative number.
        "parse_ms": t_parse_end.saturating_sub(t_parse_start),
        "verify_ms": t_verify_end.saturating_sub(t_parse_end),
        "total_ms": t_verify_end.saturating_sub(t_request_start),
    });

    Response::from_json(&body)
}

/// Compile-check only: proves a `#[event(queue)]` consumer + `dry-core` verify link together and
/// build for wasm32 in this crate. Never wired to a real queue in the spike (no `wrangler.toml`
/// producer/consumer binding is deployed) — Task 6 replaces this with the real job pipeline
/// (load from R2, resolve pack/profile, write report, ack/retry).
#[derive(serde::Deserialize)]
struct SpikeQueueMessage {
    gcode: String,
}

#[event(queue)]
async fn queue(
    message_batch: worker::MessageBatch<SpikeQueueMessage>,
    _env: Env,
    _ctx: Context,
) -> Result<()> {
    for message in message_batch.messages()? {
        let params = review_import_params();
        let body = message.body();
        if let Ok(imported) = import_gcode_reader_with_map(body.gcode.as_bytes(), &params) {
            let _metrics = simulate(&imported.toolpath);
            let _report = verify(&imported.toolpath, &Contracts::default());
        }
    }
    message_batch.ack_all();
    Ok(())
}
