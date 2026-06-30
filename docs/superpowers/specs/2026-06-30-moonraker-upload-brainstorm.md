# Moonraker upload hook — brainstorm / design sketch
**Direction 2 · Product Directions §2 · 2026-06-30**

---

## Problem / motivation

After `dry review-gcode` or `dry rewrite-gcode`, the user still has to open Mainsail/Fluidd, drag the file
over, and click "Print". That manual step is where a bad file can slip through — or where a verified file
sits idle for too long. The gap is the last metre of the pipe:

```
PrusaSlicer / OrcaSlicer / Cura
  -> .gcode
  -> dry review-gcode / rewrite-gcode  (safe, deterministic — already ships)
  -> ???
  -> Moonraker / Klipper / printer     (not yet bridged)
```

`dry upload` closes that gap: it runs the same gate that `review-gcode` already runs and conditionally
calls `POST /server/files/upload` on Moonraker. The printer only receives a file that the deterministic
engine has accepted.

**Current state of network egress.** The workspace has exactly one network crate — `dry-llm` — which
calls the Anthropic Messages API. It is wired to `dry-cli` as an `optional` dependency behind a
`[features] llm` feature flag. The `dry-core` crate has no network code whatsoever, and the workspace
default build (`default-members = ["crates/core", "crates/cli"]`) links no HTTP stack. Any Moonraker
path must follow the same discipline.

---

## The gate — accept / warn / reject

The gate is already built into the engine. The primitives (from `crates/core/src/verify.rs` and
`crates/core/src/report.rs`) are:

| Concept | Rust type / method | Semantics |
|---|---|---|
| Finding severity | `Severity::Error` / `Severity::Warning` | Error = machine-safety violation; Warning = quality advisory |
| Gate predicate | `Report::ok()` | true iff `error_count() == 0` (no Error findings) |
| Error tally | `Report::error_count()` | counts only `Severity::Error` findings |
| Warning check | `report.findings.iter().any(|f| f.severity == Severity::Warning)` | warnings that do not block |

The three warning-only rules (`travel-without-retraction`, `first-layer-height`, `first-layer-speed`)
are advisory; all twelve remaining rules produce errors.

**Gate semantics for `dry upload`:**

1. **Accept** (`error_count == 0`, no warnings): upload the file; if `--print` was passed, start it;
   exit 0.
2. **Warn** (`error_count == 0`, warnings present): print warning list to stderr with source lines; upload
   the file but do NOT auto-start even if `--print` was passed; exit 0 (the upload succeeded, the user
   is warned). If `--force` is also passed, respect `--print` and start.
3. **Reject** (`error_count > 0`): print all Error findings to stderr with source lines; refuse to upload;
   exit 1. Pass `--force` to override and upload anyway (the gate becomes advisory).

Exit codes match `verify` / `review-gcode` conventions: 0 = clean (or warn-only upload succeeded), 1 =
errors present (upload blocked), 2 = CLI/IO error (die path, see `fn die` in `main.rs`).

**Optional rewrite before upload.** If `--rewrite <mode>` (`safe`|`balanced`|`max`) is given, run
`rewrite-gcode --mode <mode>` on the file in memory first, then gate on the rewritten result. The
rewritten bytes (not the original file) are what gets uploaded. This means the printer always gets the
cleanest available version, not the raw slicer output.

---

## Where the network code lives — three approaches

### (a) A new feature-gated crate `dry-moonraker` — **recommended**

Mirror the exact structure of `dry-llm`:

```
crates/moonraker/
  Cargo.toml            [package] name = "dry-moonraker"; [dependencies] ureq, serde, serde_json
  src/lib.rs            MoonrakerConfig, MoonrakerError, upload_file(), start_print()
```

Wire into `dry-cli/Cargo.toml`:

```toml
[features]
llm        = ["dep:dry-llm"]
moonraker  = ["dep:dry-moonraker"]

[dependencies]
dry-moonraker = { path = "../moonraker", optional = true }
```

Add `"crates/moonraker"` to `workspace.members` in the root `Cargo.toml`, but keep it out of
`default-members` (just like `dry-llm`). The default `cargo build -p dry-cli` links no HTTP stack.

CLI stubs in `main.rs` follow the same `#[cfg(not(feature = "moonraker"))] / #[cfg(feature =
"moonraker")]` split that `run_explain_llm` / `run_compare_llm` use, including the
`#[cfg_attr(not(feature = "moonraker"), allow(dead_code))]` guard on the args struct.

**Why this is cleanest:**
- `dry-core` stays pure; no HTTP dependency ever contaminates it.
- Each network concern (LLM calls, printer uploads) is isolated in its own crate with its own
  dependency surface (`ureq` + whatever Moonraker needs).
- The workspace default build stays free of any HTTP stack — `cargo build -p dry-cli` stays fast and
  link-clean.
