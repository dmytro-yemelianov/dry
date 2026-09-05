# `services/cloud` — Dry Cloud control plane

This TypeScript Cloudflare Worker is the **sole public ingress** for hosted DryMachina verification.
It authenticates clients, applies quotas, stores uploads and job state, and dispatches asynchronous
work to the native [`containers/verify-runner`](../../containers/verify-runner) image. It does not
implement verification itself.

```text
client
  -> POST /v1/jobs/verify (Bearer, <=100 MB)
  -> R2 upload + D1 job + Cloudflare Queue
  -> private VerifyContainer instance
  -> containers/verify-runner POST /verify
  -> R2 report + D1 terminal status
  -> GET /v1/jobs/{id}
```

The older `deploy/cloudflare` synchronous proxy is retired. The Rust Worker in `crates/cloud` is an
archived measurement spike and is not a product ingress. See [ADR 0003](../../docs/adr/0003-hosted-verification-service.md).

## Current status

- Source, bindings and production/staging configurations are implemented.
- Worker-runtime unit and integration-style tests run under `@cloudflare/vitest-pool-workers`.
- `wrangler deploy --dry-run` validates both production and staging and builds the runner image.
- `itest/jobs-local.sh` exercises real 1/10/50 MB Worker → R2/Queue → container transfer when Docker
  is available. Local Cloudflare Container networking cannot reach its host registry stub, so this
  is transfer evidence through the runner's post-body-write stage, not a completed report-parity test.
- **No live production deployment is established by repository evidence.** Image publication and a
  green dry-run are not proof that an endpoint is serving traffic.

## Safety and capacity invariants

- `POST /v1/jobs/verify` requires an exact `Content-Length` and rejects bodies above 100 MB before
  writing to R2 or D1.
- The runner repeats the 100 MB cap through `MAX_BODY_BYTES` and only fetches profiles from the host
  derived from `REGISTRY_URL` through `ALLOWED_REGISTRY_HOST`.
- Production and staging use Cloudflare Containers `standard-3` (8 GiB). The measured 43-50x import
  amplification makes a 100 MB input approximately 5 GiB at peak, leaving roughly 3 GiB headroom.
- Each job is mapped to a named container instance; Queue delivery is at-least-once, so terminal jobs
  are idempotently skipped on redelivery.
- Turnstile bypass is accepted only in the local `dev` environment. Staging and production fail closed.

## Local verification

```sh
cd services/cloud
npm ci
npm run check
npx wrangler deploy --dry-run --outdir /tmp/dry-cloud-production
npx wrangler deploy --env staging --dry-run --outdir /tmp/dry-cloud-staging
```

For the local large-body transfer probe, Docker must be running:

```sh
services/cloud/itest/jobs-local.sh
```

Read the script's result labels literally: `TRANSFER PATH` does not mean a full hosted round trip.
The first complete end-to-end proof must run in provisioned staging against an accessible registry.

## First deployment

Before enabling the deploy job, provision distinct production and staging resources named in
`wrangler.jsonc`: D1 databases, KV namespaces, R2 buckets, job queues, dead-letter queues and the
Container/Durable Object binding. Replace placeholder binding IDs, configure Turnstile as encrypted
Worker secrets, and configure the GitHub `staging` and `production` environments with Cloudflare
credentials plus `DRY_CLOUD_BASE_URL`.

The workflow deploys staging on `main` and production on a version tag. Its `/healthz` smoke proves
only control-plane liveness. Before calling the service deployed, run an authenticated staging job
through submission, queue processing and report retrieval, then record an executed rollback drill.

## Rollback

Use Wrangler's deployment history for the selected environment and roll back to the last known-good
version. A rollback is incomplete until `/healthz` answers with the exact control-plane contract and
an authenticated staging verify job reaches a terminal state. This procedure remains **undemonstrated**
until an account-backed drill is recorded.
