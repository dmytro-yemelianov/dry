# Dry Cloud — API-Driven Registry + Engine SaaS (Design)

**Date:** 2026-07-28
**Status:** Approved design, pre-implementation. **Supersedes**
`2026-07-28-commercial-license-product-design.md` (binary license enforcement dropped by
owner decision; the `dry-license` crate built under that spec is repurposed for pack
signing).
**Owner:** Dmytro Yemelianov

## Purpose

Monetize Dry as an **API-driven SaaS** instead of license-enforcing the binary: a hosted
capability registry plus the Dry engine itself running server-side (wasm on Cloudflare
Workers), consumed by the free proprietary CLI, by slicer plugins, and by farm-management
software. **Price depends on usage.** The CLI authenticates gh-style (device flow); the
binary never phones home except for explicitly cloud-backed commands.

Working name: **Dry Cloud** (final name/domain is an open item).

## What is sold

| Surface | Free | Paid (usage-based) |
|---|---|---|
| Public registry reads (packs, search) | ✓ generous quota | metered beyond quota |
| Cloud verify jobs (engine-as-API) | small monthly quota | metered per job/MB |
| API keys for integrations | 1 key | multiple keys, higher rate limits |
| Private/org registries, publishing, proofs storage | — | Phase 2 (subscription + usage) |

Exact prices are NOT fixed by this spec — metering ships first; billing (Stripe/LS on
top of usage counters) is Phase 2. MVP launches with free quotas + visible usage.

## The registry data model — Capability Pack v1

A **pack** is a versioned, Ed25519-signed JSON document describing one printer
(or printer+toolhead variant). Eight sections, mirroring the owner's list:

1. **identity** — make/model/variant, firmware family+version range, pack semver.
2. **toolhead** — hotend/extruder/probe hardware, firmware features (PA, input shaper).
3. **filaments** — per-material temps/flow/cooling envelopes.
4. **macros** — named G-code macros with declared semantics (safe-to-strip, timing).
5. **presets** — process presets + mappings to slicer profile fields (Orca/Prusa/Cura).
6. **compatibility** — claims (works-with slicer X ≥ v, firmware Y range) with evidence links.
7. **observations** — calibration observations (measured PA, resonance, flow tests) with
   method + date + reporter.
8. **provenance** — trust-ladder state (`draft → imported → dry-verified →
   hardware-observed → maintained`), source attribution, signature block, and the
   **resolved dry-profile-v1** this pack compiles to.

Section 8 is the bridge: `GET /packs/{id}/profile` returns a ready
`dry-profile-v1` JSON — the existing engine contract — so the CLI and the verify
endpoint consume packs with ZERO engine changes. Packs extend, never fork, the
profile contract (`docs/11-profiles-and-reports.md`).

Schema lives at `spec/dry-pack-v1.schema.json` (new), validated the same way as
report schemas (JSON Schema + the Python validator + drift-gated goldens for the
seed packs).

## Architecture (all Cloudflare, one account)

```
CLI / slicer plugin / farm software
        │ (Bearer: device-flow token or API key)
        ▼
  api worker (workers-rs, Rust) ──── D1: accounts, keys(hashed), packs meta,
        │                                jobs, usage_events
        │                            R2: pack blobs, uploaded g-code, reports
        ├── GET /v1/packs?q=…        KV: device-flow codes (TTL), rate limits
        ├── GET /v1/packs/{id}[/profile]
        ├── POST /v1/jobs/verify  ──► Queue ──► consumer (same worker, queue handler):
        │                                        dry-core verify (native wasm32)
        │                                        against pack-resolved profile,
        │                                        report → R2, status → D1
        ├── GET /v1/jobs/{id}     (status + report JSON + shareable link)
        ├── POST /v1/auth/device  + /v1/auth/token   (RFC 8628)
        └── GET /v1/me /v1/usage
  auth pages worker route: /activate (user_code entry, Turnstile-protected)
```

- **workers-rs** (Rust Workers) so `dry-core` links directly — no JS↔wasm glue for the
  engine, one language across engine and API. The existing `crates/wasm` stays as-is
  for the browser; the api worker is a NEW crate (`crates/cloud`) compiled with
  `worker` crate. **Feasibility spike is Task 0 of the plan** (CPU limits: a Queues
  consumer gets minutes-scale CPU on paid plans; MVP caps uploads at 50 MB and
  publishes measured verify-time-per-MB from the spike).
- **Auth ("cloudflare auth", per 2026-07-28 research):** no Cloudflare consumer-IdP
  exists; Access is workforce-shaped (50-service-token cap). We run our OWN thin auth
  on Workers primitives: RFC 8628 device flow (`dry auth login` prints a code + URL,
  polls the token endpoint), opaque API keys (`dry_live_<random>`) stored as SHA-256
  hashes in D1, timing-safe comparison, Turnstile on the activation page. Account
  identity = email (verified via the device-flow activation page) — GitHub/social
  login can be added later via OpenAuth without schema changes.
