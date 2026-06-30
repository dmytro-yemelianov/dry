# `dry upload` (Moonraker upload hook) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `dry upload <file> --moonraker <url>` — run dry's verify gate (accept/warn/reject), then conditionally upload the (optionally rewritten) g-code to a Moonraker host and optionally start the print.

**Architecture:** A new feature-gated crate `dry-moonraker` (the only network code; mirrors `dry-llm`) holds the HTTP upload/start-print calls. `dry-cli` gains a `moonraker` feature + a `Cmd::Upload` that reuses the existing import/verify/rewrite primitives for the gate and calls `dry-moonraker` when the gate allows. `dry-core` is untouched and stays pure.

**Tech Stack:** Rust workspace; `ureq` (blocking HTTP, hand-built multipart), `serde`/`serde_json`, `clap`. Moonraker HTTP API (`POST /server/files/upload`, `POST /printer/print/start`).

## Global Constraints

- **`dry-core` stays pure** — no changes to it; the gate reuses existing public APIs.
- **Feature-gated** — `dry-moonraker` is `optional = true` on `dry-cli` behind `[features] moonraker = ["dep:dry-moonraker"]`; a workspace member but NOT in `default-members`. Default `cargo build`/`cargo test` links no HTTP stack. All `--moonraker` code in `main.rs` is `#[cfg(feature = "moonraker")]` (+ a `#[cfg(not(...))]` die-stub).
- **Gate semantics:** accept (0 errors, 0 warnings) → upload (+ start if `--print`); warn (0 errors, ≥1 warning) → upload, do NOT auto-start unless `--force`; reject (≥1 error) → no upload, exit 1, unless `--force`. Exit codes: 0 ok/warn-uploaded, 1 reject, 2 IO/network error (`die`).
- **Baked defaults:** `--print` opt-in; no `--profile` allowed but warns; `--rewrite` uploads rewritten bytes under the source basename; `--force` lets errors through AND unblocks `--print` on warn. Key from `MOONRAKER_API_KEY` (or `--api-key-env <VAR>`).
- **No live-network tests in CI** — `dry-moonraker`'s pure pieces (multipart, URL join, decode, header set) are unit-tested; the actual upload/start calls are not.
- **Workspace versions** inherited (`version.workspace = true`, etc.), matching `crates/llm/Cargo.toml`.
- **Commit cadence:** one commit per task.

---

### Task 1: `dry-moonraker` crate — types + multipart + URL helper (pure)

**Files:** Create `crates/moonraker/Cargo.toml`, `crates/moonraker/src/lib.rs`; Modify root `Cargo.toml` (`members` += `"crates/moonraker"`); Test: inline.

**Interfaces:** Produces `MoonrakerConfig`, `MoonrakerError` (+Display+Error), `UploadResponse`, `PrintResponse`, `const MULTIPART_BOUNDARY: &str`, `fn build_multipart(filename, bytes) -> Vec<u8>`, `fn join_url(base, path) -> String`.

- [ ] **Step 1: Manifest + workspace member**

`crates/moonraker/Cargo.toml` (mirror `crates/llm/Cargo.toml`):
```toml
[package]
name = "dry-moonraker"
description = "Dry's Moonraker (Klipper) upload client — network code, feature-gated."
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
ureq = { version = "2", features = ["json", "tls"] }
```
Add `"crates/moonraker"` to the root `Cargo.toml` `[workspace] members` (keep `default-members = ["crates/core", "crates/cli"]` unchanged).

- [ ] **Step 2: Write the failing tests**

`crates/moonraker/src/lib.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multipart_wraps_the_file_part() {
        let body = build_multipart("part.gcode", b"G1 X0\n");
        let s = String::from_utf8_lossy(&body);
        assert!(s.contains(&format!("--{MULTIPART_BOUNDARY}")));
        assert!(s.contains(r#"Content-Disposition: form-data; name="file"; filename="part.gcode""#));
        assert!(s.contains("application/octet-stream"));
        assert!(s.contains("G1 X0"));
        assert!(s.trim_end().ends_with(&format!("--{MULTIPART_BOUNDARY}--")));
    }
    #[test]
    fn join_url_trims_trailing_slash() {
        assert_eq!(join_url("http://voron.local/", "/server/files/upload"), "http://voron.local/server/files/upload");
        assert_eq!(join_url("http://voron.local", "/server/files/upload"), "http://voron.local/server/files/upload");
    }
}
```

