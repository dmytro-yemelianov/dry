# Dry Cloud MVP Implementation Plan — REVISION 2

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Revision note:** Revision 1's Tasks 0–11 were re-planned after (a) the Task 0 spike
verdict (in-Worker verify memory-capped at ~1 MB → owner chose **Cloudflare Containers**
for verify compute) and (b) discovery that the **printer registry already exists and is
live** (`dmytro-yemelianov/dry-printer-registry` → `api.dry.yemelianov.dev`; CLI
`dry printer search|inspect|resolve` landed in `c426149`). See the spec's **Revision 2**
section — it is authoritative. Old Tasks 1–3/5 (signing, pack schema/types, seeds,
registry APIs) are deferred/covered and removed from this plan.

**Goal:** Ship the Dry Cloud authenticated compute layer: device-flow accounts + API
keys, an async verify-job API running dry-core NATIVELY in a Cloudflare Container,
usage metering with free quotas, `dry auth` + `dry cloud verify` CLI commands, and docs.

**Architecture:** `services/cloud/` — a TypeScript Worker (Hono-less plain router or
itty; match the registry service's conventions where sane) with D1 (accounts, tokens,
jobs, usage), KV (device codes), R2 (uploads + reports), a Queue, and a Durable-Object-
backed Container (`@cloudflare/containers`) running `containers/verify-runner/` — a
small Rust axum HTTP shim over the same dry-core import+verify path the CLI uses.
Profiles resolve through the PUBLIC registry REST (no duplicated resolution logic).

**Tech Stack:** TS Worker + `@cloudflare/vitest-pool-workers` (the `cloudflareTest()`
plugin API — NOT `defineWorkersConfig`, removed in 0.18.x), `@cloudflare/containers`,
D1/KV/R2/Queues/Turnstile, Rust (axum) container, wrangler 4.

## Global Constraints

- Everything from the spec's Revision 2 division of labor: registry concerns belong to
  the public repo — this plan builds ONLY accounts/keys/jobs/metering/CLI-cloud/docs.
- Registry consumption is via `https://api.dry.yemelianov.dev` REST/GraphQL; base URL
  configurable everywhere (`REGISTRY_URL` var in the Worker; runner receives it).
- Auth invariants (unchanged from Rev 1): RFC 8628 device flow; tokens
  `dry_at_<43·b64url>`, keys `dry_key_<43·b64url>`; SHA-256 hashes only in D1;
  timing-safe compare; KV codes TTL 600 s; user_code `XXXX-XXXX` from alphabet
  `BCDFGHJKLMNPQRSTVWXZ23456789`; Turnstile on `/activate`
  (`TURNSTILE_DEV_BYPASS=1` honored only in dev).
- **Byte-identity invariant:** a cloud verify report must be byte-identical to local
  `dry verify --json` with the same profile+input — asserted by an automated test.
- Verify-runner contract: `POST /verify?pack=<id>&version=<ver>` with raw G-code body;
  the runner fetches the resolved profile from the registry, runs dry-core, returns
  `200 {report}` or `4xx/5xx {error, stage}` with stages
  `profile-unavailable | input-invalid | engine-error`.
- Upload cap: 100 MB (container has 6 GiB; the Worker enforces `Content-Length`).
  Worker→container transfer for large bodies is a KNOWN RISK (open CF issue on
  >10–15 MB transfers): Task R3 must test 1/10/50 MB locally and, if large bodies
  fail, switch to the documented fallback (runner pulls the object via a short-lived
  signed Worker URL) — record which path shipped.
- Quotas (vars): `QUOTA_JOBS_PER_MONTH=20`, `QUOTA_KEYS=1`. Public registry reads are
  NOT metered here (registry service owns reads).
- No live network in automated tests EXCEPT localhost (wrangler dev, docker). Registry
  calls in tests hit a stubbed local registry fixture server, never production.
- CLI: cloud commands mirror `crates/cli/src/printer_registry.rs` idioms (ureq,
  `--source`-style URL override → `DRY_CLOUD_URL`); `DRY_TOKEN` env precedence; config
  dir `$XDG_CONFIG_HOME/dry/` XDG-first. `die()`/exit-code conventions
  (`crates/cli/src/main.rs:466-469`). Offline commands must never touch the network.
