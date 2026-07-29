# Dry IR Language and Formal Assurance — Handover (2026-07-29)

This is the current resume point for the Dry IR language/ecosystem and FM1 mathematical-assurance
work. It is separate from the Dry Cloud deployment handover at
`docs/superpowers/plans/2026-07-28-dry-cloud-HANDOVER.md`.

Start by reading:

- `docs/02-roadmap.md` — deferred D1 scope, entry gate, non-goals and exit gate;
- `docs/04-tasks.md` — D1 epics and current FM1 status;
- `docs/20-dry-ir-ecosystem-implementation-plan.md` — merge-sized D1 implementation sequence;
- `docs/21-mathematical-assurance-plan.md` — assurance layers, claims and FM1 packets;
- `proofs/claims.toml` — machine-readable assurance claims;
- `proofs/feature-numeric-boundaries-v0.toml` — source-pinned binary64 boundary inventory;
- `proofs/feature-planar-numeric-profile-v0.toml` — provisional numeric preconditions and budgets.

## Repository and publication state

- Repository: `dmytro-yemelianov/dry`
- Branch: `feat/dry-cloud`
- Draft PR: <https://github.com/dmytro-yemelianov/dry/pull/177>
- Assurance implementation head before this handover-only change:
  `bf9113a feat(formal): bound orientation angular error`
- Local and remote branch were synchronized and the worktree was clean before this handover file was
  created.
- CI run `30484518002` for `bf9113a` is fully green: core native/wasm, formal assurance, Python,
  TypeScript, wasm binding, docs, vectors, benches and dependency audit all passed.

GitHub authentication has multiple local accounts. For this repository:

```sh
gh auth switch --user dmytro-yemelianov
gh auth setup-git
git push
gh auth switch --user miwaniza
gh auth setup-git
```

The second switch restores the owner's previously active account. A push authenticated as
`miwaniza` fails with HTTP 403 for this repository.

## What has landed

The current branch contains a continuous FM1 proof/refinement line from exact language semantics to
bounded planar binary64 behavior.

### Assurance foundation and logical IR

- Lean 4/Mathlib project, CI warnings-as-errors build and schema-backed claim validation.
- Exact dimension/unit algebra and deposition dimensional identity.
- Serializer-neutral L2 v0 logical syntax, structured well-formedness failures and
  `validate_success_iff`.
- Structural L2 logical equality theorem.
- Generated TSV/JSON well-formedness fixtures with independent schema checks.

### Feature language semantics and implementation refinement

- Exact planar transform action.
- Generic `Feature`, ordered `Group` and `Repeat` semantics.
- Source ordering, repeat decomposition, exact operation counts, resource-budget preservation and
  deterministic evaluation.
- Checked success/first-error semantics for the bounded production feature subset.
- Lean-generated refinement corpora consumed independently by Rust.
- Parent-first Feature/Repeat composition-tree shape bridge.
- Thirty-five pinned, compiled `features.rs` mutations; all 35 are killed by named proof fixtures.

### Numeric assurance

The recent commit chain is:

| Commit | Packet |
|---|---|
| `c4bbc22` | Parametric local rounding-error formulas |
| `945361d` | Reject non-finite feature arithmetic inputs |
| `7f34533` | Scoped binary64 local operation bounds |
| `cf63656` | Ordered degree-to-radian error bound |
| `fbf4c02` | Conditional pinned-`libm` coefficient bound |
| `7dd5e3e` | Sequential repeat-transform accumulation |
| `cc784bf` | Arbitrary parenthesized composition-tree accumulation |
| `716ba3a` | Selected-path composition-tree refinement |
| `15b6bfa` | Lean-generated native numeric intervals |
| `999a15e` | Native transform-application refinement |
| `c16dc95` | Tree point/Arc/orientation application bounds |
| `25bc6d3` | Nested end-to-end transform-application refinement |
| `bf9113a` | Unit orientation nonzero/angular theorem and native contract corpus |

Current published profile state:

- 13 source-pinned binary64 boundaries;
- 9 provisional proof-precondition limits;
- 17 budgets: 16 bounded and 1 implementation policy;
- 27 registered assurance claims with no Lean placeholders;
- local orientation XY error at most `2^-29`;
- arbitrary-tree orientation XY component error at most `2^-8`, with Z exact;
- for an exact unit orientation under all tree/application premises, the computed vector is nonzero
  and its unoriented angular error is at most `1/4` radian.

The newest implementation corpus contains six exact-rational vectors. Rust independently confirms:

- the zero vector is rejected by `resolve_checked`;
- signed axis vectors and `(3/5, 0, 4/5)` receive no `orientation-not-unit` finding;
- `(0, 0, 2)` and `(1, 1, 0)` are accepted by resolution and flagged by verification.

Relevant sources:

- `formal/Dry/Numeric/Orientation.lean`
- `formal/Dry/Tests/OrientationContractFixtures.lean`
- `proofs/fixtures/orientation-contract-refinement-v0.json`
- `crates/core/tests/orientation_refinement.rs`

## Assurance boundary — do not overclaim

The project is aligned with Lean/Coq/Dafny/CompCert-style formal methods in architecture, but Dry is
not yet an end-to-end verified compiler.

Keep these distinctions explicit:

1. **Exact Lean semantics are not native Rust refinement.** An abstract theorem may remain
   refinement-pending.
2. **The numeric profile is `assurance-precondition-only`.** Most magnitude limits are not runtime
   language validity limits.
3. **The `libm` 0.2.16 one-ULP statement is an imported upstream MPFR-testing contract**, not an
   exhaustive source proof.
4. **The large repeat/tree translation ceilings are conservative proof envelopes**, not production
   tolerances.
