# Dry Cloud MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the Dry Cloud MVP per `docs/superpowers/specs/2026-07-28-dry-cloud-registry-design.md`: a workers-rs API (capability-pack registry + one async verify endpoint running dry-core as wasm), device-flow auth with API keys, usage metering with free quotas, signed packs verified offline by the CLI, seeded content, and API docs.

**Architecture:** New crates: `crates/pack` (Pack v1 types/validation/profile-resolution), `crates/signing` (renamed from `crates/license`, generalized to detached signatures), `crates/cloud` (the workers-rs API: fetch router + queue consumer, D1/R2/KV). CLI gains `auth`, `printer`, `cloud verify` commands behind a default-on `cloud` feature (ureq, mirroring `dry-llm`'s HTTP pattern). Seeds derive from the committed golden profiles.

**Tech Stack:** workers-rs (`worker` crate), D1, R2, Queues, KV, Turnstile; Rust end-to-end; wrangler for dev/deploy; JSON Schema + the existing Python validator for pack goldens.

**Task 0 is a feasibility spike.** Its FINDINGS doc (`docs/superpowers/specs/2026-07-28-cloud-spike-findings.md`) is a binding interface for Tasks 5–7: worker crate version pin, measured verify throughput, the MVP upload cap, and any workers-rs API deviations from this plan's code sketches. Where a later task's code conflicts with FINDINGS, FINDINGS wins — note the adaptation in the task report.

## Global Constraints

- The engine stays pure: `dry-core` gets NO new deps, NO network, NO cloud awareness. Packs resolve to `dry-profile-v1` and enter the engine through existing interfaces only.
- The CLI without login keeps every existing offline capability; cloud commands fail with clear errors when unauthenticated (exit 2, `die()` convention at `crates/cli/src/main.rs:466-469`).
- Auth: RFC 8628 device flow; access tokens are opaque `dry_at_<43 chars base64url>`, API keys `dry_key_<43 chars>`; ONLY SHA-256 hashes stored (D1), timing-safe comparison; device/user codes live in KV with 600s TTL; user_code format `XXXX-XXXX` from the unambiguous alphabet `BCDFGHJKLMNPQRSTVWXZ23456789`.
- Every authenticated request writes a `usage_events` row; quota exhaustion = 429 + `Retry-After` + JSON pointing at `/v1/usage`. Free quotas (config vars, not code): 500 registry reads/day, 20 verify jobs/month, 1 API key.
- Pack signing: registry Ed25519 key (`SIGNING_KEY_PKCS8_B64` secret, `KEY_ID` var — the Task-2-committed TEST keypair for dev/tests; production key via ceremony in Task 10). Signature is detached over the exact pack JSON bytes as served; CLI verifies BEFORE parsing.
- No live network in automated tests. Worker integration tests run against `wrangler dev` spawned locally; CLI cloud tests point `DRY_CLOUD_URL` at it.
- MVP upload cap: 50 MB unless Task 0's FINDINGS lowers it.
- Versioning: this work targets **v0.5.0**; do not bump manifests until the release-prep task.
- Branch: `feat/dry-cloud` (rename/continue from `feat/commercial-license` which already carries the crates/license commit). Commit after every task. gh pushes need the account dance (`gh auth switch -u dmytro-yemelianov` → push → `gh auth switch -u miwaniza`).
- workers-rs conventions (verified against current docs): `#[event(fetch)]` + `Router`, `#[event(queue)]` + `MessageBatch<T>`, `ctx.env.d1("DB")`, `env.queue("...")`, `ctx.kv("...")`, `ctx.secret("...")`/`ctx.var("...")`; wrangler build via `worker-build --release`.

---

### Task 0: Feasibility spike — dry-core verify inside a Worker

**Files:**
- Create: `crates/cloud/Cargo.toml`, `crates/cloud/src/lib.rs` (minimal), `crates/cloud/wrangler.toml`, `docs/superpowers/specs/2026-07-28-cloud-spike-findings.md`
- Modify: root `Cargo.toml` (`exclude` crates/cloud from the workspace, like `crates/wasm` — it targets wasm32 with its own lock profile; confirm the same exclusion pattern)

**Interfaces:**
- Produces: FINDINGS doc with (a) pinned `worker` crate version + worker-build version; (b) measured wall time of `dry-core` gcode import+verify at 1/10/50 MB inputs under `wrangler dev` (and whether the paid-plan CPU ceiling from current Cloudflare docs accommodates it in a queue consumer); (c) the confirmed MVP upload cap; (d) any deviations from this plan's workers-rs API sketches; (e) R2 multipart/body-size notes; (f) go/no-go on queue-consumer verify vs the Container fallback.

- [ ] **Step 1:** Scaffold `crates/cloud` with `worker = "<current>"`, `dry-core = { path = "../core" }`, a fetch handler exposing `POST /spike/verify` that reads the body, runs the same import+verify path the CLI's `review-gcode` uses (find the exact `dry_core` entry points by reading how `crates/cli/src/main.rs` review path calls core), and returns timing JSON `{bytes, parse_ms, verify_ms}` via `Date::now()` deltas.
- [ ] **Step 2:** `wrangler dev` it; POST the three sizes (generate synthetic gcode by repeating a golden fixture; exact commands in the findings doc); record timings. Also compile-check an `#[event(queue)]` consumer stub in the same crate.
- [ ] **Step 3:** WebFetch the current Cloudflare limits page for Workers CPU time (fetch + queue consumers, paid plan) and R2/request body limits; reconcile with measurements.
- [ ] **Step 4:** Write FINDINGS with the (a)–(f) verdicts. If no-go for queue-consumer verify: STOP — controller escalates to the owner with the Container fallback sizing before any further task runs.
- [ ] **Step 5:** Commit: `spike(cloud): dry-core verify on workers-rs — findings`

---

### Task 1: `crates/signing` — rename + generalize to detached signatures (TDD)

**Files:**
- Rename: `crates/license` → `crates/signing` (git mv; crate name `dry-signing`; update root `Cargo.toml` members)
- Modify: `crates/signing/src/lib.rs`
- Test: extend the existing 8 tests + cross_stack fixture paths

**Interfaces:**
- Produces (consumed by pack + CLI + cloud):
  - Everything existing (`verify_token` etc.) stays — delete nothing (token verification may return for future products; it's 150 lines).
  - NEW: `pub fn verify_detached(payload: &[u8], sig_b64url: &str, key_id: &str, keys: &[(&str, [u8; 32])]) -> Result<(), LicenseError>` — Ed25519 over the raw payload bytes.
  - NEW: `impl std::error::Error for LicenseError {}` and rename the error type to `SigningError` (type alias `LicenseError` kept for the existing tests).
  - The Task-2 (licensing plan) fixtures move with the crate: `crates/signing/tests/fixtures/` — **the test keypair `test-1` and `keygen.mjs`/`sign.mjs` under `tools/license-issuer/scripts/` move to `tools/cloud/scripts/`**.
- TDD: new tests first — detached verify happy path (JS-signed fixture over an arbitrary JSON doc), tampered payload, wrong key id.

- [ ] Steps: failing tests → `cargo test -p dry-signing` RED → implement → GREEN → `cargo clippy` + `cargo fmt` clean → workspace green → commit `refactor(signing): generalize license crate to detached pack signing`.

*(Note: Task 2 of the superseded licensing plan was never executed — `keygen.mjs`/`sign.mjs`/fixtures described there are CREATED here, following that plan's Step 1-2 code verbatim, at the new `tools/cloud/scripts/` path.)*

---

### Task 2: Capability Pack v1 — schema, types, profile resolution (TDD)

**Files:**
- Create: `crates/pack/Cargo.toml`, `crates/pack/src/lib.rs`, `spec/dry-pack-v1.schema.json`
- Modify: root `Cargo.toml` members; `tools/validate_reports.py` (or a sibling `tools/validate_packs.py` — follow the existing validator's structure)
- Test: `crates/pack/src/lib.rs` unit tests + `crates/pack/tests/goldens.rs`

**Interfaces:**
- `pub struct Pack` with the eight sections as typed sub-structs: `identity: Identity`, `toolhead: Toolhead`, `filaments: Vec<Filament>`, `macros: Vec<MacroDecl>`, `presets: Vec<Preset>`, `compatibility: Vec<CompatClaim>`, `observations: Vec<Observation>`, `provenance: Provenance` — field lists in the schema file are the source of truth; keep them MINIMAL (only what the seed content can honestly populate; every section's Vec may be empty).
- `Provenance` includes `trust: TrustLevel` (`draft|imported|dry-verified|hardware-observed|maintained`), `sources: Vec<String>`, `resolved_profile: serde_json::Value` (a full dry-profile-v1 document), `key_id: Option<String>` (signature travels OUTSIDE the pack bytes — detached).
- `pub fn resolve_profile(pack: &Pack) -> Result<serde_json::Value, PackError>` — returns `provenance.resolved_profile` after validating it parses as a profile via `dry_core`'s existing profile loader (find the exact loader fn used by `--profile` in the CLI).
- `pub fn validate(pack_json: &[u8]) -> Result<Pack, PackError>` — serde + semantic checks (semver id format `make/model@x.y.z`, trust-level/evidence consistency: `dry-verified`+ requires ≥1 source).
- Schema mirrors the structs (`additionalProperties: false` discipline, like the report schemas).
- TDD: golden round-trip test against ONE hand-written example pack committed at `conformance/packs/example/voron-2.4-350.json` (realistic content, sections 4/6/7 sparse).

- [ ] Steps: schema + failing tests → implement types/validate/resolve → GREEN → validator script extended and passing on the golden → workspace green → commit `feat(pack): capability pack v1 schema, types, profile resolution`.

---

### Task 3: Seed packs from golden profiles

**Files:**
- Create: `tools/cloud/seed-packs.rs` — a small binary (workspace `[[bin]]` in a `tools/cloud/Cargo.toml` member or a `crates/pack` example) converting `conformance/profile-matrix/*.json` (6 profiles) into packs; `conformance/packs/seed/*.json` (6+ packs)
- Test: extend `crates/pack/tests/goldens.rs` to validate every seed pack (drift-gated: regenerating must be byte-identical)

Requirements: identity from the profile's printer/firmware fields; `provenance.trust = "imported"`, `sources` pointing at the in-repo profile path; `resolved_profile` = the profile verbatim; filaments section populated from the profile's material data (the matrix is Marlin/Klipper/Duet × PLA/PETG/ABS — so 6 packs each with 1 filament, or 3 printers × 3 filaments if the matrix structure supports merging: decide from the actual files and justify in the report).

- [ ] Steps: read the actual profile-matrix files → converter → seeds generated + committed → validator green over all → commit `feat(pack): seed packs from the golden profile matrix`.

---

### Task 4: Cloud worker — auth (device flow + API keys)

**Files:**
- Modify: `crates/cloud/` (from the spike scaffold): `src/lib.rs` (router), new `src/auth.rs`, `src/db.rs`; `crates/cloud/schema.sql`; `crates/cloud/wrangler.toml` (D1 `DB`, KV `CODES`, vars, secrets docs)
- Test: `crates/cloud/tests/` native unit tests for pure logic (token generation/hashing, user-code alphabet, RFC 8628 state machine as a pure function) + `tools/cloud/itest/auth.sh` integration script against `wrangler dev`

**Interfaces (HTTP, consumed by Task 7 CLI):**
- `POST /v1/auth/device` (no auth) → `{device_code, user_code, verification_uri, verification_uri_complete, expires_in: 600, interval: 5}`; state in KV key `dev:<device_code>` TTL 600 (`{user_code, status: "pending"}`), reverse `usr:<user_code>`.
- `GET /activate` + `POST /activate` (HTML, Turnstile-protected): user enters code + email → email row upserted in `accounts`, KV state → `{status:"approved", account_id}`.
  - MVP identity note: entering a deliverable email is asserted, not verified, at activation — verification email lands in Phase 2; record this in the API docs as a known limitation.
- `POST /v1/auth/token` (RFC 8628 polling): pending → `{"error":"authorization_pending"}`; approved → `{access_token: "dry_at_…", token_type: "Bearer"}` (hash → `tokens` row, KV entries deleted); expired → `{"error":"expired_token"}`; poll faster than `interval` → `{"error":"slow_down"}`.
- `POST /v1/keys` (Bearer auth) → `{key: "dry_key_…", id}` shown ONCE; `GET /v1/keys`; `DELETE /v1/keys/{id}`. Free tier: 1 active key (enforced).
- `GET /v1/me` → `{account_id, email, created_at}`.
- Auth middleware: `Authorization: Bearer dry_at_…|dry_key_…` → SHA-256 → D1 lookup → attach account to request data; timing-safe compare per Workers best practices.

`schema.sql`:
```sql
CREATE TABLE accounts (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE tokens (hash TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id), kind TEXT NOT NULL CHECK (kind IN ('at','key')), label TEXT, created_at TEXT DEFAULT (datetime('now')), revoked INTEGER DEFAULT 0);
CREATE TABLE packs (id TEXT PRIMARY KEY, version TEXT NOT NULL, title TEXT NOT NULL, trust TEXT NOT NULL, r2_key TEXT NOT NULL, sig TEXT NOT NULL, key_id TEXT NOT NULL, published_at TEXT DEFAULT (datetime('now')), yanked INTEGER DEFAULT 0);
CREATE TABLE jobs (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, status TEXT NOT NULL, pack_id TEXT, input_r2 TEXT, report_r2 TEXT, error TEXT, created_at TEXT DEFAULT (datetime('now')), finished_at TEXT);
CREATE TABLE usage_events (id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, route TEXT NOT NULL, bytes INTEGER DEFAULT 0, at TEXT DEFAULT (datetime('now')));
CREATE INDEX usage_by_account_day ON usage_events (account_id, at);
```

- [ ] Steps: schema + pure-logic unit tests (RED→GREEN native `cargo test -p dry-cloud --lib`) → router endpoints → `tools/cloud/itest/auth.sh` (spawn `wrangler dev --local`, curl the full device flow with a scripted activation POST — Turnstile bypassed via a `TURNSTILE_DEV_BYPASS=1` var honored ONLY when set, documented as dev-only) → commit `feat(cloud): device-flow auth, bearer tokens, API keys`.

---

### Task 5: Cloud worker — registry APIs + signed publish

**Files:**
- Modify: `crates/cloud/src/lib.rs`, new `src/registry.rs`; wrangler.toml (R2 `PACKS` bucket binding)
- Test: native unit tests for search/meta logic; extend `tools/cloud/itest/` with `registry.sh`

**Interfaces:**
- `POST /v1/packs` (owner: requires var `OWNER_ACCOUNT_ID` match — community publishing is out of MVP): body = pack JSON → `dry_pack::validate` (compiled into the worker) → sign detached (WebCrypto via worker's crypto or `dry-signing` with the secret key — use `dry-signing` + ed25519-dalek `SigningKey` from the `SIGNING_KEY_PKCS8_B64` secret decoded; add a `signing` feature to `dry-signing` exposing sign, compiled ONLY into the cloud crate) → R2 put (`packs/<id>/<version>.json`) → D1 meta row with `sig`, `key_id`.
- `GET /v1/packs?q=` → D1 LIKE search over id/title (MVP; FTS later) → `[{id, version, title, trust}]`.
- `GET /v1/packs/{id}` → `{meta, sig, key_id, pack: <full JSON from R2>}`.
- `GET /v1/packs/{id}/profile` → the resolved profile JSON (server resolves via `dry_pack::resolve_profile`).
- All reads require auth (metering) — the generous free quota is the "public" tier; a `PUBLIC_READS=1` var can drop auth on GETs later without code restructure (route the check through one policy fn).

- [ ] Steps: unit tests → endpoints → itest publishing one seed pack then reading it back with signature verification in the test script (use `sign.mjs`-created dev key as the worker secret) → commit `feat(cloud): pack registry with signed publish`.

---

### Task 6: Cloud worker — async verify jobs (queue + wasm engine)

**Files:**
- Modify: `crates/cloud/`: new `src/jobs.rs`, queue consumer in `src/lib.rs`; wrangler.toml (queue producer+consumer `VERIFY_JOBS`, R2 `UPLOADS`, `REPORTS` prefixes on the PACKS bucket or a second bucket — per FINDINGS)
- Test: itest `jobs.sh` — small fixture gcode from `conformance/`, full lifecycle

**Interfaces:**
- `POST /v1/jobs/verify?pack=<id>` (Bearer; body = raw gcode, cap per FINDINGS; `Content-Length` enforced) → R2 put → D1 job row (`queued`) → queue send `{job_id}` → `202 {id, status_url}`.
- Queue consumer: load input from R2 → resolve pack → profile → run the same core import+verify path as the spike → `Report` JSON → R2 → D1 `done` (or the failure states from the spec: `upload-invalid|too-large|engine-error|timeout`) → `message.ack()`; engine panics caught (`std::panic::catch_unwind` around the pure-core call) → `engine-error`, ack (no infinite retry).
- `GET /v1/jobs/{id}` → `{id, status, pack_id, created_at, finished_at, report?: <inline JSON when done>, error?}`. Report must be byte-identical to what a local `dry verify --json` with the same profile produces (THE product claim — assert it in the itest by running the local CLI against the same fixture+profile and diffing).

- [ ] Steps: unit-test the job state machine natively → endpoints + consumer → itest lifecycle incl. the byte-identity diff → commit `feat(cloud): async verify jobs running dry-core in the worker`.

---

### Task 7: Usage metering + quotas

**Files:**
- Modify: `crates/cloud/src/lib.rs` (middleware), new `src/usage.rs`; wrangler.toml (vars `QUOTA_READS_PER_DAY=500`, `QUOTA_JOBS_PER_MONTH=20`, `QUOTA_KEYS=1`)

**Interfaces:**
- Every authed request: `usage_events` insert (route class: `read|job|auth|keys`).
- Quota check BEFORE the work; exceeded → `429` + `Retry-After` + `{"error":"quota_exceeded","usage_url":"/v1/usage"}`.
- `GET /v1/usage` → `{today: {reads}, month: {jobs}, quotas: {…}}` (two D1 aggregate queries).

- [ ] Steps: unit tests for the quota window math (day/month boundaries, UTC) → middleware → itest: exhaust a low test quota (`QUOTA_READS_PER_DAY=3` in dev vars) and assert the 429 shape → commit `feat(cloud): usage metering and free quotas`.

---

### Task 8: CLI cloud commands

**Files:**
- Modify: `crates/cli/Cargo.toml` (feature `cloud = ["dep:ureq", "dep:dry-pack", "dep:dry-signing"]`, default on — mirrors `moonraker`), `crates/cli/src/main.rs` (+~200 lines: `Auth`, `Printer`, `CloudVerify` commands — helpers in a new `crates/cli/src/cloud.rs` module to respect the main.rs size), `dirs` dep (token storage `$XDG_CONFIG_HOME/dry/cloud-token`, same XDG-first helper as the licensing plan's Task 3 sketch)
- Test: `crates/cli/tests/cloud.rs` — spawns `wrangler dev` (skip with a clear message when wrangler is absent: `#[ignore]`-gated + a CI env opt-in, following how moonraker tests handle their mock server)

**Interfaces (user-visible):**
- `dry auth login` (device flow: print `verification_uri_complete` + code, poll per `interval`, store token), `dry auth status`, `dry auth logout`. `DRY_TOKEN` env overrides the file; `DRY_CLOUD_URL` overrides the API base (default the production URL; tests point it at wrangler dev).
- `dry printer search <q>`, `dry printer show <id>` (meta + trust + sections summary), `dry printer add <id>`: GET pack → `dry_signing::verify_detached` against the embedded registry key set (same `PRODUCTION_KEYS`-style const + test-key escape hatch pattern from the licensing plan Task 3: `DRY_CLOUD_ALLOW_TEST_KEY=1`) → write pack + resolved profile under `$XDG_CONFIG_HOME/dry/printers/<id>/` → print the `--profile` path to use.
- `dry cloud verify <file> --printer <id>` → submit, poll, print human summary + report path/link; `--json` dumps the report.
- Offline guarantee: none of this executes unless the subcommand is explicitly one of these.

- [ ] Steps: failing integration tests (login flow against dev worker with `TURNSTILE_DEV_BYPASS`, printer add + signature verify, cloud verify round trip) → implement → GREEN → full workspace green → commit `feat(cli): cloud auth, printer packs, cloud verify`.

---

### Task 9: Docs — API reference + quickstarts

**Files:**
- Create: `docs/site/cloud/index.md` (what/why + pricing model honesty: free quotas now, usage billing later), `docs/site/cloud/api.md` (every endpoint: method, auth, request/response examples — from the itest scripts so they're real), `docs/site/cloud/quickstart-cli.md` (login → printer add → cloud verify), `docs/site/cloud/quickstart-integrations.md` (curl: create key, submit job, poll — the slicer/farm integration path)
- Modify: `.vitepress/config.ts` nav/sidebar; `scripts/check-public-boundary.mjs` `allowedPublicContentPrefixes` += `docs/site/cloud/`

- [ ] Steps: write pages (examples copied from actual itest transcripts) → `DRY_DOCS_MODE=public bash build.sh` green → commit `docs(site): Dry Cloud API reference and quickstarts`.

---

### Task 10: USER checklist — infra ceremony + deploy (controller-assisted)

1. Create production resources: `wrangler d1 create dry-cloud` + apply schema; `wrangler queues create verify-jobs`; R2 bucket `dry-cloud`; KV namespace `codes`; fill IDs into wrangler.toml.
2. Turnstile site for the `/activate` page → site/secret keys.
3. **Key ceremony:** `node tools/cloud/scripts/keygen.mjs prod-1` → `SIGNING_KEY_PKCS8_B64` Worker secret + offline backup; verifying key bytes → the CLI's embedded registry key const (commit).
4. Decide the API hostname (suggest `api.dry.yemelianov.dev` or wait for a product domain) + route/custom domain on the worker.
5. `wrangler deploy` from `crates/cloud`; smoke: device flow from a real terminal, seed-pack publish (owner token), one real verify job.
6. Owner publishes the 6 seed packs via `POST /v1/packs`.

### Task 11: Release + launch line

- v0.5.0 prep: CHANGELOG (cloud commands, pack format, registry), version bumps across manifests, `scripts/check-version.sh v0.5.0` green; support-matrix row for cloud endpoints (best-effort, no SLA).
- Full workspace + conformance + docs-public build green; tag `v0.5.0`, watch release.yml.
- Launch = API live with seeds + docs published + quickstarts verified against production + usage visible. Billing intentionally absent (Phase 2) — the docs say so plainly.

---

## Self-review notes

- Spec coverage: 8-section pack schema ✓ (T2), resolved-profile bridge ✓ (T2/T5), signing/trust ladder ✓ (T1/T5), device flow + API keys + Turnstile ✓ (T4), registry reads ✓ (T5), ONE verify endpoint incl. byte-identity claim ✓ (T6), metering/quotas/429/usage ✓ (T7), CLI commands + offline guarantee ✓ (T8), docs/quickstarts ✓ (T9), seeds ✓ (T3/T10), spike-first sequencing + Container fallback stop-rule ✓ (T0), no-live-network tests ✓ (constraints; itests run local wrangler dev), ceremony + deploy ✓ (T10), v0.5.0 ✓ (T11).
- Deliberate deviations from the writing-plans "complete code" bar: workers-rs handler bodies are specified as contracts + schema/SQL rather than full listings, because Task 0's FINDINGS is the designated authority on exact API shapes — each affected task says so explicitly. Stable parts (SQL, HTTP contracts, token formats, quota semantics, CLI surface) are fully specified.
- Type/name consistency: `dry-signing`/`verify_detached` used identically in T1/T5/T8; pack type names in T2/T3/T5/T6; env names (`DRY_TOKEN`, `DRY_CLOUD_URL`, `DRY_CLOUD_ALLOW_TEST_KEY`, `TURNSTILE_DEV_BYPASS`) consistent across T4/T8.
