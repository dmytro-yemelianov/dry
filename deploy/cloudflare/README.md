# `deploy/cloudflare` — the hosted verification service

Cloudflare Containers deployment of [`containers/verify-runner`](../../containers/verify-runner).

The service decision is [ADR 0003](../../docs/adr/0003-hosted-verification-service.md):
`verify-runner` is the product, deployed as a container image. Cloudflare Containers runs that same
image, which is why the hosting choice could be made after the service choice — nothing here changes
the artifact.

## Shape

```
POST /verify?pack=…&version=…&profile=…&registry=…
        │
        ▼
   Worker (src/index.ts)          admission gate + router, no verification logic
        │  100 MB Content-Length cap, /verify and /healthz only
        ▼
   VerifyRunner container         containers/verify-runner, port 8080
        │  streams the body to an ephemeral tempfile, imports, verifies
        ▼
   byte-identical `dry verify --json` report
```

The Worker is deliberately thin. Everything about verification lives in the container, whose output
is pinned byte-for-byte against the real compiled `dry` binary by
`verify_report_is_byte_identical_to_the_real_cli`. A Worker that reimplemented any of it would be the
second divergent sketch ADR 0003 exists to prevent.

## The one coupling you must not break

`MAX_BODY_BYTES` and `instance_type` are **not independent**.

`dry-core`'s g-code import allocates roughly **43–50× the input size** in peak process memory
([measured, July 2026](../../docs/superpowers/specs/2026-07-28-cloud-spike-findings.md)). That number
is the reason this is a container at all: it is what ruled out a 128 MB Workers isolate.

| Body cap | Worst-case peak | Smallest instance that fits |
|---|---|---|
| 100 MB (configured) | ~5 GB | `standard-3` — 8 GiB, ~3 GiB headroom |
| 200 MB (the runner's own default) | ~10 GB | nothing comfortably; `standard-4`'s 12 GiB is the per-container **ceiling** |

So the cap is set explicitly here rather than left at the runner's default, because that default was
chosen as container headroom, not as a product limit. **Raising one requires resizing the other**, and
there is no larger instance to escape to past `standard-4`.

## First-time account setup

These are the steps that need an account and therefore cannot be scripted here.

1. A Cloudflare account on a **Workers Paid** plan (Containers requires it).
2. An API token with Workers Scripts **Edit** and Workers Durable Objects **Edit** for that account.
3. Repository secrets: `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`.
4. GitHub Environments named `staging` and `production` (the workflow references both; production is
   the natural place to require a reviewer).
5. A repository/environment **variable** `VERIFY_BASE_URL` per environment, so the post-deploy health
   check has something to call. Without it the check warns and skips rather than failing.
6. Confirm `ALLOWED_REGISTRY_HOST` in [`wrangler.jsonc`](wrangler.jsonc) names the real registry for
   each environment. **The unset default is the safe one**: `fetch_profile` refuses every registry
   when the variable is absent, so a misconfiguration fails closed with `502 profile-unavailable`
   rather than fetching from somewhere unintended.

There is **no signing-key secret to set.** The licence verifying keys are compiled into the binary
(`PRODUCTION_KEYS`), and the test key is accepted only under `cfg!(debug_assertions)` — a release
binary will not honour one.

## Before the account exists

The deploy workflow is safe to merge without any of the above. Its `deploy` job checks for
`CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` and, when they are absent, **skips with a notice
instead of failing** — so `main` does not go red waiting for an account. The `verify` and `config`
jobs still run on every change, so the container's tests and both wrangler configs stay checked
whether or not anything is deployed.

An unconfigured deploy is a skip. A configured deploy that fails is a failure. Keeping those distinct
is the point: a workflow that is always red says nothing.

## Deploying

CI does this: staging on merge to `main`, production on a `v*` tag
([`deploy-verify.yml`](../../.github/workflows/deploy-verify.yml)). By hand:

```sh
cd deploy/cloudflare
npm ci
npm run typecheck
npx wrangler deploy --dry-run          # resolves bindings and builds the image, touches no account
npm run deploy:staging
npm run deploy:production
```

`--dry-run` is worth running before anything else: it catches a malformed config before a rollout
rather than half-way through one.

## Rolling back

A container rollout is **gradual**, not instant like a Worker — old instances keep serving during it.

```sh
cd deploy/cloudflare
npx wrangler deployments list          # find the last known-good version id
npx wrangler rollback [<version-id>]   # omit the id to roll back one deployment
curl -fsS "$VERIFY_BASE_URL/healthz"   # a rollback is not done until this answers
```

> **D4's accept clause requires a rollback that has been *executed* in a drill, not merely written
> down.** This procedure has **not** been drilled — there is no account yet. Until it has been, treat
> this section as untested, which is exactly the distinction this repo keeps making between a
> documented capability and a demonstrated one.

## What is deliberately not exposed

`/metrics` is **not** proxied by the Worker. The runner serves Prometheus counters split by refusal
stage, plus segment totals — operational detail about other people's jobs, which should not be
world-readable. Scraping it needs a private or authenticated route, and choosing one is part of D2
(the dashboard), not something to leak by default.

Note also that a logging policy has to be written **before** the first logger is pointed anywhere:
findings quote customer coordinates and feedrates
([`24-operations-and-data-handling.md`](../../docs/24-operations-and-data-handling.md)).

## Platform facts worth knowing

Verified against the live docs rather than recalled, because this product moves:

- Containers is **generally available** on Workers Paid (the cached skill reference still said beta).
- Instance ceiling per container: **4 vCPU / 12 GiB / 20 GB disk**.
- Cold start 2–3 s; `startAndWaitForPorts()` has a 20 s budget; graceful shutdown is SIGTERM then
  SIGKILL after 15 min — comfortable for a request that streams to a tempfile and deletes on drop.
- **Disk is ephemeral** and resets when an instance stops. That suits this service exactly: uploaded
  programs are customer IP and nothing is meant to persist.
- **No autoscaling.** `max_instances` plus `getRandom(binding, INSTANCES)` *are* the load balancing,
  and the two numbers must agree — `INSTANCES` in `src/index.ts` addresses instances that must exist.
- Images must be `linux/amd64`; CI already builds multi-arch including amd64.
