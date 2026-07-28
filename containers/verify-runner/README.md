# `containers/verify-runner`

Native `dry-core` verify shim — the compute engine cloud verify jobs dispatch to. The Worker (Task
R3) sends it raw g-code over HTTP; it fetches a resolved profile from the printer registry, runs the
exact same `dry-core` import+verify call sequence the CLI's `review-gcode` path uses, and returns the
byte-identical `dry verify --json` report.

Workspace-excluded (see the root `Cargo.toml`'s `exclude`), same pattern as `crates/wasm` and
`crates/cloud`: its own `Cargo.lock`, builds standalone (`cargo build` from this directory) or in
Docker.

## API

### `POST /verify?pack=<packId>&version=<version>&profile=<profileId>&registry=<registryBaseUrl>`

Body: raw g-code, streamed to a tempfile under `/tmp` (never buffered whole in memory). Request body
cap: 200MB (`DefaultBodyLimit`) — the Worker enforces a 100MB `Content-Length` cap upstream; this is
deliberate headroom, not the product limit.

- `200` — body is `serde_json::to_string_pretty(&report) + "\n"`, byte-identical to what
  `dry verify --json` prints locally for the same profile+input (see "Byte-identity" below).
- `4xx/5xx` — `{"error": "<message>", "stage": "<stage>"}`:
  - `502 profile-unavailable` — the registry fetch failed: network error, non-2xx, or an unparseable
    profile body.
  - `422 input-invalid` — dry-core rejected the g-code with a `Result` error (not a panic) — e.g.
    malformed/non-UTF8 input.
  - `500 engine-error` — `dry-core`'s `verify()` call panicked (caught via `catch_unwind`), or an
    internal failure (tempfile I/O, report serialization).
  - Anything else (e.g. a missing/malformed query parameter) falls back to axum's default extractor
    rejection — a plain `4xx`, deliberately NOT one of the three stages above.

### `GET /healthz` → `200 {"ok": true}`

## Profile-selection decision

`docs/19-printer-registry-api.md` documents the registry's REST artifact route as:

```
GET /v1/profiles/{printer-id}/{version}/{profile-id}
```

A single pack version can resolve to more than one profile artifact — one per
material/nozzle-diameter combination (see that doc's GraphQL `profiles(materialId: ...,
nozzleDiameterMm: ...)` field). The REST doc does not say how a caller holding only `pack` +
`version` (no GraphQL client) should pick which `profile-id` to fetch; that resolution is a GraphQL
search a full registry client performs, which is out of scope for this container.

**Decision:** `POST /verify` takes an explicit `profile=<profileId>` query parameter in addition to
`pack`/`version`/`registry`. The runner does zero profile resolution of its own — it fetches exactly
`GET {registry}/v1/profiles/{pack}/{version}/{profile}`. **Task R3 (the Worker) is expected to
resolve `profile-id` via its own registry client (GraphQL) and pass it through unchanged.**

## Byte-identity

The runner does not reimplement or duplicate verification logic from `crates/cli` or `crates/core` —
neither crate was modified for this task. It depends on `dry-core` as an ordinary path dependency
(`dry-core = { path = "../../crates/core" }`) and mirrors the exact call sequence
`crates/cli/src/main.rs`'s `Cmd::ReviewGcode` arm uses (also documented and exercised identically by
the `crates/cloud` feasibility spike's `review_import_params`):

1. `profile.gcode_import_params()`, with `line_width`/`layer_height` forced to `0.45`/`0.2` if the
   profile doesn't already specify them (mirrors `gcode_review_params` in `crates/cli/src/main.rs` —
   raw g-code carries no line-width/layer-height of its own).
2. `import_gcode_reader_with_map(file, &params)`.
3. `profile.contracts()` (no CLI-flag overrides — the HTTP endpoint has none to apply).
4. `verify(&imported.toolpath, &contracts)`, wrapped in `catch_unwind` per the task's Global
   Constraints (`engine-error` on panic).
5. `serde_json::to_string_pretty(&report) + "\n"` — the same bytes `Cmd::Verify`'s `--json` flag
   prints via `println!` in `crates/cli/src/main.rs`.

`tests/handler.rs`'s `verify_report_is_byte_identical_to_a_direct_dry_core_call` test proves the HTTP
round trip (query parsing, body-streaming-to-tempfile, profile fetch, response encoding) introduces
zero byte drift versus calling that same sequence directly against the original fixture file — i.e.
the network/tempfile/HTTP layers this crate adds are transparent to the report bytes.

## Memory

The cloud spike (`docs/superpowers/specs/2026-07-28-cloud-spike-findings.md`) found `dry-core`'s
g-code import path allocates roughly 43-50x the input's byte size in peak process memory (the parsed
line vector, the per-segment `Toolpath`, the source-line map `_with_map` builds). That is why the
spike recommended a Cloudflare Container (this crate) over a Workers isolate (128MB hard ceiling) for
anything past ~1MB. This container has 6GiB, so the import-time blowup is fine at the Worker's 100MB
cap — see `.superpowers/sdd/task-R2-report.md` for measured 1MB/50MB timings and peak memory.

The request body is streamed straight to a tempfile and reopened as a plain `std::fs::File` for
`import_gcode_reader_with_map` (which accepts any `Read`), so the raw upload is never additionally
held as one big in-memory buffer — only dry-core's own internal parse/verify allocations count
against the container's memory budget.

## Docker

The Dockerfile needs the **repo root** as its build context (Docker cannot `COPY` paths outside the
build context, and this crate's `dry-core` path dependency reaches outside
`containers/verify-runner/`; `crates/core`'s manifest also inherits `edition`/`rust-version`/etc. from
the root `Cargo.toml`'s `[workspace.package]`, which requires the whole root workspace — including
its other member manifests — to be present and parseable):

```sh
docker build -f containers/verify-runner/Dockerfile -t dry-verify-runner .
docker run --rm -p 8080:8080 dry-verify-runner
```

The root `.dockerignore` trims the context (drops `target/`, `.git`, `node_modules`, `docs/site`,
`py/.venv`).

## Local development

```sh
cd containers/verify-runner
cargo test              # handler tests, incl. byte-identity, against a stub registry on localhost
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run                # binds 0.0.0.0:8080
```
