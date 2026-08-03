# Licensing

Dry is proprietary software. Source code, engines, SDK implementations, WebAssembly, packages, and
release artifacts are available only to authorized users under a separate written agreement.

This documentation is public so teams can evaluate the architecture, interfaces, supported workflows,
and integration fit. Public access does not grant a software licence or a right to redistribute the
documentation.

Versions or copies previously distributed under Apache-2.0 remain governed by the terms attached to
those copies. The current proprietary notice applies prospectively.

## How to get licensed

The CLI itself is self-serve: run it unlicensed and every command works in evaluation mode
(reports stamped `"mode": "evaluation"`, an `EVALUATION — not for production gating` banner, and
`dry upload` refusing until a license is active). See [/pricing](/pricing) for the three tiers
(Solo, Team, Pilot) and checkout, and [/activate](/activate) for `dry license activate`, the
`DRY_LICENSE` CI variable, and grace/renewal behavior. The
[CI-gate quickstart](/guide/ci-gate-quickstart) walks the whole path from install to a gated
pipeline.

For SDK embedding, custom terms, or anything outside the published tiers, email
[license@yemelianov.dev](mailto:license@yemelianov.dev).

## Dry Proprietary Notice

<!--@include: ../../LICENSE-->
