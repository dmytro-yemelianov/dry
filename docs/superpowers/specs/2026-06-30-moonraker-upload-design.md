# `dry upload` — Moonraker upload hook (Direction 2)

**Date:** 2026-06-30
**Status:** Approved design, ready for implementation
**Branch:** `feat/moonraker-upload`
**Exploration input:** `docs/superpowers/specs/2026-06-30-moonraker-upload-brainstorm.md`

## Problem

After `dry review-gcode`/`rewrite-gcode`, getting the file onto the printer is a manual Mainsail/Fluidd
drag-and-drop — the last unbridged metre of the pipe, and where a bad file can slip through. `dry upload`
closes it: run the same deterministic gate `review-gcode` runs, then conditionally `POST` the file to a
Moonraker host. The printer only ever receives a file the engine accepted.

## Decisions (resolved during brainstorming)

1. **New feature-gated crate `dry-moonraker`** — the only network code for this feature, mirroring
   `dry-llm` exactly (optional dep on `dry-cli` behind a `moonraker` feature; a workspace member but kept
   out of `default-members`, so the default build links no HTTP stack). `dry-core` stays pure.
2. **Gate = accept / warn / reject** (built on the engine's existing `Report::ok()` / `error_count()`):
   - **Accept** (0 errors, 0 warnings): upload; if `--print`, start; exit 0.
   - **Warn** (0 errors, ≥1 warning): print warnings to stderr, upload, but **do NOT auto-start** even
     with `--print` — unless `--force`; exit 0.
   - **Reject** (≥1 error): print errors to stderr, **refuse upload**, exit 1 — unless `--force` (gate
     becomes advisory and it uploads anyway).
3. **Baked defaults** (the brainstorm's open questions, resolved):
   - `--print` is **opt-in** (an explicit flag; auto-starting a physical print is deliberate).
   - **No `--profile` is allowed but warns loudly** (only structural-invariant errors fire without
     contracts) — mirroring `review-gcode`'s "`--mode` without `--profile`" warning.
   - `--rewrite <mode>` uploads the **rewritten bytes under the source basename** (transparent; no
     surprise suffix, no silent original).
   - `--force` governs both: it lets errors through AND unblocks `--print` on warn.
4. **API key from an env var** (`MOONRAKER_API_KEY` by default; `--api-key-env <VAR>` to override) —
   mirroring the `ANTHROPIC_API_KEY` pattern. Absent key + a host that requires one → `Http(401)`.

## `dry-moonraker` crate (the only network code)

`crates/moonraker/` — `Cargo.toml` (`ureq`, `serde`, `serde_json`), `src/lib.rs`:

```rust
pub struct MoonrakerConfig { pub base_url: String, pub api_key: Option<String> }
pub enum MoonrakerError { Http(u16, String), Transport(String), Decode(String) }   // + Display + Error
pub struct UploadResponse { pub filename: String }
pub struct PrintResponse  { pub job_started: bool }   // Moonraker's print/start returns "ok"; no stable job id

pub fn upload_file(cfg: &MoonrakerConfig, filename: &str, bytes: &[u8]) -> Result<UploadResponse, MoonrakerError>;
pub fn start_print(cfg: &MoonrakerConfig, filename: &str) -> Result<PrintResponse, MoonrakerError>;
```

- `upload_file`: `POST {base_url}/server/files/upload`, `Content-Type: multipart/form-data; boundary=…`,
  `X-Api-Key` header when `api_key` is `Some`. **ureq has no multipart helper**, so the body is
  hand-built: a single `file` part with `Content-Disposition: form-data; name="file"; filename="<name>"`
  + `Content-Type: application/octet-stream` + the bytes, wrapped in a generated boundary. Keep the
  boundary construction in one tested pure helper (`build_multipart(filename, bytes) -> (String /*boundary*/, Vec<u8> /*body*/)`).
- `start_print`: `POST {base_url}/printer/print/start`, JSON body `{ "filename": "<name>" }`, same key header.
- Error mapping mirrors `dry-llm`: non-2xx → `Http(code, body_snippet≤500)`; transport → `Transport`;
  bad/again-unexpected JSON → `Decode`. URL joining trims a trailing `/` on `base_url`.

## CLI surface

```
dry upload <file.gcode> --moonraker <url>
   [--api-key-env <VAR>]   (default MOONRAKER_API_KEY)
   [--print] [--force] [--rewrite safe|balanced|max]
   [--profile <path>] [--filament-diameter <mm>] [--line-width <mm>] [--layer-height <mm>]
   [--max-flow <mm3s>] [--bounds x0,x1,y0,y1,z0,z1] [--monotonic-z] [--min-temp <°C>] [--speed-range min,max]
   [--json]
```

Feature-gated exactly like `explain --llm` / `compare --llm`: a `#[cfg(not(feature = "moonraker"))]`
die-stub ("compiled without --moonraker support; rebuild with `cargo build --features moonraker`") and the
real `#[cfg(feature = "moonraker")] fn run_upload`, with `#[cfg_attr(not(feature = "moonraker"), allow(dead_code))]` on the args struct.

**Non-JSON output** (stderr summary + verdict) and **`--json`** (`UploadResult { gate: "accept|warn|reject", uploaded: bool, printed: bool, findings: [...], error_count, warning_count, moonraker_url }`). `--json` is documented, NOT drift-gated (it reports a network outcome).

## Data flow

```
Cmd::Upload {…}                                                    (dry-cli, #[cfg(feature="moonraker")])
  → open file; load_profile; gcode_review_params; import_gcode_reader_with_map → ImportedGcode
  → [--rewrite m] per-span apply_gated → emit_source_preserving_spans → rewritten bytes  (else original bytes)
  → simulate; contracts_from_inputs; verify(toolpath, contracts) → Report
  → gate: error_count>0 && !force → print errors, exit 1
          warnings present        → print warnings, warn_mode=true
          (warn no --profile)     → also warn "no profile — only structural checks ran"
  → dry_moonraker::upload_file(cfg, basename, bytes)
  → [--print && (clean || force-on-warn)] dry_moonraker::start_print(cfg, basename)
  → render summary | UploadResult JSON; cost-free (no LLM)
```

Reused `dry-core` (all already public): `import_gcode_reader_with_map`, `simulate`, `verify`,
`Report::ok()`/`error_count()`, `Severity::{Error,Warning}`, `LocatedFinding`, `Metrics`, `apply_gated`,
`emit_source_preserving_spans`, `OptimizeMode`. Reused CLI helpers: `load_profile`, `gcode_review_params`,
`contracts_from_inputs`, `profile_label`, `die`. **No changes to `dry-core`.**

## Error handling

- `--moonraker` missing → clap-required (or `die` if modeled optional). Unreadable file → `die`, exit 2.
- `MoonrakerError` → `die(e.to_string())`, exit 2 (the upload itself failed — distinct from a gate reject,
  which is exit 1). 401 (missing/invalid key) surfaces as `Http(401, …)` with an actionable hint.
- Reject (errors, no `--force`) → exit 1, no network call made.
- Non-g-code input → the existing actionable hint.

## Testing & determinism

- The gate + rewrite are deterministic dry-core paths (already golden-tested) — no new goldens needed.
- `dry-moonraker`: unit-test the **pure** pieces with no network — `build_multipart` (boundary present,
  `Content-Disposition`/filename correct, bytes embedded, terminating boundary), URL joining (trailing-`/`
  trim), the request-builder header set (`X-Api-Key` present iff key), and response/error decoding (2xx →
  `UploadResponse`; non-2xx → `Http`; transport → `Transport`). The `upload_file`/`start_print` network
  calls are the only impure lines (not unit-tested in CI — no Moonraker host); an `#[ignore]`d smoke test
  may hit a real host locally.
- CLI: gate-decision logic is thin over the tested dry-core primitives; a default-build test confirms the
  `--moonraker` die-stub message; a `--features moonraker` build + clippy must be green. No live-network
  CI test.

## Scope / YAGNI (deferred)

Print-status polling / websocket progress; multi-host fan-out; auth beyond `X-Api-Key`; subfolder upload;
cancel/pause; `dry watch` auto-upload; `--dry-run` (gate-only, no HTTP); TLS pinning/custom CA.
