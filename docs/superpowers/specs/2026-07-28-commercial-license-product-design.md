# Dry Commercial Product — Licensed CLI for Post-Slicer Review (Design)

**Date:** 2026-07-28
**Status:** Approved design, pre-implementation
**Owner:** Dmytro Yemelianov

## Purpose

Turn the shipped review/audit engine capability into a product a stranger can buy and
self-onboard: public product page → purchase → license issuance → authenticated value
(`dry review`/gate running in their CI) **within one hour, no call required**. Pilot
deals ($1.5k–$5k) remain a manual motion on the same rails.

This is the first product of the four surfaces in `docs/05-product-directions.md` /
`docs/site/marketing/` (wedge → capability registry → machine SaaS → CAD embed). The
wedge ships first because the engine side is already on `main` (review, verify, trace,
compare, explain, explain --llm, rewrite modes, kinematics rules, import-printer-cfg,
Moonraker gate) — what's missing is entirely commercial wrapper.

## Decision Log (final)

| Area | Decision |
|---|---|
| Product | The Dry CLI itself under a commercial license — no second brand. Marketing wedge: "deterministic post-slicer review and gating." |
| Delivery | Licensed CLI + CI integration. SDKs/wasm are NOT part of this product (embed SDK = separate future product; enforcement never touches `dry-core` or SDK crates). |
| Success criterion | Stranger buys + self-onboards ≤ 1 hour, docs-only. |
| License enforcement | Ed25519-signed **offline** license token verified by the binary. No phone-home, no telemetry, works air-gapped. |
| Payments | Merchant of record: **Lemon Squeezy** (Paddle as fallback if LS rejects the product). Pilots by manual invoice. |
| Trial | **Eval mode built into the binary** (see below). Artifacts stay on public GitHub Releases — distribution doubles as trial funnel. |
| Tiers | **Solo $990/yr** (1 user, ≤3 machines) · **Team $4,990/yr** (≤10 users, ≤25 machines, priority email support) · **Pilot $1.5k–5k** (90-day Team license + hands-on onboarding via `docs/pilots/`). |
| Billing cadence | **Annual-first** (deviates from the $99–$499/mo hypothesis in `docs/site/marketing/index.md`): monthly billing + offline keys ⇒ monthly key rotation in customers' CI — friction that kills self-serve. Monthly can be added later if demanded. |
| Machine counts | Honor-declared, embedded in the license, stamped into report footers. Auditable, not enforced — consistent with offline keys. |

## License token format

One line, email-and-paste friendly:

```
DRY-LICENSE-V1.<base64url(payload JSON)>.<base64url(ed25519 signature over the payload bytes)>
```

Payload fields: `id` (ULID), `licensee` (display name), `email`, `tier`
(`solo|team|pilot`), `machines` (int), `issued`, `expires` (RFC3339), `key_id`.
The signature covers the exact base64url payload bytes — no canonical-JSON pitfalls.
Public keys (keyed by `key_id`) are embedded in the binary; a key set enables rotation
without invalidating issued licenses.

### CLI surface (`crates/license` + `dry-cli` integration)

- `dry license activate <token-or-file>` — verifies, stores at the platform config dir.
- `dry license status` — licensee, tier, expiry, entitlements, grace state.
- **`DRY_LICENSE` env var** — the primary CI path (one GitHub Actions secret; no file
  management). Env var takes precedence over the stored file.
- Enforcement points: report-producing commands (`review-gcode`, `verify`, `compare`,
  `explain`, `rewrite-gcode`) stamp mode + licensee into reports; `dry upload`
  (Moonraker gate) **requires** a valid license.
- **Never brick:** invalid/expired license ⇒ 14-day grace (warning banner) ⇒ eval mode
  with a loud notice. Never a refusal — a lapsed card must not break a customer's CI.

### Eval mode (the funnel)

Unlicensed = fully functional review/verify/trace/compare/explain, with:
1. every report stamped `"mode": "evaluation"` + an "EVALUATION — not for production
   gating" banner in human-readable output;
2. `dry upload` refuses, printing the licensing pointer.

No size/count caps (caps generate support load and punish evaluation depth; the
watermark + gate carry the commercial boundary).

## Issuance service (one Cloudflare Worker)

Stateless except for an audit log. If the Worker dies, every issued license keeps
verifying offline forever.

- `POST /webhook/lemonsqueezy` — HMAC-verified. On order/renewal events: build payload,
  sign with the private key (Worker secret), email the token to the buyer (Workers
  `send_email` binding), append to D1 `licenses` (id, email, tier, expires, order_id,
  created).
- `POST /admin/issue` — bearer-secret-authed manual issuance (pilots, reissues,
  goodwill extensions).
- Renewal webhook ⇒ automatic reissue email with extended expiry.
- Refund webhook ⇒ logged as revoked in D1 (offline keys cannot be recalled —
  accepted risk, bounded by expiry).

