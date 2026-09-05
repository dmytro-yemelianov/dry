# `deploy/cloudflare` — retired direct proxy

The synchronous public `POST /verify -> Container` proxy that lived here was retired in the v0.11
architecture reconciliation. It bypassed Dry Cloud authentication, quotas, D1/R2 persistence and
the asynchronous job lifecycle, creating a second incompatible public API for the same verifier.

The supported topology is now:

```text
services/cloud (public async control plane)
  -> D1 / R2 / Queue
  -> private VerifyContainer
  -> containers/verify-runner (sole native verifier)
```

Deployment configuration and the operator runbook live in
[`services/cloud`](../../services/cloud). This tombstone is intentionally non-executable so old links
explain the migration without leaving a second deployable ingress in the repository.