- Branch `feat/dry-cloud`; commit per task; **do not modify** registry-owned files
  (`crates/cli/src/printer_registry.rs` may be EXTENDED only where a task says so);
  gh account dance for pushes.
- Versioning: v0.5.0 at the release task only.

---

### Task R1: `services/cloud` scaffold + device-flow auth + API keys

**Files:**
- Create: `services/cloud/{package.json,wrangler.jsonc,tsconfig.json,schema.sql,vitest.config.ts}`, `services/cloud/src/{index.ts,auth.ts,tokens.ts,activate.ts}`, `services/cloud/test/auth.test.ts`

**Interfaces (HTTP — consumed by Task R5 CLI):**
- `POST /v1/auth/device` → `{device_code, user_code, verification_uri, verification_uri_complete, expires_in: 600, interval: 5}`; KV `dev:<device_code>` + `usr:<user_code>` TTL 600.
- `GET|POST /activate` — HTML form (code + email), Turnstile-verified server-side; approve → accounts upsert + KV state `approved`.
- `POST /v1/auth/token` — RFC 8628 grant `urn:ietf:params:oauth:grant-type:device_code`: `authorization_pending` / `slow_down` (poll < interval) / `expired_token` / success `{access_token, token_type:"Bearer"}`; single-use (KV deleted on grant).
- `POST /v1/keys` (Bearer) → `{id, key}` shown once; `GET /v1/keys` (ids/labels/created only); `DELETE /v1/keys/{id}`. `QUOTA_KEYS` enforced.
- `GET /v1/me` → `{account_id, email, created_at}`.
- Shared middleware `requireAuth(req, env)` → account row via SHA-256 lookup over `tokens`; `crypto.subtle.timingSafeEqual` on hash compare; SECURITY_HEADERS on every response (mirror the yemelianov-dev pattern: mirror-in-code + drift test not needed here since no `_headers` file — just a constants module).

`schema.sql`:
```sql
CREATE TABLE accounts (id TEXT PRIMARY KEY, email TEXT UNIQUE NOT NULL, created_at TEXT DEFAULT (datetime('now')));
CREATE TABLE tokens (hash TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id), kind TEXT NOT NULL CHECK (kind IN ('at','key')), label TEXT, created_at TEXT DEFAULT (datetime('now')), revoked INTEGER DEFAULT 0);
CREATE TABLE jobs (id TEXT PRIMARY KEY, account_id TEXT NOT NULL, status TEXT NOT NULL, pack_id TEXT, pack_version TEXT, input_r2 TEXT, report_r2 TEXT, error TEXT, stage TEXT, created_at TEXT DEFAULT (datetime('now')), finished_at TEXT);
CREATE TABLE usage_events (id INTEGER PRIMARY KEY AUTOINCREMENT, account_id TEXT NOT NULL, route TEXT NOT NULL, bytes INTEGER DEFAULT 0, at TEXT DEFAULT (datetime('now')));
CREATE INDEX usage_by_account_day ON usage_events (account_id, at);
```

- [ ] TDD via `vitest-pool-workers`: failing tests for the full device-flow state machine (pending→approve→grant→single-use), slow_down timing, expired codes, key lifecycle + quota, timing-safe auth middleware (bad token 401, revoked 401), Turnstile bypass only when var set. → implement → green → `npm run check` (tsc) → commit `feat(cloud): device-flow auth, bearer tokens, API keys (services/cloud)`.

---

### Task R2: `containers/verify-runner` — native dry-core verify shim

**Files:**
- Create: `containers/verify-runner/{Cargo.toml,src/main.rs,Dockerfile,README.md}` (workspace-EXCLUDED member, like crates/wasm — it builds in Docker, not in the workspace lock)

