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
cap: 200MB by default (`tower_http::limit::RequestBodyLimitLayer` — NOT
`axum::extract::DefaultBodyLimit`, which only caps `Bytes`-based extractors and has no effect on a
handler that reads the raw body itself, like this one), overridable via the `MAX_BODY_BYTES` env var
(parsed once at router-build time — mainly so tests can install a tiny cap) — the Worker enforces a
100MB `Content-Length` cap upstream; 200MB is deliberate headroom, not the product limit.

- `200` — body is `serde_json::to_string_pretty(&report) + "\n"`, byte-identical to what
  `dry verify --json` prints locally for the same profile+input (see "Byte-identity" below).
- `4xx/5xx` — `{"error": "<message>", "stage": "<stage>"}`:
  - `502 profile-unavailable` — the registry fetch failed or was refused: network error, non-2xx, an
    unparseable profile body, or the SSRF allowlist rejecting the registry URL (see "Registry
    allowlist" below).
  - `422 input-invalid` — dry-core rejected the g-code with a `Result` error (not a panic) — e.g.
    malformed/non-UTF8 input — OR the request body exceeded `MAX_BODY_BYTES`.
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

## Registry allowlist (SSRF)

`fetch_profile` refuses to fetch a profile from anywhere except the single operator-configured
registry host. The host is read from `ALLOWED_REGISTRY_HOST` — there is **no default fallback that
accepts everything**; if the env var is unset, every fetch is refused (`502 profile-unavailable`,
fail closed). The registry base URL must be `https://` and its host must equal
`ALLOWED_REGISTRY_HOST` exactly, EXCEPT plain `http://` is additionally allowed when the host is
`127.0.0.1` or `localhost` — a deliberate dev/test escape hatch so a local stub registry (which
doesn't terminate TLS) can be exercised without weakening the production rule (every other host must
be `https`).

## Byte-identity

The runner does not reimplement or duplicate verification logic from `crates/cli` or `crates/core` —
neither crate was modified for this task. It depends on `dry-core` as an ordinary path dependency
(`dry-core = { path = "../../crates/core" }`) and mirrors the exact call sequence local
`dry import-gcode <file> --profile <p> -o <ir>` followed by `dry verify <ir> --profile <p> --json`
performs — the **plain** import path, NOT the `review-gcode` path (`crates/cli/src/main.rs`'s
`Cmd::ReviewGcode` arm forces `line_width`/`layer_height` to `0.45`/`0.2` via `gcode_review_params`
when a profile omits them; that forced-defaults behavior does NOT apply to `import-gcode` -> `verify`
and must not apply here either):

1. `profile.gcode_import_params()` — `crates/cli/src/main.rs:1779-1798`'s plain `gcode_import_params`
   composition, with no CLI-flag overrides (the HTTP endpoint has none to apply) and no forced
   defaults. Absent `process.line_width`/`process.layer_height` stay `None`, exactly as they would
   for `dry import-gcode --profile <p>` with no `--line-width`/`--layer-height` flags.
2. `import_gcode_reader_with_map(file, &params)`.
3. `profile.contracts()` (no CLI-flag overrides — the HTTP endpoint has none to apply).
4. `verify(&imported.toolpath, &contracts)`, wrapped in `catch_unwind` per the task's Global
   Constraints (`engine-error` on panic).
5. `serde_json::to_string_pretty(&report) + "\n"` — the same bytes `Cmd::Verify`'s `--json` flag
   prints via `println!` in `crates/cli/src/main.rs`.

`tests/handler.rs`'s `verify_report_is_byte_identical_to_the_real_cli` test (and its "profile omits
process defaults" sibling, which pins the previously-masked forced-defaults divergence) build and
shell out to the REAL, compiled `dry` binary — `dry import-gcode` piped into `dry verify --json` in a
tempdir — and byte-compare its stdout against the runner's own HTTP response for the same
profile+gcode. This is a genuine external ground truth, not a comparison of the runner against
itself.

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
docker run --rm -p 8080:8080 -e ALLOWED_REGISTRY_HOST=api.dry.yemelianov.dev dry-verify-runner
```

The root `.dockerignore` trims the context (drops `target/`, `.git`, `node_modules`, `docs/site`,
`py/.venv`).

The runtime stage runs as a fixed-uid, non-root, non-login system user (`runner`, uid `10001`), not
root — `USER runner` in the Dockerfile. `/tmp` ships world-writable (with the sticky bit) in the
`debian:bookworm-slim` base image for every user, not just root, so the non-root user can still
stream request bodies there; no extra permission setup is needed.

## Local development

```sh
cd containers/verify-runner
cargo test              # handler tests, incl. real-CLI byte-identity, against a stub registry on localhost
cargo clippy --all-targets -- -D warnings
cargo fmt --check
ALLOWED_REGISTRY_HOST=api.dry.yemelianov.dev cargo run   # binds 0.0.0.0:8080
```

`cargo test`'s byte-identity tests build the real `dry` CLI binary once (`cargo build -p dry-cli
--quiet` against the main engine workspace at the repo root) and shell out to it — the first test run
takes noticeably longer for that reason.

### Running a `/verify` against a local stub registry

The `http://` escape hatch in `validate_registry_url` exists for exactly this: any host other than
`127.0.0.1`/`localhost` must be `https`, so a stub registry that terminates no TLS still works.

```sh
# a registry is just static files at /v1/profiles/{pack}/{version}/{profile}
mkdir -p /tmp/reg/v1/profiles/marlin-i3/1.0.0
cp conformance/profile-matrix/marlin-pla-i3/profile.json /tmp/reg/v1/profiles/marlin-i3/1.0.0/pla-0.4
(cd /tmp/reg && python3 -m http.server 8099 &)

ALLOWED_REGISTRY_HOST=127.0.0.1 cargo run -p dry-verify-runner &
curl -X POST --data-binary @examples/part.gcode \
  'http://127.0.0.1:8080/verify?pack=marlin-i3&version=1.0.0&profile=pla-0.4&registry=http://127.0.0.1:8099'
```

The report is byte-identical to `dry import-gcode … | dry verify --json` for the same input — which
is the point of the runner, and what `verify_report_is_byte_identical_to_the_real_cli` pins. Note the
CLI stamps its licence mode into the report, so compare against an *unlicensed* CLI run (e.g. with
`HOME` pointed at an empty directory) unless the runner has a licence configured too.

### Running the whole Cloudflare shape locally

`wrangler dev` in `deploy/cloudflare` runs the real thing — Worker, Durable Object and this
container image under Docker:

```sh
cd deploy/cloudflare && npx wrangler dev      # first run builds the image; several minutes
curl http://127.0.0.1:8787/healthz            # {"ok":true}
curl -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8787/metrics   # 404: deliberately not proxied
```

`/verify` needs a registry the *container* can reach, and inside the container `127.0.0.1` is the
container itself, not your machine. Rather than weaken the SSRF guard for dev, put the stub in the
container's own network namespace:

```sh
# copy wrangler.jsonc with ALLOWED_REGISTRY_HOST=127.0.0.1, and INSTANCES=1 in src/index.ts so
# both requests land on the same instance, then:
CID=$(docker ps --format '{{.ID}} {{.Image}}' | grep -i verifyrunner | awk '{print $1}' | head -1)
docker run -d --name stub --network "container:$CID" -v /tmp/reg:/srv:ro -w /srv \
  python:3-alpine python3 -m http.server 8099 --bind 0.0.0.0
```

Doing this is how the `/healthz`-starts-the-container-bare defect was found: a container's
environment is fixed when its process starts, so whichever route starts an instance decides it for
that instance's whole life. `envVars` therefore lives on the `VerifyRunner` class, never on a single
`startAndWaitForPorts` call — see the doc comment there, and the CI guard in `deploy-verify.yml`.
