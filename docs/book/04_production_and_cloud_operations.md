# Chapter 4: Production Architecture, Cloud & Deployment

## 1. The Verify Runner Microservice (`dry-verify-runner`)

The `dry-verify-runner` is a standalone, asynchronous Rust service (built on `axum 0.7`, `tokio`, `tower-http`, and `reqwest` with `rustls-tls`) designed for high-throughput remote verification of untrusted machine code.

### Core Endpoints

* `GET /healthz` — Liveness probe (`{"ok": true}`).
* `GET /readyz` — Readiness probe reporting registry host bindings and body caps.
* `GET /metrics` — Prometheus telemetry (`dry_verify_requests_total`, `dry_verify_active_requests`, `dry_verify_segments_inspected_total`).
* `POST /verify?pack=<id>&version=<semver>&profile=<id>&registry=<url>` — Streams raw uploaded G-code to an ephemeral tempfile, fetches the resolved profile from the registry, executes verification on a dedicated blocking worker pool, and returns a structured JSON report.

---

## 2. Security & Zero-Trust Architecture

1. **SSRF Guarding**: The service refuses any connection to non-operator registry hosts.
2. **Ephemeral Data Erasure (RAII Guard)**: Uploaded G-code is held in temporary files guarded by `EphemeralGcodeFile`, which guarantees immediate unlinking on request completion, rejection, error, or panic.
3. **Zero Geometry Log Leakage**: Structured JSON logging only emits operational telemetry (`pack`, `version`, `profile`, `segments_inspected`, `duration_ms`), never customer geometry.
4. **Non-Root Execution**: Runs under system user `runner` (uid 10001) in a hardened, minimal `debian:bookworm-slim` container.

---

## 3. Cryptographic Authentication & Rate Limiting

* **Ed25519 Bearer Token Verification**: Requests carrying `Authorization: Bearer <token>` are cryptographically validated against trusted production public keys using `dry_license`.
* **Dynamic License Stamping**: Valid requests receive `license: { mode: "licensed", licensee, tier }` stamps; unauthenticated requests fall back safely to `evaluation`.
* **Sliding-Window Rate Limiting**: Enforces strict quotas (120 req/min for evaluation, 1200 req/min for licensed tiers), returning `429 Too Many Requests` on breach.

---

## 4. Operational Runbook & Deployment

### Local / Staging Launch via Docker Compose
```bash
docker compose -f deploy/docker-compose.yml up -d
```

### Load Testing with k6
```bash
k6 run tests/load/k6-verify.js
```
