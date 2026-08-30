# Deployment roadmap — from a gated engine to a production-grade service

**Status:** proposed 2026-08-02, reconciled against the code 2026-08-30 · **Owner:** unassigned ·
**Precedes:** Phase 6 (retire the oracle)

> **Reconciliation note (2026-08-30).** D2, D3 and D5 described a service that no longer exists: the
> runner had grown observability, bearer-token auth with revocation, rate limiting and a load
> benchmark without this document following. Those sections now state what is built and what is
> genuinely left. This repo's recurring defect is a claim outrunning its behaviour; a roadmap that
> *understates* what shipped is the same defect pointed the other way, and it hides which gaps are
> real. **D1 and D4 are unchanged and remain the blockers:** nothing deploys any of this, and whether
> there should be a hosted service at all is still an open decision.

## Why this document exists

Dry's *engine* is production-grade by most measures a compiler is judged on: 444 core tests, clippy
under `-D warnings` with no `#[allow]` escapes, four independent numeric-boundary inventories whose
epsilons are pinned against the Rust constants, a claim registry validated in CI, an independent
Python re-implementation of the codec, and two external oracles (FullControl for FFF output, LinuxCNC
`rs274` for CNC).

Dry's *product* is not deployed. The gap is not engine quality — it is everything between a green CI
run and a service someone can depend on. This roadmap names that gap explicitly rather than letting
"we have good tests" stand in for "we can operate this."

The distinction matters because this repo's recurring defect has been a claim that outran the
behaviour it described. "Production-grade" is the largest such claim available, so it gets the same
treatment as the others: stated in pieces, each with an acceptance test that can fail.

## What exists today

| Surface | State | Deployable? |
|---|---|---|
| CLI (`dry`) | Released binaries for 4 targets, checksummed | **Yes** — this is the shipping product |
| Python wheel / npm tarball | Built and attached to GitHub Releases | Yes, as downloads — not registry-published, deliberately |
| Public docs + gallery | Cloudflare Pages, built from source in CI | Yes |
| `containers/verify-runner` | axum service, 22 handler tests + a load benchmark; bearer-token auth with revocation, a sliding-window rate limiter, request ids, structured `tracing` and a Prometheus `/metrics` endpoint | Not yet — nothing deploys it (D4) |
| `crates/cloud` | Explicitly a *feasibility spike* — returns timing JSON, not a `Report` | No |
| `crates/license` | `verify_token` + `verify_token_with_revocation`, consumed by the runner; issuance lives in `tools/license-issuer`. No **rotation** path | Library, not a service |

## The honest gap list

These are ordered by what blocks a paying user, not by effort.

### D1 — There is no service to deploy (L)

`verify-runner` fetches a profile from a registry, imports g-code, runs `verify`, and returns the
report. It began as an MVP-shaped handler with no authentication, rate limiting, quota or request
identity; it has since grown all four (see D2 and D3) and still has no persistence. `crates/cloud` is
a spike whose own header says it is not production.

Neither is a service; they are two different sketches of one. **Deciding which one is the product —
and deleting the other — is the first deliverable**, because maintaining two divergent sketches of the
same idea is how `web/tpms.js` happened.

*Accept:* one named service, one deployment target, the other removed or explicitly marked a spike in
its own README.

