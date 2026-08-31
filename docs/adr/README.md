# Architecture decision records

Architecture decision records freeze cross-cutting language and implementation choices that must remain
reviewable independently of code.

| ADR | Status | Decision |
|---|---|---|
| [`0001-formal-assurance-constitution.md`](0001-formal-assurance-constitution.md) | Accepted | Separate abstract proof, numeric refinement, implementation refinement and physical evidence; use explicit semantic relations and a checked claim registry. |
| [`0002-numeric-ingress-and-emission-gates.md`](0002-numeric-ingress-and-emission-gates.md) | Accepted | Validate every ingress that builds an IR quantity and gate emission as defence in depth; use a postcondition at `resolve` rather than an input magnitude bound; refuse rather than clamp or emit vacuously; require an independent oracle for validity claims. |
| [`0003-hosted-verification-service.md`](0003-hosted-verification-service.md) | Accepted | There will be a hosted verification service and it is `containers/verify-runner`, deployed as a container image; `crates/cloud` stays an archived spike because a Workers isolate was measured not to fit. Cloudflare Containers is a way to run the artifact, not an alternative to it. |
