# ADR 0003 — hosted verification uses one async Cloudflare ingress and one native verifier

- **Status:** Accepted (amended 2026-09-05)
- **Date:** 2026-08-31
- **Workstream:** Deployment track D1–D4
- **Supersedes:** the unresolved topology in [`../23-deployment-roadmap.md`](../23-deployment-roadmap.md)

## Context

DryMachina had three overlapping hosted-verification sketches:

1. `crates/cloud`, a Workers-Rust feasibility spike that measured whether the full engine fits in a
   Worker isolate;
2. `deploy/cloudflare`, a synchronous public proxy directly forwarding `/verify` to a container;
3. `services/cloud`, an asynchronous control plane with device authorization, API keys, quotas,
   D1/R2 persistence, Queues and job polling.

The spike measured import memory at roughly 43–50 times the input size. A Worker isolate therefore
cannot safely host the full import-and-verify path for the supported 100 MB upload contract. Keeping
two public ingress implementations would also duplicate authentication, persistence, quota and
error semantics.

## Decision

**`services/cloud` is the sole public hosted-verification ingress and asynchronous control plane.**
It owns authentication, admission, quotas, uploaded-object persistence, job state and report
delivery.

**`containers/verify-runner` is the sole native verifier.** It runs privately behind the control
plane, receives the control plane's R2 object stream, fetches the versioned profile, streams the
input through bounded temporary storage, runs `dry-core`, and returns the report. The control plane
owns the R2 report write and D1 completion state.

Cloudflare is the intended host for this topology:

- Worker: `services/cloud`;
- D1: account, token, job and usage metadata;
- R2: uploaded programs and completed reports;
- Queues: asynchronous job dispatch;
- Cloudflare Containers: `containers/verify-runner`, sized as `standard-3` because a 100 MB input can
  require about 5 GiB during import.

`deploy/cloudflare` is retired as executable code; its directory remains only as a migration
tombstone. `crates/cloud` remains build-gated, measurement-only archival evidence and exposes only
`POST /spike/verify`. Neither is a product ingress.

## Invariants

- There is one public job API: `POST /v1/jobs/verify` followed by `GET /v1/jobs/{id}`.
- No public synchronous `/verify` route exists.
- Pages Functions do not expose verification directly or through a hosted MCP tool.
- Engine semantics live in `dry-core` and the native runner, never in the Worker.
- The Worker rejects oversized requests before persistence; the runner receives the same
  `MAX_BODY_BYTES` contract.
- Staging and production configure every D1/R2/Queue/Container binding independently; named Wrangler
  environments do not inherit them.
- A configured deployment must pass its tests, Wrangler dry-runs and exact control-plane `/healthz`
  smoke. End-to-end authenticated job smoke and a rollback drill remain separate release-readiness
  evidence.

## Consequences

- The public API is asynchronous even for small files. Embedded browser/Node verification remains
  available through `dry-wasm`/`@dry/sdk`, but it is not a hosted service tier.
- Cloudflare credentials and provisioned resource identifiers are deployment prerequisites, not
  repository code. Their absence produces an explicit skipped deployment rather than a false claim
  that a service is live.
- The repository contains a deployable, dry-run-validated topology; it does **not** claim a live
  production origin until a completed deployment, authenticated job smoke and rollback drill are
  recorded.
- Historical Pages deployments that contain `/api/verify` or hosted `/api/mcp` remain a launch
  blocker until the owner deletes them or protects `*.drymachina.pages.dev` with Cloudflare Access.
- Retention, deletion, jurisdiction and customer-data logging policy remain launch blockers; see
  [`../24-operations-and-data-handling.md`](../24-operations-and-data-handling.md).

## Rejected alternatives

- **Full verification in a Worker isolate:** rejected by measured memory amplification.
- **Public synchronous proxy plus async control plane:** rejected because it creates two auth,
  quota, persistence and error contracts.
- **Container as the public API:** rejected because device flow, keys, quotas and durable job state
  already belong to the control plane.
- **Delete the Worker spike:** rejected; it is useful falsification evidence, provided it remains
  visibly archived and cannot masquerade as a product endpoint.