**Half done, and the half that was reversible.** `crates/cloud` now carries a README saying plainly
that it is an archived feasibility spike, not a product surface: it returns timing JSON rather than a
`Report`, it answered a sizing question in July 2026 (a 128 MB Workers isolate could not hold the raw
body *plus* dry-core's ~43–50x import blowup), and that answer is why `containers/verify-runner`
streams to a tempfile in a 6 GiB container instead. It is kept building in CI so the evidence cannot
rot, and marked "do not build on it".

**The decision itself is still open, deliberately.** Naming `verify-runner` as *the shape a service
would take* is not the same as deciding there will be one. "No hosted service" remains a live and
legitimate answer: the CLI ships today, has an installed base, and as of #220 carries an SBOM and
signed provenance. Choosing it deletes D2 through D5 entirely.

What is now unblocked either way: D2's first question — **what may a log contain** — can be answered
against one service shape rather than two. See
[`24-operations-and-data-handling.md`](24-operations-and-data-handling.md), which notes that findings
quote coordinates and feedrates, so the answer is not "everything".

### D2 — Observability (M) — **largely landed in the service; no dashboard**

*Accept:* a request id on every response and log line; structured logs; latency, error-rate and
refusal-reason metrics exported; a dashboard that answers "what is failing and for whom" without a
code change.

**Landed.** The paragraph this section opened with — that `grep -rl "tracing\|opentelemetry" crates/
containers/` returned nothing — has been false since the runner grew its observability layer.
`containers/verify-runner` now depends on `tracing`/`tracing-subscriber` (JSON + env-filter) and
`tower-http`'s `request-id`/`trace`, stamps a request id through `request_id_middleware`, and serves
`GET /metrics` in Prometheus text format. The counters are the refusal-reason shape this section
asked for, not just a request total: `dry_verify_errors_total` is split by `stage`
(`profile-unavailable`, `input-invalid`, `engine-error`, `unauthorized`, `rate_limited`), alongside
active requests, cumulative duration and segments inspected.

**Remaining:** the dashboard, and the logging policy that has to precede it — the one this roadmap
already flagged, since findings quote customer coordinates and feedrates
([`24-operations-and-data-handling.md`](24-operations-and-data-handling.md)). Metrics exported by a
service nobody deploys (D4) are also not yet observability in the operational sense.

### D3 — Authentication, authorisation and quota (L) — **accept clause largely met; no rotation**

*Accept:* a token can be issued, scoped, rate-limited, revoked, and its revocation takes effect
without a redeploy. A revoked token is refused; the test proves it.

**Most of the accept clause is met at the service.** `verify_token` is no longer in isolation: the
runner reads a `Bearer` token, verifies it through `dry_license::verify_token_with_revocation`, binds
the licensee to a per-client sliding-window rate limit, and stamps the resulting mode onto the
report. Four handler tests cover the branches this clause names — a valid token, an invalid one, a
**revoked** one, and rate-limit enforcement — so "a revoked token is refused; the test proves it" is
literally satisfied. `tools/license-issuer` supplies issuance and has its own CI job.

**Remaining:** rotation, and "takes effect without a redeploy" — the revocation list is read by the
process rather than from a live source, so revoking still means restarting something. And the
commercial boundary for the **CLI** remains legal rather than technical, unchanged: the artifacts are
public downloads.

### D4 — No deployment pipeline or rollback (M)

`release.yml` builds and attaches artifacts. Nothing deploys a service, and there is no staging
environment, no migration story, no rollback procedure, no documented SLO. `docs/12-releasing.md`
covers *publishing* and explicitly stops there.

*Accept:* a versioned deploy to staging on merge, promotion to production on tag, a rollback that has
been *executed* in a drill and not merely written down.

### D5 — Measuring capacity (M) — **a load test exists; it asserts nothing**

*Accept:* a published throughput/latency curve against program size, a documented concurrency limit,
and a load test in CI that fails on regression rather than reporting a number nobody reads.

`benches/` is still a **compile gate** — it proves the benchmarks build, not that anything is fast.
Peak memory is bounded and proven (`memory_scale`, 1M segments under a counting allocator), which is
real and unusual.

**Partly landed.** `containers/verify-runner/tests/load_benchmark.rs` drives concurrent clients
against the real handler and measures throughput and p50/p95/p99 latency, and it runs in the
`verify-runner` CI job. **Remaining, and it is the half that matters:** it reports numbers rather
than asserting them, so nothing fails on regression; there is no curve against *program size*; and
no concurrency limit is documented. A load test whose numbers nobody reads is precisely what this
clause was written against.

### D6 — Supply chain is partly covered (S)

`cargo audit` runs in CI; `npm audit --audit-level=high` runs for the docs site. There is no SBOM, no
artifact signing, no provenance attestation, and `SHA256SUMS` is published beside the artifacts rather
than signed.

*Accept:* signed releases with an SBOM and provenance attestation a consumer can verify without
trusting the release page.

### D7 — No operational runbook (S)

No incident procedure, no on-call expectations, no data-retention or privacy statement for submitted
g-code — which is customer IP and currently has no stated handling policy at all.

*Accept:* a runbook covering the top failure modes, and a written data-handling policy for uploaded
programs.

**Partly done** — [`24-operations-and-data-handling.md`](24-operations-and-data-handling.md) states
what happens to an uploaded program (streamed to a `/tmp` tempfile, deleted on completion, nothing
persisted), names the three cases where that is not the whole truth (crash before `drop`, findings
that quote customer geometry, no content classification), and gives a runbook for the three error
stages. What remains needs decisions that are not documentation: identity, so a deletion request is
answerable; a logging policy, decided **before** the first logger is written since findings quote
coordinates; and a jurisdiction, which follows from D1.

## Sequencing

```
D1 (pick the service) ──┬─> D2 observability ──> D4 pipeline ──> D5 load
                        └─> D3 authn/quota ─────────┘
D6, D7 run alongside; neither blocks the others.
```

D1 first because every later item multiplies by the number of services. D2 before D4 because deploying
something you cannot observe converts an outage into a mystery.

## What this roadmap deliberately does not claim

- **It does not say the engine needs more hardening.** H1.1–H1.8 closed the classes an audit found,
  and Phase 5 is one item from its exit gate. The remaining engine work is listed in
  [`04-tasks.md`](04-tasks.md); it is not on the critical path to deployment.
- **It does not treat CI green as production readiness.** Every gate in this repo runs against a build
  machine, and none of them has ever served a request.
- **It does not assume the service is the product.** The CLI ships today and may remain the whole
  business. D1 is a genuine decision, not a formality — and "no hosted service" is a legitimate
  answer that makes D2–D5 disappear.

## Exit gate

A named service, deployed to production by pipeline, authenticated and quota-limited, observable
enough to diagnose a refusal without a code change, load-tested with a published capacity curve, with
a rollback that has been rehearsed and a runbook for the top failure modes.