## Self-onboard surface (dry-public-docs site)

- **/pricing** — tiers, checkout links (LS overlay), support expectations stated
  honestly (email, best-effort; priority for Team; no SLA pre-1.0 per
  `docs/16-support-matrix.md`).
- **/license** — activation, `DRY_LICENSE` CI setup, air-gapped FAQ, "what we collect:
  nothing" (offline verification), grace/renewal behavior.
- **"60 minutes to gated CI" quickstart** — the product spine: install (public GitHub
  Release) → eval run on the reader's own G-code → buy → set the secret → gate goes
  green. A copy-paste GitHub Actions job is the centerpiece.
- Rewrite `docs/site/licensing.md` from "contact the owner" to the real flow.
- Optional same-week: attach `dry.yemelianov.dev` to the docs Pages project (zone
  exists; one custom-domain click) and send license emails from that identity.

## Sequencing

1. **Cut `v0.4.0` now** — ships the engine being sold; proves the release rails
   (hosted CI confirmed green as of 2026-07-26). Independent of everything below.
2. `crates/license` + CLI integration + eval mode → lands in **v0.5.0**.
3. Worker + Lemon Squeezy products + docs-site pages (no repo release required).
4. **Launch** = v0.5.0 tag + pricing page live + one completed **test-mode purchase
   round-trip** (buy → email → activate → licensed CI run).

## Error handling

- Clock skew / air-gapped hosts: expiry checked against system time with the 14-day
  grace; `license status` surfaces the effective state so CI logs are diagnosable.
- Tampered/wrong-key/malformed tokens: precise, distinct error messages; always fall
  through to eval mode (never a hard exit from a report command).
- Worker outage: LS retries webhooks; purchases are never lost. Issued licenses
  unaffected.
- Lost license email: manual reissue via `/admin/issue` (D1 lookup by email/order).

## Testing

- `crates/license` unit tests: valid, expired, in-grace, tampered payload, tampered
  signature, wrong key, unknown `key_id`, rotation (old key still validates).
- CLI integration tests with a committed **test keypair** fixture: eval vs licensed
  output stamps, `DRY_LICENSE` precedence, upload refusal in eval, grace behavior.
  Production public keys differ from test keys; the test private key is explicitly
  non-secret.
- Worker: `@cloudflare/vitest-pool-workers` — HMAC verification (reject bad
  signatures), issuance path with stubbed email + D1, admin auth, refund logging.
  **No real emails or LS calls from tests.**
- Pre-launch E2E: LS test mode purchase → real email arrives → activate → licensed
  run in a scratch CI workflow.

## Risks & accepted trade-offs

- **No revocation** of issued tokens (refund abuse bounded by license expiry).
- **Honor-based seat/machine counts** — deliberate; the audit stamp in reports is the
  social enforcement.
- **EULA:** I'll draft structure/terms, but a human lawyer must review before real
  money flows. Placeholder milestone in the plan.
- **Lemon Squeezy dependency** for checkout/tax; mitigated by MoR model (they carry
  compliance) and by keys being independent of LS availability post-purchase.
- **Support surface**: `docs/16-support-matrix.md` promises no SLA pre-1.0; the
  pricing page must state support terms honestly rather than imply more.

## Competitive appendix (verified 2026-07-28)

Two web-research sweeps (10 named CNC/G-code tools; direct wedge hunt across 3DP
linters, farm QA, slicer vendors, LLM research):

- **The wedge is unoccupied.** No product combines deterministic policy rules +
  source-located findings + reproducible audit reports + CI exit codes + upload
  gating, for FFF or otherwise.
- Nearest artifacts: an OctoPrint metadata-check plugin (trivial scope);
  **LLM-ADAM** (2026 research, pre-print anomaly detection, no released tool,
  architecturally inverted — LLM-as-judge vs Dry's deterministic-gate +
  advisory-LLM); **GlitchFinder** (OOPSLA 2025, formal G-code semantics aimed at
  slicer-fidelity bugs; prototype). Print-farm QA is camera-based only; slicer
  vendors ship no post-slice verification/diff/audit.
- **Where Dry is deliberately behind:** Vericut/NCSIMUL-class collision simulation
  (stock/fixture/tool models, CNC). Out of claimed scope; never position against it.
- **Signals:** G-Wizard Editor (hobbyist CNC "linter") is discontinued — evidence for
  B2B/CI positioning over hobbyist pricing. Two serious papers in two years mean the
  space is becoming academically visible — ship inside the window; cite the research
  as third-party problem validation in marketing.

## Out of scope

SDK/embed licensing (separate product), customer portal/accounts (graduate later —
offline keys don't require it), monthly billing (add on demand), private/gated
downloads, seat enforcement, revocation infrastructure, CNC verticals, mesh slicing.
