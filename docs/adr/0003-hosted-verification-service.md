# ADR 0003 — there will be a hosted verification service, and it is the container

- **Status:** Accepted
- **Date:** 2026-08-31
- **Workstream:** Deployment track D1
- **Supersedes:** the open decision recorded in [`../23-deployment-roadmap.md`](../23-deployment-roadmap.md) §D1

## Context

The deployment track opened with a genuine fork, and it stayed open for a month because it was a
product question rather than an engineering one:

> Neither is a service; they are two different sketches of one. **Deciding which one is the product —
> and deleting the other — is the first deliverable**, because maintaining two divergent sketches of
> the same idea is how `web/tpms.js` happened.

Two sketches existed:

- `containers/verify-runner` — a native axum service that imports g-code and runs `verify`, returning
  the byte-identical `dry verify --json` report.
- `crates/cloud` — a ~105-line Cloudflare Worker (workers-rs) returning timing JSON, not a `Report`.

The roadmap was explicit that "no hosted service" was a live and legitimate answer, and that choosing
it would delete D2 through D5.

## Decision

**There will be a hosted verification service.** It is `containers/verify-runner`, deployed as a
container image. `crates/cloud` is not a candidate and stays archived.

**Cloudflare and "hosted" are not a fork here.** The direction given was "Cloudflare or hosted", and
those resolve to one answer, because the choice was already settled by measurement rather than by
preference. The July 2026 spike
([`../superpowers/specs/2026-07-28-cloud-spike-findings.md`](../superpowers/specs/2026-07-28-cloud-spike-findings.md))
asked whether the import+verify path fits in a Workers isolate and found that it does not: holding the
raw body *in addition to* dry-core's ~43–50× import blowup exceeds the 128 MB isolate ceiling for
anything past roughly 1 MB. Its own recommendation was a **container**, which is why
`verify-runner` streams the request body to a tempfile inside a 6 GiB container instead.

So the service is a container, and Cloudflare Containers is a supported way to run it — as is any
other container host. That is a hosting choice that can be made later without changing the artifact,
which is why "whenever" is a coherent instruction for it and was not for D1 itself.

## Why `verify-runner` and not a fresh service

It is not merely the surviving sketch; it is the one that already satisfies most of D2, D3 and D5:

- **Identity and authorisation** — bearer tokens verified through
  `dry_license::verify_token_with_revocation`, with a per-licensee sliding-window rate limiter. Four
  handler tests cover a valid token, an invalid one, a **revoked** one, and rate-limit enforcement.
- **Observability** — `tracing`/`tracing-subscriber` (JSON + env-filter), a request id stamped through
  `request_id_middleware`, and `GET /metrics` in Prometheus text format whose counters are split by
  refusal *stage* (`profile-unavailable`, `input-invalid`, `engine-error`, `unauthorized`,
  `rate_limited`) — the "why was this refused" shape D2 asked for.
- **Capacity** — `tests/load_benchmark.rs` drives concurrent clients against the real handler and
  measures p50/p95/p99, in CI.
- **Correctness** — `verify_report_is_byte_identical_to_the_real_cli` builds and shells out to the
  real compiled `dry` binary and byte-compares its stdout against the service's HTTP response. That is
  external ground truth, not the service agreeing with itself.
- **Supply chain** — a multi-arch image is already built and pushed to
  `ghcr.io/dmytro-yemelianov/dry-verify-runner` by CI on `main`, and `deploy/docker-compose.yml`
  describes running it.

## Consequences

**D2, D3 and D5 are no longer blocked, and are no longer empty.** What remains of each is stated in
`23-deployment-roadmap.md`: a dashboard and a logging policy for D2, key rotation and revocation that
takes effect without a restart for D3, and a load test that *asserts* rather than reports for D5.

**D4 becomes the critical path.** Nothing deploys the image today. D4 now needs, concretely:

1. a hosting target chosen and an account/project created (Cloudflare Containers, or another host —
   the artifact does not change);
2. `ALLOWED_REGISTRY_HOST` set per environment — and **not** a signing-key secret, which this ADR
   originally listed in error: the licence verifying keys are compiled into the binary
   (`PRODUCTION_KEYS`), and the test key is honoured only under `cfg!(debug_assertions)`. The only
   deployment secrets are the Cloudflare API token and account id. The registry allowlist fails
   closed when unset, so the unset default is already the safe one;
3. a staging deploy on merge to `main` and promotion on tag, alongside the existing release workflow;
4. a rollback **executed in a drill**, not written down — the accept clause is explicit about that;
5. a documented SLO, and the runbook's remaining half.

**D7's open half is now answerable.** Its blocker was jurisdiction and identity: a deletion request
cannot be honoured without knowing who submitted what, and a logging policy cannot be written before
the first logger. Both follow from a chosen host, and the logging policy must be written **before** the
dashboard, because findings quote customer coordinates and feedrates
([`../24-operations-and-data-handling.md`](../24-operations-and-data-handling.md)).

**`crates/cloud` stays exactly as it is** — building in CI, README-marked "do not build on it". Its
value is the measurement that produced this decision, and deleting it would delete the evidence. This
ADR is the "explicitly marked a spike in its own README" half of D1's accept clause; the "one named
service, one deployment target" half is satisfied by naming `verify-runner`.

## What this does not decide

- **The hosting provider.** Cloudflare Containers is the presumed target and the artifact is
  provider-agnostic. Picking one is a D4 step, not this decision.
- **Pricing, quota tiers or a public endpoint.** D3's token *mechanics* work; what a token is allowed
  to do commercially is a separate product decision.
- **That the CLI stops being the product.** It ships, has an installed base, and as of v0.9.0 carries
  an SBOM and signed provenance. The service is additive.
