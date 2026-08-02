# Operations and data handling

**Status:** D7 of [`23-deployment-roadmap.md`](23-deployment-roadmap.md) · 2026-08-03
**Scope:** `containers/verify-runner`, the only service in the tree that accepts customer data.

This document exists because uploaded g-code is **customer intellectual property** and had no stated
handling at all. It describes what the code does today, not what a mature service would do — the gaps
are named rather than smoothed over, because a policy that describes an aspiration is worse than none.

> **`verify-runner` is not deployed.** D1 has not been decided; there may never be a hosted service.
> If one is stood up, this is the policy it inherits and the runbook it starts from.

---

## Part 1 — Data handling

### What is submitted

A `POST` to `/verify` carries a raw g-code program in the body plus a profile reference. The program
is the customer's design output: toolpaths, feedrates, and by inference geometry, process parameters
and know-how. Treat it as confidential by default.

### What happens to it

1. The body is **streamed to a tempfile** under `/tmp`, prefix `dry-verify-`
   (`containers/verify-runner/src/lib.rs`). It is never buffered whole in memory — a deliberate
   choice, since dry-core's import already costs ~43–50× the input size.
2. Import and verify run on a blocking thread against that file.
3. The file is **deleted on completion** — an explicit `drop(named_file)` immediately after the
   blocking task returns.

### Retention

**None by design.** Nothing is persisted: no database, no object store, no request archive. The
response is computed and the input is deleted.

**Three honest exceptions**, none currently mitigated:

- **Crash or SIGKILL between create and drop leaves the tempfile in `/tmp`.** `drop` does not run if
  the process dies. On a container with a fresh filesystem per start this is bounded by the container
  lifetime; on a long-lived host it is not. *Mitigation if deployed: ephemeral per-request storage, or
  a startup sweep of `dry-verify-*`.*
- **The report echoes the input's shape.** `Report` now carries `segments_inspected`,
  `rules_evaluated`, `contracts` and findings that quote coordinates and feedrates. Anything that logs
  responses logs a projection of customer geometry.
- **`MAX_BODY_BYTES` bounds size, not sensitivity.** There is no content classification and none is
  planned; the service cannot tell a test cube from a production part.

### What is *not* true today

- **No authentication** — no identity is attached to a submission, so a request cannot be attributed,
  and a deletion request could not be honoured because nothing links data to a person.
- **No logging** — which incidentally means no accidental data capture, and also means an incident
  cannot be investigated. Adding logs (D2) is the point at which this policy needs revisiting: **the
  first structured logger must decide what it is allowed to record.** Findings quote coordinates.
- **No encryption at rest** for the tempfile beyond whatever the host filesystem provides.
- **No egress of submitted data.** The only outbound request is to the profile registry, gated by
  `ALLOWED_REGISTRY_HOST`, and it carries a profile identifier — never the program.

### Before this service takes real customer data

1. Decide the retention story explicitly, including the crash case.
2. Decide what logs may contain **before** writing the first one.
3. Attach identity, so deletion and export requests are answerable.
4. State the jurisdiction the data rests in, which depends on the D1 hosting decision.

---

## Part 2 — Runbook

### Health

`GET /healthz` → `{"ok": true}`. It answers from the process only: it does not check registry
reachability or exercise the engine, so **a green healthz does not mean a verify will succeed.**

### Configuration

| Variable | Effect | Failure if unset/wrong |
|---|---|---|
| `ALLOWED_REGISTRY_HOST` | allow-lists the profile registry host | every request → `502 profile-unavailable` |
| `MAX_BODY_BYTES` | request body cap | default applies; oversized bodies → `422 input-invalid` |

### The failure modes, and what each means

Errors carry a `stage` field. Match on it rather than on the message text.

| Status | `stage` | What it means | First action |
|---|---|---|---|
| `422` | `input-invalid` | The program was **refused by the engine**, or the body was malformed/oversized. Not a fault. | Read `error`. This is the correct answer to bad input, and a spike is a customer-side or upstream-generator problem. |
| `502` | `profile-unavailable` | The registry 404'd, was unreachable, or the host is not allow-listed. | Check `ALLOWED_REGISTRY_HOST`, then registry reachability. Most likely a config or dependency fault, not ours. |
| `500` | `engine-error` | dry-core panicked, or a tempfile operation failed. | **This is the only one that is our bug.** Capture the input if the customer will share it; a panic in the engine is a defect worth a test. |

The distinction that matters operationally: **`422` is the service working.** The engine refuses
malformed programs by design (ADR 0002 §4), and a refusal rate is a signal about inputs, not about
health. Alerting on 4xx here would page someone for a working guard — a mistake this repo has already
made once in reverse, by mapping a deliberate refusal to a `500`.

### Capacity

From the cloud spike, measured natively: **~1 s per 50 MB** of g-code, and import costs **~43–50×**
the input in memory. The container is provisioned at 6 GiB, so a 50 MB upload peaks around 2.5 GB.

**Concurrency is the unguarded dimension.** Two concurrent large uploads can plausibly exhaust the
container, and nothing limits in-flight requests. Verify work runs on `spawn_blocking`, so the async
reactor stays responsive while the pool saturates — meaning the symptom is *latency*, not refusal,
until memory runs out.

**There is no load test.** These numbers are single-request measurements; the concurrent behaviour
above is reasoned from the code, not observed. D5 exists to replace this paragraph with data.

### Diagnosing without observability

Today there are no logs, metrics or request ids, so incident response is limited to:

1. Reproduce with the same profile and program via the CLI: `dry import-gcode` then `dry verify`,
   which is the same composition the service performs.
2. Compare against `dry verify --json` — the service's output is byte-identical to the CLI's for the
   same inputs, and a handler test pins that.

**If you cannot get the customer's program, you cannot investigate.** That is the honest state, and it
is D2's justification.

### Rollback

There is no deployment pipeline (D4), so there is no rollback procedure. Do not treat this section as
present-and-untested; it is absent.