- Follows an already-proven pattern in this codebase; future contributors see the template immediately.
- `dry-moonraker` can be unit-tested in isolation (mock uploads, test `MoonrakerError` paths) without
  touching `dry-cli`.

### (b) A feature on `dry-cli` with a shared HTTP helper

Instead of a new crate, put the Moonraker HTTP code directly in `crates/cli/src/moonraker.rs` behind a
`#[cfg(feature = "moonraker")]` attribute. Add `ureq` as an optional dependency of `dry-cli`.

**Upside:** fewer crates; simpler workspace.
**Downside:** `dry-cli` already has a clean separation from HTTP; adding `ureq` as an optional dep of the
CLI crate rather than a dedicated network crate makes the boundary fuzzier and harder to test in
isolation. It also sets a precedent where network code can grow unchecked inside the CLI.

### (c) Generalize a tiny shared HTTP layer out of `dry-llm`

Extract `post_messages` and the `ureq` setup into a hypothetical `dry-http` helper crate, then
`dry-llm` and `dry-moonraker` both depend on it.

**Upside:** avoids duplicating `ureq` version pins.
**Downside:** over-engineered for v1 — `dry-llm` is a 280-line file; the shared surface would be
`ureq::post` + a header helper. The abstraction doesn't pay until a third HTTP client appears. Defer.

**Recommendation: (a).** It matches the existing pattern exactly, keeps `dry-core` pure, keeps the
default build lean, and is straightforward to review and extend.

---

## CLI surface

```
dry upload <file.gcode>
    --moonraker <url>          Moonraker base URL, e.g. http://voron.local
    [--api-key-env <VAR>]      Env var holding the Moonraker X-Api-Key (default: MOONRAKER_API_KEY)
    [--print]                  Start printing immediately after upload (clean/forced-warn only)
    [--force]                  Upload even when Error findings are present; also unblocks --print on warn
    [--rewrite <mode>]         safe|balanced|max — rewrite motion in memory before uploading
    [--profile <path>]         Machine/material profile JSON for import + verifier contracts
    [--filament-diameter <mm>]
    [--line-width <mm>]
    [--layer-height <mm>]
    [--max-flow <mm3s>]
    [--bounds x0,x1,y0,y1,z0,z1]
    [--monotonic-z]
    [--min-temp <°C>]
    [--speed-range min,max]
    [--json]                   Emit the UploadResult as JSON to stdout (gate verdict + findings)
```

**Key from env var.** Moonraker supports trusted-client networks (no key required) and
`X-Api-Key` auth. The key lives in an env var (default `MOONRAKER_API_KEY`) to mirror the
`ANTHROPIC_API_KEY` pattern. `--api-key-env FOO` lets users name a different variable (e.g. when
managing multiple printers). If the env var is unset and the Moonraker host requires a key, the
response will be 401 — the error propagates as a `MoonrakerError::Http(401, ...)`.

**Stderr / stdout contract (non-JSON mode):**
```
upload: path/to/file.gcode → http://voron.local
  profile:   Voron 2.4 ABS profile
  segments:  7212 (5104 moves with length)
  time:      1h 42m 15s (print 1h 39m, travel 3m 15s)
  peak flow: 14.82 mm³/s
  verify:    OK (no findings)
  uploading… done (1.2 MB in 0.3 s)
  printing:  started (job id: print_12345)
```
Or on warn:
```
  [Warning] travel-without-retraction line 4832: travel run 48.2 mm without retraction
  verify:    1 finding(s), 0 error(s) — uploaded, print NOT auto-started (pass --force to start)
```
Or on reject:
```
  [Error] max-flow line 3210: flow 24.3 mm³/s exceeds the ceiling 15.0
  verify:    3 finding(s), 2 error(s) — upload refused (pass --force to override)
error: upload blocked by 2 error finding(s)
```

---

## Components and data flow

```
Cmd::Upload { file, moonraker, api_key_env, print, force, rewrite, profile, … }
  │
  ├─ load file bytes + open as reader (fs::File::open)
  ├─ load_profile(profile) → Option<Profile>
  ├─ import_gcode_reader_with_map(input, &params) → ImportedGcode
  │     (uses gcode_review_params — same defaults as review-gcode)
  │
  ├─ [if --rewrite <mode>]
  │     apply_gated per span → Vec<Toolpath>
  │     emit_source_preserving_spans → Vec<String> (rewritten_lines)
  │     bytes_to_upload = rewritten_lines.join("\n").into_bytes()
  │  [else]
  │     bytes_to_upload = original file bytes
  │
  ├─ simulate(&imported.toolpath) → Metrics
  ├─ contracts_from_inputs(…) → Contracts
  ├─ verify(&imported.toolpath, &contracts) → Report
  │     report.ok() → bool (no Error findings)
  │     report.error_count() → usize
  │
  ├─ Gate decision:
  │     errors > 0 && !force  → print findings, exit 1
  │     warnings_only         → print findings, warn_mode = true
  │     clean || force        → proceed
  │
  └─ [feature = "moonraker"]
       dry_moonraker::MoonrakerConfig { base_url, api_key: Option<String> }
       dry_moonraker::upload_file(&cfg, filename, bytes_to_upload) → Result<UploadResponse, MoonrakerError>
         // POST /server/files/upload   multipart/form-data   X-Api-Key header (if key present)
       [if --print && (clean || force)]
         dry_moonraker::start_print(&cfg, filename) → Result<PrintResponse, MoonrakerError>
         // POST /printer/print/start   { "filename": "<name>" }
```

