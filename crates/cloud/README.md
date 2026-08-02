# `dry-cloud` — an archived feasibility spike

**This is not a product surface and is not deployed.** It is retained as evidence, not as a service.

## What it is

A ~105-line Cloudflare Worker (workers-rs) written to answer one question in July 2026: can
`dry-core`'s g-code import + verify path run inside a Workers isolate? It posts a g-code body to
`/spike/verify` and returns **timing JSON** — `{bytes, parse_ms, verify_ms, total_ms, segments,
findings}` — not a `Report`.

## What it answered

It did not fit. Holding the raw body in memory *in addition to* dry-core's ~43–50× import blowup
exhausted a 128 MB isolate at 10–50 MB inputs. That finding is written up in
`docs/superpowers/specs/2026-07-28-cloud-spike-findings.md`, and it is why
`containers/verify-runner` streams the body to a tempfile and runs in a 6 GiB container instead.

The spike therefore succeeded: it produced a measurement that redirected the design.

## Why it is still here

Deleting it would delete the evidence for a sizing decision that shaped the service that replaced it.
It is kept building in CI (`cloud (workers-rs wasm32 build)`) so it cannot rot into something that no
longer compiles and can no longer be re-run if the constraint changes.

## What it is not

- **Not the hosted service.** `containers/verify-runner` is the shape a service would take
  (see [`docs/23-deployment-roadmap.md`](../../docs/23-deployment-roadmap.md) D1).
- **Not maintained to product standards.** It has no tests, no authentication and no error contract.
  Do not build on it.
- **Not a claim that Workers is unsuitable in general** — only that this composition, at these input
  sizes, in a 128 MB isolate, was not.

If a Workers deployment is ever revisited, start from the findings document and the current engine,
not from this code.
