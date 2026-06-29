# Security policy

Dry is pre-1.0 software. We take security and **machine-safety** issues seriously — a toolpath compiler
can produce output that drives real hardware.

## Reporting a vulnerability

Please report security issues **privately**, not in public issues:

- Use GitHub's **[Report a vulnerability](https://github.com/dmytro-yemelianov/dry/security/advisories/new)**
  (Security → Advisories) on this repository, or
- open a regular issue **only** for non-sensitive hardening suggestions.

Include: affected component (engine / CLI / Python / TS / wasm / release artifact), a description, and a
minimal reproduction (an input file + the command, where possible).

We aim to acknowledge a report within a few days and to coordinate a fix and disclosure timeline with you.
Please give us reasonable time to address the issue before public disclosure.

## What counts

- **Safety-relevant** correctness bugs: a toolpath that `verify` passes but is unsafe within the
  documented rule catalog, a codec that mis-decodes, or an emitter that produces incorrect motion.
- **Memory-safety / soundness** issues in the Rust engine or the bindings.
- **Supply-chain / release-artifact** issues (a tampered or mis-built release asset).

Note the scope boundary: `verify` enforces exactly the documented rule catalog
([`docs/11-profiles-and-reports.md`](docs/11-profiles-and-reports.md)) — a clean report is **not** a
guarantee of unattended-print safety (see [`docs/14-known-limitations.md`](docs/14-known-limitations.md)).

## Supported versions

Security fixes target the **latest release** and `main`. Given the pre-1.0 status, older tags are not
maintained — upgrade to the latest `vX.Y.Z` (see [`docs/12-releasing.md`](docs/12-releasing.md)).
