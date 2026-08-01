# ADR 0001 — formal assurance constitution

- **Status:** Accepted
- **Date:** 2026-07-28
- **Workstream:** FM1.1

## Context

Dry already makes several different kinds of correctness statement:

- exact structural and `f64`-bit codec round trips;
- deterministic language lowering;
- tolerance-based geometric conformance;
- verifier findings under supplied contracts;
- native/wasm parity;
- target emission and hardware evidence.

Those statements do not have the same strength. In particular, an algebraic proof over real numbers
does not prove a binary64 implementation; a clean verifier report does not prove unmodeled behavior
safe; and a successful reference-machine run does not prove a compiler theorem.

The language/ecosystem program needs stable terminology before proof work expands with the language.

## Decision

### Assurance layers

Dry separates assurance into four layers:

1. **Abstract semantics:** machine-checked definitions and theorems over exact mathematical domains.
2. **Numeric refinement:** checked error bounds or an explicit empirical status for binary64 and sampled
   algorithms.
3. **Implementation refinement:** independent evidence that Rust implements the modeled function.
4. **Physical evidence:** pinned controller/machine/process qualification outside the compiler theorem.

No result silently inherits a stronger layer.

### Semantic relations

Every registered claim uses exactly one primary relation:

| Registry value | Mathematical notation | Meaning |
|---|---|---|
| `exact` | `=` | definitional or structural equality |
| `bit-exact` | `≡bits` | equal serialized numeric bit patterns and structure |
| `trace-exact` | `≡trace` | equal ordered abstract geometric/process trace |
| `approximate` | `≈ε` | difference bounded by a named tolerance |
| `observational` | `≡obs(O)` | indistinguishable under named observations |
| `capability-refinement` | `⊑C` | target behavior refines requirements under profile `C` |
| `invariant-preservation` | `preserves(I)` | a named invariant survives a pass |
| `rejection` | `rejects(P)` | invalidity predicate `P` cannot cross the boundary successfully |

Unqualified “semantics-preserving” is not an acceptable public pass contract.

### Claim status

A claim records three independent statuses:

- `abstract`: `specified`, `proved` or `not-applicable`;
- `numeric`: `pending`, `bounded`, `empirical` or `not-applicable`;
- `refinement`: `pending`, `checked` or `not-applicable`.

An implementation-scoped claim requires:

- `abstract = "proved"`;
- `numeric = "bounded"` or `"not-applicable"`; and
- `refinement = "checked"`.

Abstract claims may be published as abstract theorems while numeric or implementation work is pending,
but their wording must say so.

### Proof environment

FM1 starts with Lean 4 and Mathlib, pinned to matching stable releases in `formal/lean-toolchain` and
`formal/lakefile.toml`. This decision is accepted after representative planar-transform and ordered-fold
proofs build without placeholders. Replacing the prover requires a new ADR and a migration plan for
existing theorem and claim identifiers.

### Traceability

`proofs/claims.toml` is the authoritative claim registry. Its schema and validator require:

- a stable claim id and theorem name;
- language/spec version and dialect boundary;
- relation, numeric domain, assumptions and exclusions;
- Lean source and Rust implementation links;
- numeric/refinement evidence whenever the corresponding status says it exists.

The registry validator must not import or execute `dry-core`.

## Consequences

- Proof obligations are defined by public semantics instead of Rust module structure.
- Floating-point work is visible rather than hidden inside exact-real proofs.
- Optimizations must state what they preserve and what they intentionally change.
- Opaque target commands and missing verifier models block stronger claims.
- Semantic changes require a proof-impact classification.
- Broad product wording such as “the Dry compiler is formally verified” remains prohibited unless it
  names the supported subset and all applicable assurance layers.

## Rejected alternatives

- **Tests alone:** necessary for implementation evidence, insufficient for universal language laws.
- **Rust verification alone:** useful for bounded implementation properties, insufficient as the public
  multi-language semantics.
- **One global equivalence relation:** hides intentional changes and tolerance/loss boundaries.
- **Model IEEE-754 as reals:** produces attractive but invalid implementation claims.
- **Treat hardware qualification as proof:** evidence remains specific to the pinned physical setup.