5. **The orientation contract corpus is finite.** Exact rational nonzeroness/unit length is not
   generally equivalent to the implementation's binary64 `sqrt` behavior and `1e-6` verifier
   tolerance.
6. **The angular theorem is conditional.** It inherits every profile, range, rounding, imported-libm
   and exact-unit premise from the tree application proof.
7. **Full `SE(3)`, normalized quaternion semantics and named frame graphs remain D1.3-dependent.**
8. A clean verifier report establishes only the predicates actually executed with complete coverage;
   it does not certify firmware, hardware or manufacturing-process safety.

Important binary64 edge cases still outside the orientation corpus:

- finite components can overflow during `i*i + j*j + k*k`, producing infinite magnitude;
- tiny nonzero components can underflow during squaring and be rejected as zero;
- vectors near the verifier's `1e-6` boundary can disagree with exact rational unit classification;
- signed-zero, NaN payload, native/wasm and normalization-policy behavior are not universally refined.

## Recommended next packet: FM1.5a orientation-aware resolution

Do a narrow resolver packet before attempting all of FM1.5. The goal is to connect the newly proved
orientation contract to L1→L2 state propagation and the existing L2 well-formedness model.

### Scope

1. Add an executable Lean model for a minimal resolver subset:
   - `Orient(i,j,k)`;
   - position-changing `Move`;
   - resolver state with the current optional orientation;
   - emitted segments carrying the orientation active at emission.
2. Specify the representation boundary:
   - `None` is the stored default and semantically denotes `+Z`;
   - explicit orientation is last-write-wins;
   - orientation changes do not themselves emit motion.
3. Prove:
   - fold determinism;
   - default moves carry `None`;
   - an explicit orientation propagates to every later emitted segment until replaced;
   - a later orientation does not rewrite earlier segments;
   - zero/non-finite orientation rejects before successful resolution;
   - if every explicit orientation is unit, emitted segments satisfy the selected L2 orientation
     well-formedness condition;
   - non-unit but nonzero input may resolve successfully and is a verifier obligation, matching current
     production behavior.
4. Generate ordered sequence fixtures, not only isolated vectors:
   - default move;
   - orient → two moves;
   - orient A → move → orient B → move;
   - zero before motion;
   - non-unit accepted then verifier finding.
5. Consume the generated fixtures through production `resolve_checked` and `verify`.
6. Register separate abstract and implementation-scoped claims. Do not mark universal native numeric
   refinement checked from a finite corpus.
7. Add targeted mutation evidence for `resolve.rs` orientation state and `verify.rs` unit
   classification. Do not silently expand the existing `features.rs`-only mutation manifest; either
   introduce a clearly scoped resolver/verifier manifest or deliberately record mutation work as the
   next obligation.

Likely new files:

```text
formal/Dry/Semantics/ResolveOrientation.lean
formal/Dry/Tests/ResolveOrientationFixtures.lean
proofs/fixtures/resolve-orientation-refinement-v0.json
proofs/fixtures/resolve-orientation-refinement-fixtures.schema.json
crates/core/tests/resolve_orientation_refinement.rs
```

Import the semantic module from `formal/Dry.lean`, extend `tools/check_proof_fixtures.py`, update
`proofs/claims.toml`, and update FM1.5 status in `docs/04-tasks.md` and
`docs/21-mathematical-assurance-plan.md`.

### Production anchors

- `crates/core/src/resolve.rs:186` — ordered validation, including finite/nonzero orientation input;
- `crates/core/src/resolve.rs:391` — resolver orientation state starts as `None`;
- `crates/core/src/resolve.rs:416` — `Op::Orient` replaces current orientation;
- `crates/core/src/resolve.rs:436` and later segment constructors — orientation propagation;
- `crates/core/src/verify.rs:602` — `orientation-not-unit` rule and `1e-6` threshold;
- `formal/Dry/Language/Common.lean` — exact `Vec3.IsUnit`;
- `formal/Dry/Language/WellFormed.lean` — L2 orientation well-formedness.

Before editing code, use the repository knowledge graph per `AGENTS.md`:

1. `search_graph`;
2. `trace_path`;
3. `get_code_snippet`;
4. `query_graph` or `get_architecture` if needed;
5. use `rg` for non-code files, literals or when graph coverage is insufficient.

## Validation commands

Run the proportional checks during development:

```sh
cd formal
lake env lean Dry/Semantics/ResolveOrientation.lean
lake env lean --run Dry/Tests/ResolveOrientationFixtures.lean
lake build --wfail
```

Then run all repository gates before publication:

```sh
python3 tools/validate_proof_claims.py
python3 tools/validate_numeric_boundaries.py
python3 tools/check_proof_fixtures.py
python3 -m unittest discover -s tools/tests -p 'test_validate_proof_claims.py'
python3 -m unittest discover -s tools/tests -p 'test_validate_numeric_boundaries.py'
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy -p dry-core --target wasm32-unknown-unknown --locked -- -D warnings
cargo test --workspace --all-features --locked
python3 tools/check_feature_mutations.py
```

If a new resolver/verifier mutation manifest is added, run its checker separately and add it to CI.

## Working rules

- Preserve unrelated work in a dirty tree; stage explicit paths.
- Use `apply_patch` for edits.
- Keep claim scopes honest: abstract, numeric and implementation-refinement statuses are independent.
- Generated fixture snapshots must be reproducible from Lean and schema-valid.
- A mutation must compile and be killed by its named witness; compilation failure is invalid evidence.
- Do not change public validity to enforce the provisional numeric profile without an explicit contract
  decision.
- Keep D1 intentionally deferred until its roadmap entry gate is met; FM1 proof work must follow frozen
  language contracts rather than silently defining them.