**`dry-moonraker` crate surface (v1):**

```rust
pub struct MoonrakerConfig {
    pub base_url: String,       // e.g. "http://voron.local"
    pub api_key: Option<String>,
}

pub enum MoonrakerError {
    Http(u16, String),
    Transport(String),
    Decode(String),
}

pub struct UploadResponse { pub filename: String }
pub struct PrintResponse  { pub job_id: String }

pub fn upload_file(cfg: &MoonrakerConfig, filename: &str, bytes: &[u8])
    -> Result<UploadResponse, MoonrakerError>;

pub fn start_print(cfg: &MoonrakerConfig, filename: &str)
    -> Result<PrintResponse, MoonrakerError>;
```

`dry-core` types consumed by the gate: `Report`, `Report::ok()`, `Report::error_count()`,
`Severity::Error`, `Severity::Warning`, `LocatedFinding` (for source-line output), `Metrics` (for the
pre-upload summary). These are all already public. No changes needed to `dry-core`.

---

## Scope / YAGNI — v1 boundary

**In scope for v1:**
- Upload (`POST /server/files/upload`) with the gate (accept / warn / reject).
- Optional `--print` (`POST /printer/print/start`) when gate allows.
- API key from env var.
- Optional `--rewrite <mode>` to rewrite in memory before uploading.
- `--force` to override the gate.
- `--json` to emit a structured `UploadResult`.
- `dry-moonraker` crate with `MoonrakerError` error type, `upload_file`, `start_print`.

**Deferred (not v1):**
- Print job status polling / websocket progress stream (Moonraker has a Websocket API).
- Multi-host upload (fan-out to a fleet of printers).
- Moonraker auth flavors beyond `X-Api-Key` (oneshot token, CORS origin trust).
- Upload to a subfolder / path parameter.
- Cancel / pause commands.
- `dry watch` integration that triggers upload on file-change.
- A `--dry-run` that runs the gate but skips the HTTP call (useful for CI).
- TLS certificate pinning or custom CA for HTTPS Moonraker installs.

The deferred items are real but none are needed to close the verify→printer on-ramp. V1 is the
narrowest useful slice.

---

## Open questions for the user

1. **Auto-start policy.** Should `--print` be opt-in (status quo in this design: explicit flag required)
   or the default for "clean" files? Auto-starting a print from a CLI command is a significant action
   and the explicit flag feels safer, but if the primary workflow is "verify + upload + print in one
   shot", the default may want to flip.

2. **Default gate strictness when no profile is given.** Without `--profile`, the contracts that fire
   are only the structural invariants (`finite`, `travel-extrudes`, `bead`, `arc-radius`,
   `orientation-not-unit`) — all errors — plus nothing contract-driven (no `max-flow`, `bounds`,
   `speed-range`, etc.). This means `dry upload file.gcode --moonraker …` with no profile will usually
   pass. Should that be allowed, or should we require `--profile` (or at least one contract flag) before
   upload? The `review-gcode` handler emits a warning today if `--mode` is used without `--profile`;
   the same pattern could apply here.

3. **What to do with `--rewrite` + `--force` + errors.** If the user passes `--rewrite max --force`
   and the rewritten file still has errors, should we upload the rewritten bytes (potentially better
   than the original even if not clean) or fall back to the original bytes? The cleanest semantics:
   always upload `bytes_to_upload` (the rewritten result if `--rewrite` was specified), and let `--force`
   govern whether errors are a hard stop or a warning.

4. **Filename on the printer.** Moonraker lets you supply a filename in the upload form; it defaults to
   the source file's basename. Should `--rewrite` uploads append a suffix (e.g. `.rewritten.gcode`)
   to avoid silently overwriting the original file on the printer's storage? Or is silent overwrite
   acceptable because the user knows they're rewriting?

5. **`--print` on warn (warn_mode).** The current design blocks `--print` when there are warnings
   (unless `--force`). Is that the right default? Warnings today are three advisory rules
   (`travel-without-retraction`, `first-layer-height`, `first-layer-speed`) — all quality-advisory, not
   machine-safety violations. A user with a trusted profile may prefer that warnings never block printing.
   An alternative: `--print` is never blocked by warnings (only errors); `--force` applies only to
   errors.
