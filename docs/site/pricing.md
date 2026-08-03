# Pricing

> **Licensing is not open yet.** Dry is currently in a closed pilot — licenses are
> hand-issued to pilot participants, and the checkout links below are not yet live.
> To join the pilot, email [license@yemelianov.dev](mailto:license@yemelianov.dev?subject=Dry%20Pilot).

Dry is licensed per organization, annually. There is no usage metering and no phone-home: a
license is an offline-verified Ed25519 token, so pricing scales with who's using the CLI, not
how many times it runs.

## Tiers

| | Solo | Team | Pilot |
|---|---|---|---|
| **Price** | $990 / year | $4,990 / year | $1,500–$5,000 (one-time, 90 days) |
| **Users** | 1 | up to 10 | — |
| **Machines** | up to 3 | up to 25 | Team limits |
| **Support** | Email, best-effort | Email, priority | Hands-on onboarding |
| | [Buy Solo →](https://dry.lemonsqueezy.com/checkout/buy/VARIANT_UUID_SOLO) | [Buy Team →](https://dry.lemonsqueezy.com/checkout/buy/VARIANT_UUID_TEAM) | [Talk to us →](mailto:license@yemelianov.dev?subject=Dry%20Pilot) |

Machine and user counts are honor-declared — embedded in your license token and stamped into
every report footer for audit, not enforced by the binary. Pilot is a 90-day Team-tier license
plus guided onboarding through the [pilot playbooks](/guide/), invoiced manually; email us to
start one.

Checkout is handled by Lemon Squeezy, our merchant of record — they collect VAT/sales tax where
applicable and email a receipt. A license token arrives by email within a few minutes of
purchase; see [Activation](/activate) for what to do with it.

## Eval mode vs. licensed

The CLI is fully functional without a license — there is no crippled trial. The difference is
scope and a couple of guardrails:

| | Eval mode (no license) | Licensed |
|---|---|---|
| `review-gcode`, `verify`, `trace-gcode`, `compare`, `explain`, `rewrite-gcode` | ✅ full output | ✅ full output |
| Report footer | `"mode": "evaluation"` + a printed `EVALUATION — not for production gating` banner | `"mode": "licensed"`, stamped with your licensee name and tier |
| `dry upload` (Moonraker gate) | ❌ refuses before any network call, points here | ✅ |
| Result/finding caps | None — evaluate on real jobs at full depth | None |
| CI gating on exit code | ✅ works today | ✅ |
| Air-gapped use | ✅ (offline verification either way) | ✅ |

In short: download a release, point it at your own G-code, and see the real findings — the
license is what turns on the print-side upload gate and removes the evaluation banner.

## Support terms

We're honest about this rather than implying more: pre-1.0, there is no SLA. Solo gets
best-effort email support; Team gets priority email support (same channel, higher queue
priority) — see the full [support matrix](https://github.com/dmytro-yemelianov/dry/blob/main/docs/16-support-matrix.md)
for what's Supported vs. Experimental in the engine itself. Bugs go through GitHub issues;
security or safety reports go to
[`SECURITY.md`](https://github.com/dmytro-yemelianov/dry/blob/main/SECURITY.md).

## Where Dry fits

Dry is a deterministic policy gate for sliced G-code — source-located findings, reproducible
audit reports, CI exit codes, and an upload gate — with an *advisory* LLM layer on top
(`explain --llm`), not an LLM-as-judge in the critical path. As of this writing that combination
is not sold anywhere else: the closest published work is the 2026 pre-print **LLM-ADAM**
(LLM-as-judge anomaly detection, no released tool, architecturally the inverse of Dry's
deterministic-gate-plus-advisory-LLM design) and **GlitchFinder** (OOPSLA 2025, formal G-code
semantics targeting slicer-fidelity bugs, a research prototype). We read both as third-party
validation that the problem is real, not as competition yet. Dry does not attempt CNC
collision simulation (Vericut/NCSIMUL territory) — see the
[support matrix](https://github.com/dmytro-yemelianov/dry/blob/main/docs/16-support-matrix.md)
for exactly what is and isn't covered.
