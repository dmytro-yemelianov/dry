# Clean-room provenance & licensing discipline

Dry is **proprietary** and **independent of FullControl's code**. FullControl (and its fork) are GPLv3;
Dry must therefore never be a derivative work of them. This document is the discipline that protects
Dry's provenance, ownership, and commercial licensing boundary. *(This is engineering policy, not legal
advice—confirm customer terms and provenance conclusions with counsel.)*

## The rule

FullControl is used **only** as:
1. **Inspiration** — design ideas (the multi-level IR, toolframe, passes, flavors). Ideas and
   architecture are not copyrightable; reimplementing them from scratch is fine.
2. **A behavioural oracle, at dev/CI time only** — FullControl is *run* to generate the expected outputs
   (g-code, metrics) that Dry's conformance tests target. Matching functional output (identical g-code)
   is interoperability, not copying.

FullControl is **never**:
- copied into Dry's source (no porting the Rust kernel / Python verbatim — reimplement every line);
- shipped in, linked into, or part of any Dry release artifact (the oracle is a dev/CI dependency
  isolated under `conformance/oracle/`, excluded from published packages);
- the source of vendored files (test code, profiles) — those are regenerated (see below).

## What this means in practice

- **Code:** written from the spec in `docs/` and first principles. If you find yourself looking at a
  FullControl source file to copy structure, stop — work from the *behaviour* (its output) instead.
- **Conformance corpora:** **generated** by running the oracle, not vendored from FullControl's repo.
  The generator script lives in `conformance/oracle/` and is not part of any Dry release. Once Dry is
  self-consistent, its own outputs become the reference and the oracle is retired (FullControl's role
  asymptotes to zero — "a fading scent").
- **Device profiles:** regenerated from **primary sources** (printer/firmware documentation,
  slicer-neutral configuration) or treated as factual numeric data — not lifted as FullControl's profile
  files. Provenance recorded per profile.
- **Math/behaviour parity:** reproducing identical g-code bytes is a *goal*, achieved by independent
  implementation measured against the oracle — not by copying the code that produces them.

## Provenance log

Keep a short, auditable record (this file + commit history) that Dry was implemented clean-room: the
spec predates the code, the code references the spec (not FullControl source), and the oracle is
quarantined to dev/CI. If contributors have read FullControl source, note it — ideas are fine, copying
expression is not.

## Why proprietary (not GPLv3)

Dry is commercial infrastructure ("LLVM for machine motion") with a publicly documented integration
contract and privately delivered implementations. Proprietary licensing preserves control over source,
packages, and embedding rights while allowing customer-specific evaluation and production agreements.
The clean-room discipline above is what makes independent commercial licensing defensible despite the
FullControl oracle being GPLv3.

Copies previously distributed under Apache-2.0 remain under those attached terms. The proprietary
notice applies prospectively; it cannot revoke rights already granted for earlier copies.
