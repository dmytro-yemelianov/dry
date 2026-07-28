# Dry Cloud — Project Handover (RESUMED 2026-07-28)

Owner paused the project after Task R3, then resumed it from this document. Task R4
is now complete; this remains the resume point for Task R5. Read it with:
- Spec: `docs/superpowers/specs/2026-07-28-dry-cloud-registry-design.md` (**Revision 2 section is authoritative**)
- Plan: `docs/superpowers/plans/2026-07-28-dry-cloud-mvp.md` (Revision 2 — tasks R1–R8)
- Spike findings: `docs/superpowers/specs/2026-07-28-cloud-spike-findings.md`
- Per-task reports: `.superpowers/sdd/task-*.md` on the machine that ran the work (gitignored; the ledger `.superpowers/sdd/progress.md` summarizes them)

## What this project is

Monetize Dry as an API-driven SaaS ("Dry Cloud"): accounts (RFC 8628 device flow) +
API keys, an async **verify-job API** that runs dry-core NATIVELY in a Cloudflare
Container (byte-identical reports to local `dry import-gcode` → `dry verify --json`),
usage metering with free quotas (billing = Phase 2), and `dry auth` / `dry cloud
verify` CLI commands. The **printer registry is NOT this project** — it lives in the
public repo `dmytro-yemelianov/dry-printer-registry` (built by a Codex agent, live at
`api.dry.yemelianov.dev`); Dry Cloud consumes it read-only via `REGISTRY_URL`.

## State: done and reviewed (branch `feat/dry-cloud`, all commits reviewed via SDD)

| Piece | Commits | State |
|---|---|---|
| Spike: dry-core in Workers | `1705c7b` | NO-GO >1MB (memory 43–50× input vs 128MB isolate) → **owner chose Containers** (`standard-2`, ≈$0 at MVP volume) |
| `services/cloud` auth | `5555032`,`57e6738` | Device flow, bearer tokens + API keys (SHA-256-only storage), Turnstile w/ prod fail-closed guard, atomic single-use grant (D1 `grants` table), IP rate limits. 22 tests |
| `containers/verify-runner` | `e546960`,`23e6a2c`,`6c96119` | Rust axum shim over dry-core; **byte-identity proven against the real CLI binary** (incl. stripped-defaults fixture); SSRF allowlist (`ALLOWED_REGISTRY_HOST`, fail-closed, https-only + localhost escape); non-root (uid 10001); body-limit 413→422 envelope; 12 tests. Docker: build from REPO ROOT: `docker build -f containers/verify-runner/Dockerfile .` |
| Jobs API + queue + container dispatch | `c511836`,`5273c84`,`e944b79` | POST /v1/jobs/verify (cap→quota→strict-version-resolve→R2→D1→queue order), queue consumer with redelivery idempotency, partial-failure handling (`queue-send-failed` stage, orphan-R2 delete), owner-scoped GET with inlined report, DLQ config, itest with docker-baseline cleanup. 43 tests total in services/cloud |
| Usage metering + quotas | `f0209f5` | Every authenticated cloud request records a `usage_events` row (`job\|keys\|auth`); the `jobs` table is the single canonical monthly job-quota source; quota failures return the specified 429 + UTC-month `Retry-After`; `GET /v1/usage` reports monthly jobs/bytes and quotas. 62 tests total in services/cloud; `npm run check` green. |
| Old licensing product | `8a11be2` (`crates/license`) | SUPERSEDED spec; crate parked for future pack-signing. Do not delete |

Also relevant, ALREADY SHIPPED (not on hold): **v0.4.0 released** (tag + 10-asset GitHub
Release, 2026-07-28); the portfolio site (yemelianov.dev) is live — separate repo.

## Next task at resume: R5 (CLI `dry auth` + `dry cloud verify`)

Plan section `### Task R5`. Add `dry auth login|status|logout` and
`dry cloud verify` with mock-server integration tests; mirror
`crates/cli/src/printer_registry.rs` networking idioms and the Moonraker
`TcpListener` test-server pattern. Then R6 (docs), R7 (deploy — see below), and
R8 (v0.5.0 release).

## Deferred debts that MUST close before/at deploy (R7)

1. **Real-runner byte-identity E2E** — locally blocked by Cloudflare's container
   proxy-sidecar networking (host-bound stub registry unreachable over http from the
   container netns). Must run against the real deployment: submit a job, diff the
   report against local CLI output.
2. `wrangler deploy --dry-run` (+ `--env dev`) after the DLQ config change (skipped
   locally: dev-env dry-run triggers a container build).
3. Create infra: D1 `dry-cloud`, KV, queues `verify-jobs` + `verify-jobs-dlq` (+`-dev`
   variants), R2 bucket, Turnstile site, container deploy, hostname
   `cloud.dry.yemelianov.dev`; secrets/vars per wrangler.jsonc comments —
   `REGISTRY_URL` must match the runner's `ALLOWED_REGISTRY_HOST` (coupling documented
   in wrangler.jsonc).
4. Real `TURNSTILE_SITE_KEY`/`TURNSTILE_SECRET_KEY` via `wrangler secret put` (prod
   fails closed until set — deliberate).

## Known minor debts (accepted, listed for the final whole-branch review)

- services/cloud: broad catch on grants INSERT maps all D1 errors to `expired_token`
  unlogged; `grants` table never cleaned; `Retry-After` on rate limits is always the
  full window; quota check TOCTOU at the boundary; `markJobError` itself unguarded.
- runner: 413→422 rewrite keyed on status only; no percent-encoding of pack/version/
  profile URL params; profile sha256 verification TODO; DNS-rebinding residual in the
  host allowlist.
- itest jobs-local.sh: `wrangler dev` launch line hardcodes `--port 8787` (kill
  pattern matches it literally — fine, but single-source it if touched).

## Operating notes for whoever resumes

- SDD process: fresh implementer subagent per task + reviewer gate; ledger at
  `.superpowers/sdd/progress.md`. Final whole-branch review still owed before merge.
- **Owner's machine-load rule:** nice -n 10 + `-j 4` for heavy builds; build the
  Docker image at most once; kill spawned process TREES (wrangler respawns workerd);
  one heavy agent at a time (Codex often runs concurrently).
- **gh account dance:** `gh auth switch -u dmytro-yemelianov` → push → switch back to
  `miwaniza`. "Repository not found" = wrong account active.
- Coordinate with the Codex agent on registry-side changes (schemas, publish flow,
  future pack signing) — that's its lane; Dry Cloud only consumes.
