# Deployment roadmap — from a gated engine to an operable hosted service

**Status:** reconciled 2026-09-05 against [ADR 0003](adr/0003-hosted-verification-service.md).

## Chosen topology

```text
client
  │ HTTPS: auth, quotas, async jobs
  ▼
services/cloud (public Worker control plane)
  ├─ D1: accounts, tokens, jobs, usage
  ├─ R2: uploads and reports
  └─ Queue ──► containers/verify-runner (private native verifier)
                  └─ versioned public printer registry
```

`deploy/cloudflare` is retired. `crates/cloud` is an archived measurement spike, not a service tier.
The supported embedded alternative is `dry-wasm`/`@dry/sdk`, which runs locally in the caller.

## Evidence and remaining gaps

| Track | Implemented and gated | Still required before production claim |
|---|---|---|
| D1 topology | One async ingress and one native verifier; duplicate proxy and Worker verifier retired | none |
| D2 observability | runner JSON tracing, request ids, refusal stages and Prometheus metrics | dashboard, alerts, redaction/logging policy |
| D3 identity/quota | device flow, opaque API keys, ownership checks, monthly quota, revocation | email verification, key rotation procedure |
| D4 deployment | staging-on-main / production-on-tag workflow, target-specific tests, production+staging Wrangler dry-runs, exact control-plane health smoke | provision resources and credentials; authenticated job smoke; execute and record rollback drill |
| D5 capacity | concurrent runner benchmark; 100 MB admission contract; `standard-3` container sizing from measured 43–50× import amplification | assert SLO thresholds on production-like infrastructure |
| D6 supply chain | pinned actions/toolchains, lockfiles, multi-arch GHCR image, SBOM/checksums/provenance in releases | verify deployed image digest during promotion |
| D7 operations/data | data-flow and failure-mode inventory | approve retention, deletion, jurisdiction, incident and logging policy |

## Current truth

The topology is implemented, locally tested and deployment-config validated. That is not evidence
that a public service is live. Until Cloudflare resources are provisioned and the authenticated
staging job plus rollback drill succeed, docs and release notes must use **implemented** or
**configured**, never **deployed** or **available at a production hostname**.

## Release sequencing

1. Keep the deployment workflow and every target-specific gate green on `main`.
2. Provision distinct staging and production D1, R2, Queue/DLQ and Container resources.
3. Store Cloudflare credentials through repository environments and set `DRY_CLOUD_BASE_URL`.
4. Deploy staging from `main`; run an authenticated upload → queue → runner → report smoke.
5. Execute a rollback drill and record recovery time and the restored revision.
6. Approve the D7 policy and capacity SLO.
7. Only then publish the production origin and promote on a version tag.

## Exit gate

One documented production origin accepts an authenticated 100 MB-bounded job, returns the native
runner's report through the asynchronous API, exposes operational telemetry without leaking customer
toolpaths, satisfies the measured SLO, and has a demonstrated rollback. All artifacts resolve to a
named release and deployed image digest.
