# Chapter 4: Production Architecture, Cloud & Deployment

## 1. One public control plane, one verifier

DryMachina hosted verification is asynchronous. `services/cloud` is the sole public Cloudflare
Worker ingress for device login, API keys, quota, uploads and job polling. It stores job metadata in
D1, inputs/reports in R2 and dispatches work through Queues.

`containers/verify-runner` is a private native service behind that control plane. It resolves a
versioned printer profile, streams G-code to a tempfile and returns the same deterministic report
contract as the CLI. The engine never runs in the public Worker.

## 2. Public API

- `GET /healthz` — control-plane liveness only.
- `POST /v1/auth/device`, `POST /v1/auth/token`, `GET|POST|DELETE /v1/keys` — identity and keys.
- `POST /v1/jobs/verify` — authenticated, quota-controlled raw G-code upload; returns HTTP 202.
- `GET /v1/jobs/{id}` — owner-only status and completed report.
- `GET /v1/usage` — canonical monthly usage and quota.

There is no public synchronous `/verify` endpoint. Embedded clients use `dry-wasm`/`@dry/sdk` locally.

## 3. Security and capacity

Tokens are opaque and only hashes are stored. Staging/production reject the development Turnstile
bypass. Inputs above 100 MB fail before persistence, and the private runner receives the same limit.
Because import can consume 43–50× the source size, the Cloudflare Container is configured as
`standard-3` (8 GiB); full verification in a Worker isolate is explicitly unsupported.

## 4. Deployment evidence

The deployment workflow runs the runner tests, the Worker typecheck/runtime tests, and production
plus staging Wrangler dry-runs before any account mutation. With protected-environment credentials it
deploys staging from `main` and production from a version tag, then checks the exact public health
contract.

This repository currently proves **implemented and deployable**, not **live production service**.
Launch still requires provisioned resources, an authenticated end-to-end staging job, an executed
rollback drill, approved data policy and capacity SLO.

Docker Compose remains a local runner-development tool; it is not the public production topology.

## 5. Release governance

All lockstep release surfaces are checked by `scripts/check-version.sh`. Release artifacts, SBOM,
checksums and attestations prove what was built; deployment records must additionally pin the Worker
revision and private image digest. A production promotion is incomplete until both identities are
recorded and the post-deploy smoke succeeds.