**Interfaces:**
- `POST /verify?pack=<id>&version=<ver>&registry=<base-url>` — body: raw G-code (streamed to disk under `/tmp`, NOT buffered fully in memory where avoidable); fetches `GET <registry>/v1/profiles/...` resolved profile (exact REST path per `docs/19-printer-registry-api.md` — read it); runs the same dry-core import+verify calls as the CLI review path; responds `200` with the EXACT `serde_json::to_string_pretty(&report) + "\n"` bytes the CLI writes, or `{error, stage}` per the Global Constraints stages.
- `GET /healthz` → `{ok:true}`.
- Dockerfile: multi-stage (rust:1.88 build → debian-slim runtime), binds `0.0.0.0:8080`.

- [ ] TDD: Rust unit tests for the handler logic with a stubbed profile server (spawn `std::net` mock like `crates/cli/tests/cli.rs:86-92` does for Moonraker) + a conformance fixture gcode; the byte-identity test: run the runner handler fn and `dry-cli`'s verify on the same inputs, assert identical bytes. → implement → `docker build` + `docker run` + curl smoke (1 MB and 50 MB synthetic files from the spike's generator) → commit `feat(cloud): native verify-runner container`.

---

### Task R3: jobs API + queue + container dispatch

**Files:**
- Modify: `services/cloud/` — `src/jobs.ts`, `src/container.ts` (`VerifyContainer extends Container` from `@cloudflare/containers`, `sleepAfter` short), wrangler.jsonc (queue producer/consumer `verify-jobs`, R2 `STORAGE`, container config with the runner image, DO binding + migration)
- Test: `services/cloud/test/jobs.test.ts` + `services/cloud/itest/jobs-local.sh`

**Interfaces:**
- `POST /v1/jobs/verify?pack=<id>&version=<ver>` (Bearer; raw body ≤100 MB) → R2 `uploads/<job_id>` → jobs row `queued` → queue send → `202 {id, status_url}`.
- Queue consumer: job → container stub (`getByName(job_id)` for isolation) → stream input from R2 into `POST /verify` → persist report to R2 `reports/<job_id>.json` + `done`, or error+stage; `message.ack()` always after a terminal state (retry only on container-start failures, max 2).
- `GET /v1/jobs/{id}` (Bearer, owner-only) → `{id, status, pack_id, created_at, finished_at, report?, error?, stage?}` — report inlined when done.
- Failure taxonomy from the Global Constraints; `too-large` rejected at POST time via Content-Length.

- [ ] Steps: vitest tests with a FAKE container binding (inject a fetch-stub for the DO/container path — unit-level) → implement → **local integration** `itest/jobs-local.sh`: `wrangler dev` with containers enabled (requires local Docker; document `docker info` precheck) + the stub registry fixture; run 1 MB, 10 MB, 50 MB — record which Worker→container transfer path worked (direct stream vs signed-URL fallback per Global Constraints) → byte-identity assertion vs local CLI → commit `feat(cloud): async verify jobs on containers`.
- [ ] If local containers-in-wrangler-dev cannot run in this environment after genuine attempts: mark the itest `SKIPPED-LOCAL` with exact blocker, keep unit tests green, and flag DONE_WITH_CONCERNS — the E2E then happens at deploy (Task R7) before launch.

---

### Task R4: usage metering + quotas

- Modify: `services/cloud/src/{usage.ts,index.ts}`; tests.
- Every authed request → `usage_events` row (route class `job|keys|auth`); `QUOTA_JOBS_PER_MONTH` checked BEFORE job creation (UTC month window) → `429 {"error":"quota_exceeded","usage_url":"/v1/usage"}` + `Retry-After`; `GET /v1/usage` → `{month:{jobs, bytes}, quotas}`.
- [ ] TDD (window math incl. month boundaries; 429 shape; usage endpoint) → implement → commit `feat(cloud): usage metering and job quotas`.

---

### Task R5: CLI — `dry auth` + `dry cloud verify`

**Files:**
- Modify: `crates/cli/src/main.rs` (new `Auth`, `Cloud` subcommands; helpers in new `crates/cli/src/cloud.rs`), `crates/cli/Cargo.toml` (reuse the ureq dep added by the registry work; add `dirs = "5"`)
- Test: `crates/cli/tests/cloud.rs`