- [ ] **Step 3: Run → fail**

Run: `cargo test -p dry-moonraker`
Expected: FAIL — `build_multipart`/`join_url`/`MULTIPART_BOUNDARY` undefined.

- [ ] **Step 4: Implement (above the tests)**

```rust
//! `dry-moonraker` — Dry's Moonraker upload client. The only network code for the upload feature;
//! mirrors `dry-llm`'s feature-gated, ureq-based structure. `dry-core` stays pure.

use serde::Deserialize;

/// Connection to a Moonraker host. `api_key` is sent as `X-Api-Key` when present.
pub struct MoonrakerConfig {
    pub base_url: String,
    pub api_key: Option<String>,
}

#[derive(Debug)]
pub enum MoonrakerError {
    Http(u16, String),
    Transport(String),
    Decode(String),
}
impl std::fmt::Display for MoonrakerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoonrakerError::Http(c, b) => write!(f, "Moonraker returned HTTP {c}: {b}"),
            MoonrakerError::Transport(m) => write!(f, "network error reaching Moonraker: {m}"),
            MoonrakerError::Decode(m) => write!(f, "could not parse Moonraker response: {m}"),
        }
    }
}
impl std::error::Error for MoonrakerError {}

pub struct UploadResponse { pub filename: String }
pub struct PrintResponse { pub job_started: bool }

/// Fixed multipart boundary — deterministic for testing; long+unique to avoid g-code collisions.
pub const MULTIPART_BOUNDARY: &str = "dry7c0d3moonrakerboundary8f3a1e9d";

/// Build a `multipart/form-data` body with a single `file` part. ureq has no multipart helper.
pub fn build_multipart(filename: &str, bytes: &[u8]) -> Vec<u8> {
    let header = format!(
        "--{MULTIPART_BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\
Content-Type: application/octet-stream\r\n\r\n"
    );
    let footer = format!("\r\n--{MULTIPART_BOUNDARY}--\r\n");
    let mut body = Vec::with_capacity(header.len() + bytes.len() + footer.len());
    body.extend_from_slice(header.as_bytes());
    body.extend_from_slice(bytes);
    body.extend_from_slice(footer.as_bytes());
    body
}

/// Join a base URL (trailing `/` trimmed) with an absolute path.
pub fn join_url(base: &str, path: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), path)
}
```

- [ ] **Step 5: Run → pass; build; commit**

Run: `cargo test -p dry-moonraker` (PASS); `cargo build` (whole default workspace builds with the new member); `cargo fmt -p dry-moonraker` + `cargo clippy -p dry-moonraker --all-targets -- -D warnings`.
```bash
git add Cargo.toml crates/moonraker/
git commit -m "feat(moonraker): dry-moonraker crate — types, multipart body, url helper"
```

### Task 2: `upload_file` + `start_print` (network) + response decode

**Files:** Modify `crates/moonraker/src/lib.rs`; Test: inline (decode only).

**Interfaces:** Produces `fn upload_file(cfg, filename, bytes) -> Result<UploadResponse, MoonrakerError>`, `fn start_print(cfg, filename) -> Result<PrintResponse, MoonrakerError>`, and a private `fn post(cfg, path, content_type, body) -> Result<serde_json::Value, MoonrakerError>` (the only network I/O).

- [ ] **Step 1: Write the failing test (decode is pure)**

Add to `tests`:
```rust
    #[test]
    fn upload_response_decodes_filename() {
        // Moonraker upload returns { "item": { "path": "part.gcode", ... }, ... }
        let v: serde_json::Value = serde_json::from_str(r#"{"item":{"path":"part.gcode"}}"#).unwrap();
        assert_eq!(decode_upload(&v).unwrap().filename, "part.gcode");
    }
    #[test]
    fn missing_path_is_decode_error() {
        let v: serde_json::Value = serde_json::from_str(r#"{"item":{}}"#).unwrap();
        assert!(matches!(decode_upload(&v), Err(MoonrakerError::Decode(_))));
    }
```