- **Usage metering:** every authenticated request appends a `usage_events` row
  (account, route class, bytes, job cost); a scheduled Worker rolls up daily
  aggregates. `GET /v1/usage` exposes it; quotas enforced per route class with clear
  429s. Billing on top is Phase 2.
- **Pack signing:** packs are signed server-side at publish (registry key, Ed25519).
  The CLI verifies signatures offline via the repurposed crate (rename
  `crates/license` → `crates/signing`, keep `verify_token`-style API generalized to
  detached payload signatures). The trust ladder is thereby cryptographically
  anchored, not just a label.

## CLI additions (free binary, cloud-backed commands)

- `dry auth login|logout|status` — device flow; token in the platform config dir
  (`$XDG_CONFIG_HOME/dry/`), `DRY_TOKEN` env override for CI.
- `dry printer search <query>` / `dry printer show <id>` — registry reads.
- `dry printer add <id>` — pulls the pack, verifies its signature offline, materializes
  the resolved profile locally; thereafter plain `--profile` (or a new `--printer <id>`
  sugar) uses it. Works offline after the pull.
- `dry cloud verify <file> --printer <id>` — submits a cloud verify job, polls, prints
  the report (and the shareable link). Local `dry verify` remains fully offline.
- No enforcement anywhere: unauthenticated CLI keeps every existing offline feature.

## MVP scope (launch line)

1. Pack schema v1 + validator + **seed content**: the 6 golden profile-matrix printers
   (`conformance/profile-matrix/`) + Klipper imports converted to packs (sections 1, 2,
   3, 5, 8 populated; 4, 6, 7 present but sparse — honesty over fake density).
2. Auth worker (device flow + API keys + Turnstile) with tests.
3. Registry read APIs + pack signing + CLI `auth`/`printer` commands.
4. ONE engine endpoint: async verify job (upload → queue → wasm verify → report + link).
5. Usage metering + quotas + `/v1/usage` + CLI display.
6. Docs-site section: API reference, quickstarts (CLI login, slicer-plugin curl
   example), registry browse page (static, from the public API).
7. NOT in MVP: billing/checkout, private registries/orgs, community publishing (owner
   seeds all packs at launch; publish API exists but is owner-token-gated), compare/
   rewrite endpoints, web dashboard beyond the activation page.

## Error handling

- Verify jobs: per-stage failure states (`upload-invalid`, `too-large`, `engine-error`,
  `timeout`) persisted on the job; reports carry the same deterministic engine output
  as local runs (byte-comparable — that's the sales pitch).
- Device flow: RFC-mandated `authorization_pending`/`slow_down`/`expired_token`
  responses; codes are single-use, 10-min TTL.
- Quota exhaustion: 429 + `Retry-After` + a JSON body pointing at `/v1/usage`.
- Registry unavailability degrades the CLI gracefully: cloud commands fail with clear
  errors; everything offline is untouched.

## Testing

- Pack schema: golden seed packs, drift-gated like report goldens; Python validator
  extended.
- `crates/cloud`: workers-rs unit tests + miniflare-based integration (device flow
  end-to-end with a scripted "browser", API-key auth incl. timing-safe path, verify
  job lifecycle with a small fixture G-code, quota 429s). No live network in tests.
- CLI: integration tests against a local `wrangler dev` instance of the api worker
  (spawned by the test harness), covering login/pull/verify-job round trip.
- Spike (Task 0) publishes measured wasm verify throughput → sets the MVP size cap.
- Pre-launch E2E: real deploy, real device-flow login from the CLI, one real verify
  job, usage counter increments visible.

## Risks & accepted trade-offs

- **Engine-on-Workers CPU limits** — the spike de-risks first; fallback is a
  Container/Durable-Object runner behind the same job API (interface unchanged).
- **Empty-network cold start** — seeded with only ~6-10 packs at launch; mitigated by
  import tooling (`import-printer-cfg` → pack) making self-serve pack creation trivial,
  and by the free tier.
- **Rolling our own auth** — smallest viable OAuth surface (device flow only, no
  password storage, no social federation at MVP); Turnstile + rate limits on the two
  auth endpoints; independent review of the auth worker before launch.
- **Usage pricing undefined at launch** — deliberate: publish quotas, measure real
  usage, attach billing in Phase 2. Revenue starts later than the license model would
  have; the network/dataset compounds instead.
- Old licensing branch: `feat/commercial-license` keeps Task 1's crate (repurposed);
  the remaining licensing plan tasks are cancelled.

## Out of scope (MVP)

Billing/checkout, orgs/private registries, community publishing moderation, compare/
rewrite/explain endpoints, web dashboard, slicer plugin implementations (we ship curl
examples + API docs; plugins are partner/Phase-2 work), CNC verticals, SLAs.