**Interfaces (user-visible):**
- `dry auth login [--cloud-url <u>]` — device flow: print code + `verification_uri_complete`, poll per `interval` honoring `slow_down`; store token at `$XDG_CONFIG_HOME/dry/cloud-token` (0600). `dry auth status` (whoami via `/v1/me` + usage one-liner), `dry auth logout` (delete file). `DRY_TOKEN` env > file; `DRY_CLOUD_URL` > default.
- `dry cloud verify <file> --printer <pack-id> [--pack-version <v>] [--json]` — POST body, poll `GET /v1/jobs/{id}` (1 s→5 s backoff, 10 min cap), human summary (findings count, verdict) or full `--json` report; nonzero exit mirrors local verify semantics (exit 1 on error-severity findings — match how `run_upload` tallies at `main.rs:1978-1983`).
- Unauthenticated → `die("not logged in — run `dry auth login`")` (exit 2). No other command touches the network.

- [ ] TDD: integration tests against a MOCK cloud server (std TcpListener mock like the Moonraker tests — no wrangler dependency): device-flow happy path + slow_down, token precedence, verify submit/poll/exit-code, offline-guarantee (assert `dry verify` runs with `DRY_CLOUD_URL` pointing at a dead port). → implement → workspace green → commit `feat(cli): dry auth and dry cloud verify`.

---

### Task R6: docs

- Create `docs/site/cloud/{index.md,api.md,quickstart-cli.md,quickstart-integrations.md}`; nav/sidebar; boundary allowlist prefix `docs/site/cloud/`. Content: honest pricing state (free quotas now, usage billing later), full endpoint reference with examples lifted from the test transcripts, CLI quickstart (login → printer resolve → cloud verify), curl quickstart for integrations (key create → job submit → poll). Link registry docs (public repo) rather than duplicating them. Note the email-asserted-not-verified MVP limitation.
- [ ] Write → `DRY_DOCS_MODE=public bash docs/site/build.sh` green → commit `docs(site): Dry Cloud auth and verify-job API docs`.

---

### Task R7: USER — infra + deploy (controller-assisted checklist)

1. `wrangler d1 create dry-cloud` + schema; KV namespace; `wrangler queues create verify-jobs`; R2 bucket `dry-cloud`; fill IDs into wrangler.jsonc.
2. Turnstile site (activate page) → keys as secrets.
3. Container: `wrangler deploy` builds/pushes the runner image (containers config in wrangler.jsonc); confirm instance type `standard-2`.
4. Hostname: `cloud.dry.yemelianov.dev` custom domain on the Worker (zone exists).
5. Vars: `REGISTRY_URL=https://api.dry.yemelianov.dev`, quotas.
6. Prod smoke: real device-flow login from a terminal, one real verify job (a Benchy-scale file) — confirm the report matches a local run byte-for-byte; check `/v1/usage` incremented; verify container scale-to-zero after `sleepAfter`.

### Task R8: v0.5.0 release + launch

- CHANGELOG (cloud commands, auth, verify jobs; registry integration credit already in c426149's wording), version bumps (`scripts/check-version.sh v0.5.0`), support-matrix row (cloud: best-effort, no SLA).
- Full gates: `cargo test --workspace`, services/cloud tests, docs public build.
- Tag + release.yml watch; docs live; announce line: registry (free, public) + cloud verify (free quota, usage-priced later).

---

## Self-review notes

- Spec Rev-2 coverage: TS worker ✓ (R1), container runner + byte-identity ✓ (R2/R3), registry-resolved profiles ✓ (R2), transfer-size risk + fallback ✓ (R3), metering scoped to jobs ✓ (R4), CLI auth/verify + offline guarantee ✓ (R5), docs ✓ (R6), deploy/hostname/smoke ✓ (R7), release ✓ (R8). Deferred items (signing, pack loader, seeds, read metering) recorded in the spec — deliberately absent here.
- Consistency: stage taxonomy identical R2/R3; token/key formats identical R1/R5; quota var names R1/R4/R7; `DRY_CLOUD_URL`/`DRY_TOKEN` R5 only.
- Judgment left to implementers: exact registry REST profile path (read docs/19 + the live /schema.graphql); which Worker→container transfer path survives the R3 size test; matching the registry service's TS conventions where they exist.