- [ ] **Step 2: Run → fail** (`decode_upload` undefined). Run: `cargo test -p dry-moonraker decode`.

- [ ] **Step 3: Implement**

```rust
fn decode_upload(v: &serde_json::Value) -> Result<UploadResponse, MoonrakerError> {
    v["item"]["path"].as_str()
        .map(|p| UploadResponse { filename: p.to_string() })
        .ok_or_else(|| MoonrakerError::Decode(format!("no item.path in upload response: {v}")))
}

fn post(cfg: &MoonrakerConfig, path: &str, content_type: &str, body: &[u8]) -> Result<serde_json::Value, MoonrakerError> {
    let mut req = ureq::post(&join_url(&cfg.base_url, path)).set("Content-Type", content_type);
    if let Some(k) = &cfg.api_key { req = req.set("X-Api-Key", k); }
    match req.send_bytes(body) {
        Ok(r) => r.into_json().map_err(|e| MoonrakerError::Decode(format!("invalid JSON: {e}"))),
        Err(ureq::Error::Status(code, r)) => Err(MoonrakerError::Http(code, r.into_string().unwrap_or_default().chars().take(500).collect())),
        Err(ureq::Error::Transport(t)) => Err(MoonrakerError::Transport(t.to_string())),
    }
}

/// POST the g-code to `/server/files/upload` as multipart/form-data. Network.
pub fn upload_file(cfg: &MoonrakerConfig, filename: &str, bytes: &[u8]) -> Result<UploadResponse, MoonrakerError> {
    let body = build_multipart(filename, bytes);
    let ct = format!("multipart/form-data; boundary={MULTIPART_BOUNDARY}");
    decode_upload(&post(cfg, "/server/files/upload", &ct, &body)?)
}

/// Start a print of an already-uploaded file via `/printer/print/start`. Network.
pub fn start_print(cfg: &MoonrakerConfig, filename: &str) -> Result<PrintResponse, MoonrakerError> {
    let body = serde_json::json!({ "filename": filename }).to_string();
    let _ = post(cfg, "/printer/print/start", "application/json", body.as_bytes())?;
    Ok(PrintResponse { job_started: true })
}
```

- [ ] **Step 4: Run → pass; build; commit**

Run: `cargo test -p dry-moonraker` (PASS); `cargo build`; `cargo clippy -p dry-moonraker --all-targets -- -D warnings`.
```bash
git add crates/moonraker/src/lib.rs
git commit -m "feat(moonraker): upload_file + start_print + response decode"
```

---

### Task 3: `moonraker` feature + `Cmd::Upload` flags + stubs

**Files:** Modify `crates/cli/Cargo.toml` (optional dep + feature), `crates/cli/src/main.rs` (variant + routing + stubs).

**Interfaces:** Adds `Cmd::Upload { file, moonraker, api_key_env, print, force, rewrite, profile, filament_diameter, line_width, layer_height, max_flow, bounds, monotonic_z, min_temp, speed_range, json }`; `UploadArgs` struct; `#[cfg(not(feature="moonraker"))]` die-stub + a temporary `#[cfg(feature="moonraker")]` stub so both build configs are green.

- [ ] **Step 1: Cargo.toml** — add to `crates/cli/Cargo.toml`:
```toml
dry-moonraker = { path = "../moonraker", optional = true }
```
and extend `[features]`:
```toml
moonraker = ["dep:dry-moonraker"]
```

