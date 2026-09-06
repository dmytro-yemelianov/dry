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

## Image promotion

The committed `containers[].image` is a Dockerfile path, which is what `wrangler deploy --dry-run`
validates. A real deploy never uses it. `deploy-verify.yml` promotes an image that CI already built,
published and signed, through [`tools/promote_runner_image.py`](../../tools/promote_runner_image.py):

1. Resolve the release-named tag in GHCR — `X.Y.Z` for a `vX.Y.Z` production promotion, `sha-<short>`
   for a staging promotion off `main` — to its immutable `sha256:` digest.
2. Verify the signed build provenance for that digest with `gh attestation verify`, requiring the
   attested subject digest, source repository, source commit, builder workflow and builder ref to
   match the promotion exactly. Any mismatch fails closed, before anything is deployed.
3. Copy the verified image into the Cloudflare managed registry. Cloudflare Containers pull only from
   `registry.cloudflare.com`, Docker Hub, Amazon ECR and Google Artifact Registry — never from GHCR —
   so the image is pulled **by digest** and re-pushed under the derived tag `sha256-<digest hex>`.
4. Deploy with a generated config (`wrangler.promotion.jsonc`, gitignored) whose only difference from
   the committed config is that one container image, pinned to that tag.
5. Record `deployment-evidence.json`: release, source commit, source image digest, promoted image and
   the deployed environment revision. The record is rejected as incomplete if the revision is missing.

`tools/check_image_promotion_policy.py` runs in CI and fails if the builder stops publishing an
attested semver-named digest, or if the deploy job deploys without the promotion gate and its config.

CI also renders a promotion config from a placeholder digest and dry-runs both environments, so the
generated shape stays deployable without an account or a published image.

What remains unproved against a live account is the Cloudflare registry push and the deployed
revision id; neither has ever run. Treat the first configured staging deploy as the proof of those
two steps, not this description.

## Rollback

Use Wrangler's deployment history for the selected environment and roll back to the last known-good
version. A rollback is incomplete until `/healthz` answers with the exact control-plane contract and
an authenticated staging verify job reaches a terminal state. This procedure remains **undemonstrated**
until an account-backed drill is recorded.