- [ ] **Step 2: `Cmd::Upload` variant** — add to `enum Cmd` (mirror the flag style of `Explain`/`ReviewGcode`):
```rust
    /// Verify a g-code file and upload it to a Moonraker host (accept/warn/reject gate).
    Upload {
        file: String,
        #[arg(long)] moonraker: String,
        #[arg(long, default_value = "MOONRAKER_API_KEY")] api_key_env: String,
        #[arg(long)] print: bool,
        #[arg(long)] force: bool,
        #[arg(long)] rewrite: Option<OptimizeModeArg>,
        #[arg(long)] profile: Option<String>,
        #[arg(long)] filament_diameter: Option<f64>,
        #[arg(long)] line_width: Option<f64>,
        #[arg(long)] layer_height: Option<f64>,
        #[arg(long)] max_flow: Option<f64>,
        #[arg(long)] bounds: Option<String>,
        #[arg(long)] monotonic_z: bool,
        #[arg(long)] min_temp: Option<f64>,
        #[arg(long)] speed_range: Option<String>,
        #[arg(long)] json: bool,
    },
```
Match arm: route to `run_upload(UploadArgs { … })` (all fields moved in). Add the `UploadArgs` struct (mirror `ExplainLlmArgs`, `#[cfg_attr(not(feature = "moonraker"), allow(dead_code))]`), a `#[cfg(not(feature = "moonraker"))]` die-stub (`die("this build was compiled without --moonraker support; rebuild with `cargo build --features moonraker`".into())`), and a TEMPORARY `#[cfg(feature = "moonraker")] fn run_upload(_: UploadArgs) -> std::process::ExitCode { unimplemented!() }` so both build configs are green (Task 4 replaces it).

- [ ] **Step 3: Verify both build configs**

Run: `cargo build` (default → green; `dry upload` routes to the die-stub); `cargo build --features moonraker` (green, temporary stub); `cargo test` (default; existing unaffected); `cargo run -- upload --help` shows the flags. `cargo fmt` + `cargo clippy --all-targets -- -D warnings` + `cargo clippy --all-targets --features moonraker -- -D warnings`.

- [ ] **Step 4: Commit**
```bash
git add crates/cli/Cargo.toml crates/cli/src/main.rs
git commit -m "feat(cli): moonraker feature + dry upload flags + stubs"
```

---

### Task 4: `run_upload` — gate + rewrite + upload (orchestration)

**Files:** Modify `crates/cli/src/main.rs` (replace the temporary `#[cfg(feature="moonraker")]` stub).

**Interfaces:** Consumes `dry_moonraker::{MoonrakerConfig, upload_file, start_print, MoonrakerError}`; `dry_core::{import_gcode_reader_with_map, simulate, verify, ReviewReport, Severity, OptimizeMode, apply_gated, Toolpath, EmitParams}`; CLI helpers `load_profile`, `gcode_review_params`, `contracts_from_inputs`, `profile_label`, `die`, `optimize_mode_label`.

- [ ] **Step 1: Implement `run_upload`** (remove the `#[allow(dead_code)]` once fields are read):

```rust
#[cfg(feature = "moonraker")]
fn run_upload(args: UploadArgs) -> std::process::ExitCode {
    use std::path::Path;
    let api_key = std::env::var(&args.api_key_env).ok();   // None → trusted-client (no key); 401 if host requires one
    let input = fs::File::open(&args.file).unwrap_or_else(|e| die(format!("cannot read {}: {e}", args.file)));
    let profile = load_profile(args.profile.as_deref());
    if profile.is_none() {
        eprintln!("warning: no --profile — only structural invariants are checked (no flow/bounds/speed contracts)");
    }
    let params = gcode_review_params(profile.as_ref(), args.filament_diameter, args.line_width, args.layer_height);
    let imported = import_gcode_reader_with_map(input, &params).unwrap_or_else(|e| die(format!("cannot import {}: {e}", args.file)));
    let contracts = contracts_from_inputs(profile.as_ref(), args.bounds.as_deref(), args.max_flow, args.speed_range.as_deref(), args.monotonic_z, args.min_temp);

    // Optional rewrite (mirror Cmd::RewriteGcode's per-span gated rewrite) → the bytes we upload.
    let (bytes_to_upload, rewrite_note) = if let Some(modearg) = args.rewrite {
        let mode = OptimizeMode::from(modearg);
        let kinematics = profile.as_ref().and_then(|p| p.machine.kinematics.as_ref());
        let emit_params = EmitParams { relative_e: true, travel_g1_e0: false, five_axis: false,
            kinematics: dry_core::Kinematics::default(),
            flavor: profile.as_ref().map(|p| p.emit_params().flavor).unwrap_or(dry_core::FirmwareFlavor::Marlin) };
        let mut span_tps = Vec::new();
        for span in imported.motion_spans() {
            let r = span.segment_range();
            let span_tp = Toolpath { version: imported.toolpath.version, meta: imported.toolpath.meta.clone(),
                segments: imported.toolpath.segments[r].to_vec() };
            span_tps.push(apply_gated(&span_tp, &contracts, mode, kinematics).toolpath);
        }
        let lines = imported.emit_source_preserving_spans(&span_tps, &emit_params)
            .unwrap_or_else(|e| die(format!("cannot rewrite {}: {e}", args.file)));
        (lines.join("\n").into_bytes(), format!(" (rewritten --mode {})", optimize_mode_label(mode)))
    } else {
        (fs::read(&args.file).unwrap_or_else(|e| die(format!("cannot read {}: {e}", args.file))), String::new())
    };

    // Gate on the (possibly rewritten) toolpath — re-import the rewritten bytes so findings match what we upload.
    let gate_toolpath = if args.rewrite.is_some() {
        import_gcode_reader_with_map(std::io::Cursor::new(&bytes_to_upload), &params)
            .unwrap_or_else(|e| die(format!("cannot re-import rewritten g-code: {e}"))).toolpath
    } else { imported.toolpath.clone() };
    let metrics = simulate(&gate_toolpath);
    let report = verify(&gate_toolpath, &contracts);
    let review = dry_core::ReviewReport::build(Some(args.file.clone()), profile_label(profile.as_ref()),
        gate_toolpath.segments.len(), metrics, &report, |seg| imported.source_line_for_segment(seg));

    let errors = review.error_count;
    let warnings = review.findings.iter().filter(|f| f.severity == Severity::Warning).count();
    // Print findings (errors + warnings) with source lines.
    for f in &review.findings {
        let tag = if f.severity == Severity::Error { "Error" } else { "Warning" };
        let line = f.source_line.map(|l| format!(" line {l}")).unwrap_or_default();
        eprintln!("  [{tag}] {}{line}: {}", f.rule, f.message);
    }
    if errors > 0 && !args.force {
        eprintln!("error: upload blocked by {errors} error finding(s) (pass --force to override)");
        return std::process::ExitCode::from(1);
    }
    let warn_mode = warnings > 0;

    // Upload.
    let basename = Path::new(&args.file).file_name().and_then(|s| s.to_str()).unwrap_or("upload.gcode").to_string();
    let cfg = dry_moonraker::MoonrakerConfig { base_url: args.moonraker.clone(), api_key };
    let up = dry_moonraker::upload_file(&cfg, &basename, &bytes_to_upload).unwrap_or_else(|e| die(e.to_string()));
    let mut printed = false;
    let may_print = args.print && (!warn_mode || args.force) && (errors == 0 || args.force);
    if may_print {
        dry_moonraker::start_print(&cfg, &up.filename).unwrap_or_else(|e| die(e.to_string()));
        printed = true;
    }

    if args.json {
        let gate = if errors > 0 { "reject-forced" } else if warn_mode { "warn" } else { "accept" };
        let env = serde_json::json!({ "gate": gate, "uploaded": true, "printed": printed,
            "error_count": errors, "warning_count": warnings, "moonraker_url": args.moonraker,
            "filename": up.filename, "rewrite": rewrite_note.trim() });
        print!("{}\n", serde_json::to_string_pretty(&env).unwrap());
    } else {
        eprintln!("upload: {} → {}{}", args.file, args.moonraker, rewrite_note);
        eprintln!("  verify: {} finding(s), {errors} error(s) — uploaded as {}", review.findings.len(), up.filename);
        if warn_mode && args.print && !args.force { eprintln!("  print NOT auto-started (warnings present; pass --force)"); }
        if printed { eprintln!("  printing: started"); }
    }
    std::process::ExitCode::SUCCESS
}
```

**Implementer note:** confirm `EmitParams`/`Kinematics`/`FirmwareFlavor`/`Profile::emit_params()` field names against `Cmd::RewriteGcode` (`crates/cli/src/main.rs:925-934`) and copy its exact `emit_params` construction; confirm `ReviewReport::build`'s closure arg (`source_line_for_segment`) matches the `Explain` handler. Adjust to the real signatures.

- [ ] **Step 2: Build both configs + test**

Run: `cargo build` (default, green); `cargo build --features moonraker` (green); `cargo test` (default unaffected); `cargo clippy --all-targets -- -D warnings` AND `cargo clippy --all-targets --features moonraker -- -D warnings`; `cargo fmt`.

- [ ] **Step 3: Manual e2e (network — local, with a Moonraker host or a stub)**

```bash
MOONRAKER_API_KEY=… cargo run --features moonraker -- upload conformance/reports/compare/slow.gcode \
  --moonraker http://localhost:7125 --profile <p>
```
Expected: the gate summary, then `uploading… done`; reject path exits 1 with errors; `--force` overrides. (If no Moonraker host is available, verify the gate/exit-code logic with a bad/locked file and confirm the upload attempt produces a clear `MoonrakerError` rather than a panic.)

- [ ] **Step 4: Commit**
```bash
git add crates/cli/src/main.rs
git commit -m "feat(cli): dry upload — gate + optional rewrite + Moonraker upload/print"
```

---

### Task 5: docs + CHANGELOG + CI

**Files:** `docs/15-cli-cookbook.md`, `docs/05-product-directions.md`, `CHANGELOG.md`, `.github/workflows/ci.yml`.

- [ ] **Step 1: Cookbook** — add a `dry upload` recipe to `docs/15-cli-cookbook.md`: the accept/warn/reject gate, `--print`/`--force`/`--rewrite`, `MOONRAKER_API_KEY`, the verify→printer workflow (`review-gcode` → `upload --rewrite balanced --print`).
- [ ] **Step 2: Product directions** — in `docs/05-product-directions.md`, mark Direction 2 (the verify→printer on-ramp) shipped (v1: upload + gate + optional print/rewrite).
- [ ] **Step 3: CHANGELOG** — `[Unreleased]`:
```
- `dry upload <file> --moonraker <url>`: verify gate (accept/warn/reject) then upload the (optionally
  `--rewrite`-cleaned) g-code to a Moonraker host, with optional `--print`; `--force` overrides the gate.
  New feature-gated `dry-moonraker` crate is the only network code; `dry-core` stays pure.
```
- [ ] **Step 4: CI** — in `.github/workflows/ci.yml`, extend the feature step (the one that builds `--features llm`) to also cover moonraker: `cargo build --features moonraker` + `cargo test -p dry-moonraker` (+ `cargo clippy --all-targets --features moonraker -- -D warnings` if the llm clippy step is mirrored). `dry-moonraker`'s tests are offline — safe in CI.
- [ ] **Step 5: Final check + commit** — `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features moonraker -- -D warnings && cargo test && cargo build --features moonraker && cargo test -p dry-moonraker`. Commit `docs+ci: dry upload (Moonraker)`.

---

## Self-Review

**Spec coverage:** `dry-moonraker` crate (types, multipart, upload/start) → Tasks 1–2; `moonraker` feature + flags + stubs → Task 3; gate (accept/warn/reject) + rewrite + upload orchestration + baked defaults → Task 4; docs/CHANGELOG/CI → Task 5. Determinism: gate/rewrite are existing tested dry-core paths; `dry-moonraker` pure pieces unit-tested; network not CI-tested.

**Placeholder scan:** the one "confirm against the real code" marker is Task 4's `EmitParams`/`emit_params` construction + the `ReviewReport::build` closure — pinned to `Cmd::RewriteGcode` (`main.rs:925-934`) and the `Explain` handler, the exact existing call sites to copy. The manual e2e is a network step (no Moonraker host in CI). All other steps carry real code.

**Type consistency:** `MoonrakerConfig`/`MoonrakerError`/`UploadResponse`/`PrintResponse`/`build_multipart`/`join_url`/`upload_file`/`start_print`/`MULTIPART_BOUNDARY` defined in Tasks 1–2 and consumed in Task 4; `decode_upload`/`post` private helpers in Task 2; `UploadArgs` + `run_upload` + the die-stub names consistent across Tasks 3–4; the `moonraker` feature name + `dep:dry-moonraker` consistent across Tasks 3/5. The gate predicate uses `review.error_count` + a `Severity::Warning` count, matching the engine's `Report::ok()`/`error_count()` semantics.
