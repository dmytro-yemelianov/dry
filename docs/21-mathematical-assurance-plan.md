# Dry language mathematical assurance — implementation plan

This document plans mathematical proofs for the Dry language and compiler. It complements the public
Dry IR v0 specification and the deferred D1 language/ecosystem program. It is an assurance plan, not a
claim that the current compiler, verifier or emitted machine code has already been formally verified.

The workstream identifier is **FM1**. FM1 may begin on the stable v0 semantics before D1 activation, but
proofs for future quantities, frames, channels, capabilities and L3 contracts must follow the contract
decisions made by D1.1–D1.9. Proof work must not freeze an experimental API by accident.

## 1. Outcome and assurance boundary

FM1 is complete when Dry publishes:

1. a machine-checked mathematical model of the public L0–L3 semantic boundaries used by the reference
   workflows;
2. checked theorems for the exact language properties named in this plan;
3. explicit numeric error bounds for the properties implemented with floating-point or sampled
   geometry;
4. an executable refinement bridge connecting the mathematical model to the Rust implementation and
   public conformance vectors;
5. proof-status metadata that distinguishes proved, bounded, tested and out-of-model claims; and
6. a reproducible proof build that an independent contributor can run without private infrastructure.

The intended top-level compiler theorem is deliberately conditional:

> For a well-formed program, a validated registry/profile and a supported target, compilation either
> returns a located failure or returns a target program whose modeled trace refines the source trace
> under the declared equivalence, tolerance, capability and loss policy.

This theorem does **not** imply collision freedom, process success, firmware correctness, hardware
correctness, certification or safe unattended operation unless those assumptions and models are
separately present and named.

## 2. Claim vocabulary

Every proof obligation, pass contract and public assurance statement must use one of these relations.
Plain “preserves semantics” is not precise enough.

| Relation | Meaning | Typical use |
|---|---|---|
| `=` | Definitional or structural equality | deterministic normalization, exact codec round-trip |
| `≡bits` | Equality of serialized numeric bit patterns and structure | Dry IR v0 codec preservation |
| `≡trace` | Same ordered geometric/process trace in the abstract semantics | exact lowering and exact optimization |
| `≈ε` | Trace or metric difference bounded by named tolerance `ε` | arc fitting, spline flattening, float implementation |
| `≡obs(O)` | Indistinguishable under an explicit set of observations `O` | target text differences with equal modeled motion |
| `⊑C` | Target trace refines source requirements under capability profile `C` | L2 → L3 lowering |
| `preserves(I)` | Invariant `I` holds after a pass whenever its preconditions and `I` held before | pass contracts |
| `rejects(P)` | All inputs satisfying invalidity predicate `P` are rejected before the unsafe boundary | bad frames, units, capabilities or opaque code |

Each theorem record must also identify its numeric domain, assumptions, covered syntax, excluded syntax
and implementation-refinement status.

## 3. Four assurance layers

Dry must not conflate abstract mathematics, floating-point execution, implementation behavior and
physical machines.

### 3.1 Layer A — abstract language semantics

The mathematical model uses exact integers, rationals and real numbers as appropriate. It defines:

- syntax and well-formedness for the supported L0, L1, L2 and L3 subsets;
- typed quantities and dimensions;
- points, vectors, rotations, rigid transforms and named frame graphs;
- process/channel state and its explicit propagation rules;
- small-step or fold semantics for ordered programs;
- traces containing geometry, toolframe and typed process observations;
- capability requirements and target support;
- diagnostics, losses and opaque regions as semantic results rather than log strings.

Layer A supports exact theorems. It never silently models an IEEE-754 operation as exact real
arithmetic.

### 3.2 Layer B — finite-precision refinement

Layer B connects abstract expressions to binary64 execution. Each operation is classified as:

- **exact in range**, such as bounded integer counts and selected additions/multiplications for which an
  exactness precondition is proved;
- **correctly or deterministically rounded**, only where the pinned implementation provides that
  contract;
- **interval bounded**, where the result is enclosed by a computed interval;
- **empirically characterized**, where no proof has landed and the status is explicitly not “proved”.

Transcendentals (`sin`, `cos`, `atan2`), `sqrt`, normalization, accumulated path length and sampled
splines require Layer B treatment. Native/wasm parity tests remain useful evidence but are not a bound
on error from the real-valued semantics.

### 3.3 Layer C — implementation refinement

Layer C checks that Rust implements the modeled function:

- shared, versioned model fixtures generated independently of `dry-core`;
- property tests over valid and invalid inputs;
- differential checks between the proof model and Rust;
- mutation tests showing that the bridge catches plausible semantic defects;
- traceable links from Rust functions and public spec clauses to theorem names.

Python, TypeScript and wasm remain thin surfaces. Their obligation is schema/API refinement to the Rust
engine plus cross-SDK semantic equivalence, not a duplicate proof of the compiler.

### 3.4 Layer D — physical evidence

Layer D qualifies a pinned controller, machine and process setup through versioned, reproducible
protocols. Evidence records the exact machine/profile/controller, test program, tolerances, expected
observations, safety owner, abort criteria, results and artifact hashes.

Physical evidence may validate named assumptions for that setup, but it is not inherited by the
abstract, numeric or implementation layers and does not turn a bounded compiler theorem into a
universal manufacturing-safety or process-success theorem.

## 4. Proof technology and repository layout

Use **Lean 4 with Mathlib** for the initial formal model and theorem corpus. Lean is the default because
the work needs algebra, finite maps/lists, real analysis and executable definitions in one checked
environment. A tooling ADR in FM1.1 must pin versions and confirm the decision with two representative
proofs before the repository treats it as permanent.

Rust property tests and the existing independent conformance implementation form the refinement bridge.
An additional Rust verifier technology may be piloted for bounded implementation properties, but it is
not a substitute for the language semantics and is not on the critical path.

Planned layout:

```text
formal/
  lean-toolchain
  lakefile.toml
  lake-manifest.json
  Dry/
    Numeric/
      Quantity.lean
      FloatModel.lean
      ErrorBudget.lean
    Geometry/
      Vec3.lean
      Rotation.lean
      Transform.lean
      Arc.lean
      Spline.lean
    Language/
      Common.lean
      L0.lean
      L1.lean
      L2.lean
      L3.lean
      Trace.lean
      WellFormed.lean
    Semantics/
      ExpandFeatures.lean
      Resolve.lean
      Capabilities.lean
      LowerTarget.lean
      LiftTarget.lean
    Passes/
      MergeCollinear.lean
      ArcFit.lean
      AdaptiveSpeed.lean
      TravelReorder.lean
    Codec/
      LogicalRoundTrip.lean
      Chunking.lean
    Verify/
      Predicates.lean
      Coverage.lean
    Assurance/
      Composition.lean
      ClaimRegistry.lean
  Tests/
proofs/
  claims.toml
  assumptions.md
  coverage.json
  fixtures/
tools/
  export_proof_fixtures.*
  check_proof_coverage.*
```

`proofs/claims.toml` is the machine-readable claim registry. Each record contains:

- stable claim id and theorem name;
- language/spec version;
- source and target dialect;
- exact relation from §2;
- assumptions and exclusions;
- Lean source location;
- Rust implementation location;
- refinement fixture/property-test location;
- abstract status: `specified`, `proved` or `not-applicable`;
- numeric status: `pending`, `bounded`, `empirical` or `not-applicable`;
- refinement status: `pending`, `checked` or `not-applicable`.

An implementation-scoped claim requires abstract status `proved`, numeric status `bounded` or
`not-applicable`, and refinement status `checked`. An abstract proof with pending numeric/refinement
status supports only the corresponding abstract theorem.

## 5. Semantic model decisions

### 5.1 Values and quantities

- A dimension is an exponent vector over a frozen base-dimension vocabulary.
- A quantity pairs a numeric value with a dimension; unit labels are scale factors into canonical
  units, not dimensions themselves.
- Unit normalization must preserve dimension and physical value.
- Addition/subtraction require equal dimensions; multiplication/division add/subtract exponent vectors.
- Public defaults are part of the syntax/version contract and are never inferred inside a theorem.
- Invalid, non-finite or out-of-domain values are represented as rejected inputs, not arbitrary reals.

Initial quantity theorems:

- dimension composition forms a commutative group;
- normalization is idempotent;
- conversion through canonical units is coherent;
- valid unit aliases normalize identically;
- incompatible dimensions are rejected;
- the current deposition equation is dimensionally valid:
  `Length × Length × Length × Ratio = Volume`;
- `Volume ÷ Area = Length` for positive filament area.

### 5.2 Geometry and coordinate frames

- Points and vectors are distinct types.
- A pose is an element of `SE(3)` in the full model; current planar feature poses embed into `SE(3)`.
- Rotations use a normalized quaternion in the future public model. The planar v0 feature model may use
  a proved equivalent rotation matrix about Z.
- A frame graph is a finite directed structure with a designated root and unique resolvable path for
  each referenced frame.
- Transform application is total only for validated transforms and resolved frames.

Initial geometry theorems:

- identity, associativity and inverse laws for valid rigid transforms;
- transform action law: `(a ∘ b) • p = a • (b • p)`;
- distances and angles are invariant under rigid transforms;
- planar pose composition agrees with the current `Transform::compose`;
- repeat instance zero is identity and instance `n` is the `n`th power of the step transform;
- points receive translation; vectors/orientations do not;
- arc center and endpoints transform coherently and clockwise sense is preserved by proper rotations;
- frame resolution is deterministic;
- acyclic, uniquely rooted frame graphs resolve; cycles, missing frames and ambiguous roots reject.

### 5.3 Traces and observations

A trace is not merely a point list. Each event records:

- start/end toolframe;
- path primitive or its denotation;
- process/channel valuation;
- tool and target-relevant state;
- provenance;
- modeled duration/material observations where defined;
- opaque or lost behavior markers.

Trace equivalence is parameterized by the observations a consumer is allowed to see. This prevents an
optimization from being called semantics-preserving when it changes a relevant property such as
duration, ordering, temperature state or deposited geometry.

### 5.4 Diagnostics and partiality

Compiler stages are modeled as total functions returning either a value or structured diagnostics.
Panic freedom is an implementation obligation; semantic invalidity is a normal result.

For every failing precondition, the proof target is:

1. the invalid input cannot produce a successful value;
2. the diagnostic identifies the relevant node/segment where the public contract promises a location;
3. earlier successful stages do not erase the source location;
4. opaque/manual operations block claims about the behavior they may affect.

## 6. Proof inventory by compiler boundary

### 6.1 L0 feature expansion

Current target: `features::expand_features`.

Prove:

- deterministic expansion;
- ordered `Group` is list concatenation in source order;
- `Repeat(count, step, body)` expands to instances `0 .. count-1`;
- nested pose composition obeys the transform action law;
- expansion preserves process/channel ordering;
- emitted op count is bounded by the accepted expansion budget;
- depth and node budgets reject before unbounded recursive expansion;
- transformed manual G-code rejects;
- successful transformed geometry has fully defined local positions.

Do not claim yet:

- full `SE(3)` frame correctness;
- general L0 manufacturing-intent correctness;
- a bound on transcendental rounding without FM1.4.

### 6.2 L1 → L2 resolution

Current target: `resolve_checked` / `resolve_unchecked`.

Model resolution as a fold over explicit state. Prove for validated inputs:

- determinism and totality;
- segment order follows op order;
- each emitted segment starts at the prior running position;
- modal compatibility state is propagated exactly as specified;
- travel segments deposit zero volume;
- depositing line/arc/spline segments use the declared volume relation;
- filament is volume divided by positive filament cross-sectional area;
- dwell/retract/unretract/deposit do not change tool position;
- line endpoints inherit omitted axes from prior state;
- arc endpoint, sweep and helical length formulas meet their stated geometric preconditions;
- spline endpoints agree with the through-point sequence;
- output satisfies the selected L2 structural well-formedness predicate.

The current spline length is sampled. Its equality target is equality to the specified sampling
algorithm, not equality to the analytical Catmull–Rom arc length. An analytical error theorem requires
additional regularity assumptions and belongs to FM1.4.

### 6.3 Simulation and verification

For `simulate`, prove fold laws and non-negativity under well-formed inputs, then show that totals equal
the sum/max definitions in the public metric specification.

For `verify`, define every rule as a predicate independent of message formatting. For each rule prove:

- **soundness:** absence of that finding implies the predicate holds, when all required inputs/models
  were present and the rule reports complete coverage;
- **completeness where feasible:** violation of the modeled predicate produces the finding;
- **location correctness:** the reported segment participates in the violation;
- **stream equivalence:** streaming and materialized verification produce the same findings after the
  documented ordering normalization.

A clean report proves only the conjunction of executed rule predicates. Missing contracts, skipped
models and opaque operations must appear in coverage and prevent stronger claims.

### 6.4 Optimization passes

Give every pass its own observation set and relation.

| Pass | Intended contract |
|---|---|
| `merge_collinear` | exact deposited trace and process-state preservation; endpoints, volume and filament conserved |
| `arc_fit` | `≈ε` geometric trace; endpoint/process state exact; declared radial/deviation bound |
| `adaptive_speed` | geometry/material exact; speeds changed only within declared kinematic/process envelope |
| `coasting` | intentional material redistribution under a named process relation; never “exact semantics” |
| `z_hop` | deposited trace exact; travel trace changes according to clearance policy |
| `travel_reorder` | deposited-run multiset and each run's internal order exact; global order/travel trace intentionally changed |

Pipeline composition is legal only when the output predicate of one pass implies the precondition of
the next. Gated optimization must prove that rejection returns the original input and acceptance
satisfies the gate predicates.

### 6.5 Codecs and versioning

For the logical JSON, `DRY0` and `DRY1` models prove:

- `decode(encode(x)) = x` for representable well-formed values;
- field presence/omission maps to the specified logical option/default;
- `DRY0` columns and `DRY1` rows decode to the same logical segment stream;
- chunk partition choice does not change the decoded toolpath;
- streaming concatenation is associative and agrees with materialized decode;
- supported legacy encoding versions map to the current logical model as specified;
- unknown kinds/versions/flags and malformed lengths reject;
- declared resource limits are checked before modeled allocation/decompression;
- frozen v0 semantics are unaffected by future schema additions.

The implementation claim for v0 numeric quantities is `≡bits`, including signed zero where the current
spec requires exact `f64` bit preservation. NaN and infinity remain outside the valid domain.

### 6.6 Capabilities and L2 → L3 → lift

After D1.6–D1.9 freeze these contracts, prove:

- requirement extraction is deterministic and monotone when program requirements are added;
- a successful match covers every mandatory requirement;
- missing mandatory capabilities reject before emission;
- lowering is deterministic for a pinned profile/backend/options tuple;
- emitted modeled actions refine the L2 trace under `⊑C`;
- source maps cover every modeled target action;
- lift recovers the declared recoverable observations;
- declared losses and opaque commands prevent an exact round-trip claim;
- `lift(lower(x)) ≡obs(O) x` for the target's published recoverable observation set `O`.

Controller firmware and physical actuation are assumptions outside the compiler theorem. Layer D
hardware qualification may validate those assumptions for a pinned setup but cannot turn them into
universal theorems.

## 7. Work breakdown

### FM1.1 — assurance constitution and tooling spike

Dependencies: published L2 v0 spec (met).

Work packets:

1. **FM1.1a — claim registry schema.** Define stable ids, statuses, assumptions and traceability fields.
2. **FM1.1b — relation ADR.** Freeze the relations in §2 and rules for public wording.
3. **FM1.1c — Lean spike.** Prove planar transform composition and a small list-fold determinism theorem.
4. **FM1.1d — toolchain pin.** Commit reproducible Lean/Mathlib versions and local/CI commands.
5. **FM1.1e — independence check.** Have a contributor reproduce the build from a clean checkout.

Exit gate: two representative proofs build in CI; the claim registry rejects missing assumptions,
relations or implementation links.

### FM1.2 — quantities, syntax and well-formedness kernel

Dependencies: FM1.1; D1.2 for future public quantity vocabulary.

Work packets:

1. dimension algebra and canonical-unit normalization;
2. common finite syntax/container definitions;
3. L1/L2 v0 syntax and well-formedness;
4. diagnostics/result semantics;
5. serializer-neutral logical equality;
6. valid/invalid fixture exporter.

Exit gate: the quantity theorems in §5.1 and decidable well-formedness equivalence are checked.

### FM1.3 — transforms, feature expansion and frames

Dependencies: FM1.2; current P2.3 for planar expansion; D1.3 for full named frames.

Work packets:

1. vector/point distinction and planar transform action;
2. current `Feature@pose`/`Group`/`Repeat` semantics;
3. expansion budget theorem;
4. `SE(3)` pose and quaternion conventions;
5. frame-graph validation/resolution;
6. Rust differential fixtures and mutation checks.

Exit gate: every theorem listed in §5.2 and §6.1 is checked for its landed scope; current Rust expansion
passes generated refinement cases.

### FM1.4 — numeric and curve error budgets

Dependencies: FM1.2–FM1.3.

Work packets:

1. inventory every `f64` and transcendental boundary in the public semantic core;
2. define versioned tolerance/error-budget profiles;
3. prove or import justified operation-level bounds;
4. compose bounds for pose application, line/arc length and orientation normalization;
5. specify Catmull–Rom sampling and prove endpoint/positivity properties;
6. derive a conditional spline-length/deviation bound or explicitly leave it empirical;
7. test native/wasm results against the checked intervals.

Exit gate: no public geometry theorem crosses from reals to Rust without a checked bound or an explicit
`empirical` status.

### FM1.5 — resolver, simulation and verifier

Dependencies: FM1.2–FM1.4.

Work packets:

1. fold semantics for state/channel propagation;
2. line, arc, spline and zero-motion cases;
3. volume/filament dimensional and value laws;
4. simulation metric folds;
5. independent verifier predicates;
6. soundness/completeness proofs per rule;
7. streaming/materialized equivalence;
8. Rust refinement corpus.

Exit gate: the supported resolver subset produces well-formed L2; every claimed verifier rule has a
coverage-aware theorem and refinement test.

### FM1.6 — optimization pass contracts

Dependencies: FM1.3–FM1.5.

Work packets:

1. freeze the observation/relation matrix from §6.4;
2. prove exact passes first (`merge_collinear`);
3. prove bounded passes (`arc_fit`, then numeric speed constraints);
4. formalize intentional-change relations for coasting, z-hop and travel reorder;
5. prove pipeline precondition chaining and gate behavior;
6. add counterexample fixtures for claims that are intentionally too strong.

Exit gate: no pass is labeled semantics-preserving without a named, checked observation relation.

### FM1.7 — codec and compatibility proofs

Dependencies: FM1.2; can run in parallel with FM1.3–FM1.6.

Work packets:

1. logical JSON/DRY0/DRY1 model;
2. row/column correspondence;
3. encode/decode inverse theorems;
4. chunking/stream equivalence;
5. legacy-version mapping;
6. invalid-input rejection and resource-bound model;
7. refinement against all frozen public vectors and generated structural cases.

Exit gate: the logical inverse and format-equivalence theorems are checked; Rust and the independent
reader pass every generated proof fixture.

### FM1.8 — capabilities, targets and lift

Dependencies: D1.6, D1.8, D1.9; FM1.2–FM1.5.

Work packets:

1. capability set/order and requirement extraction;
2. fail-closed match theorem;
3. generic backend refinement interface;
4. FFF backend proof for a bounded command subset;
5. selected non-FFF backend proof for its reference subset;
6. lift/recovery observation theorem;
7. loss/opaque-command barriers;
8. end-to-end composition theorem.

Exit gate: FFF and the selected D1 non-FFF workflow satisfy the conditional compiler theorem for their
published subsets.

### FM1.9 — publication, maintenance and release gates

Dependencies: FM1.1–FM1.8 as claims mature.

Work packets:

1. publish proof sources, generated documentation and claim coverage;
2. link normative spec clauses to claim ids;
3. add theorem/implementation drift checks;
4. require proof impact classification for semantic changes;
5. publish unsupported syntax and open assumptions;
6. reproduce the proof build in release CI;
7. commission an external review of the top-level theorem and numeric boundary.

Exit gate: an independent reviewer can map every supported public assurance claim to a checked theorem,
numeric status, Rust refinement evidence and explicit assumptions.

## 8. Dependencies and delivery lanes

```text
                    ┌────────► FM1.3 transforms/features/frames ──┐
FM1.1 ──► FM1.2 ─────┼────────► FM1.5 resolve/sim/verify ──────────┼──► FM1.6 passes ──┐
                    ├────────► FM1.7 codecs ──────────────────────┤                 │
                    └────────► FM1.4 numeric bounds ◄─────────────┘                 │
                                                                                   ├──► FM1.9
D1.6 + D1.8 + D1.9 ───────────────────────────────► FM1.8 targets/lift ◄────────────┘
```

After FM1.2, geometry/lowering and codec work can proceed independently. Numeric bounds join every claim
implemented with binary64. Target/lift proofs remain gated by D1 contract freeze; beginning them against
prototype backends would create proof churn and accidental compatibility promises.

## 9. Provisional effort and staffing

These are engineering-effort ranges, not elapsed-time commitments. FM1.1 and FM1.2 replace them with
measured estimates.

| Milestone | Provisional effort | Main specialization |
|---|---:|---|
| FM1.1 constitution/tooling | 1–2 engineer-weeks | formal methods + build/CI |
| FM1.2 kernel/quantities | 3–5 engineer-weeks | type systems + Lean |
| FM1.3 transforms/features/frames | 4–7 engineer-weeks | geometry + Lean/Rust |
| FM1.4 numeric error budgets | 5–9 engineer-weeks | numerical analysis |
| FM1.5 resolve/simulate/verify | 6–10 engineer-weeks | semantics + process math |
| FM1.6 optimization passes | 5–10 engineer-weeks | computational geometry |
| FM1.7 codecs/versioning | 4–7 engineer-weeks | formal data formats |
| FM1.8 targets/lift | 6–12 engineer-weeks | compiler correctness + controllers |
| FM1.9 publication/review | 2–4 engineer-weeks | assurance/release engineering |
| **Total** | **36–66 engineer-weeks** | before contingency |

The minimum sustainable team is one formal-methods owner, one Rust/compiler owner and fractional
numerical-analysis review. Backend domain owners must review the assumptions for FM1.8. No theorem author
may self-approve a new hardware-safety claim.

## 10. CI and change control

### Per pull request

- build all affected Lean modules with the pinned toolchain;
- check `proofs/claims.toml` schema and theorem/source links;
- regenerate and diff affected proof fixtures;
- run affected Rust refinement/property tests;
- reject a semantic code/spec change with no proof-impact declaration;
- keep ordinary Rust/Python/TypeScript/wasm conformance gates green.

### Nightly

- full proof build from a clean cache;
- expanded property and differential cases;
- native/wasm interval checks;
- mutation tests for the model/Rust bridge;
- compatibility checks across supported spec and proof-model versions.

### Release

- publish claim coverage and assumption manifests;
- verify proof artifact/toolchain hashes;
- rebuild proof documentation from source;
- require explicit release notes for weakened, retired or newly empirical claims;
- block “formally verified” product wording unless the exact supported subset and layers are named.

## 11. Definition of done for a proof-bearing work packet

A packet is done only when:

1. the public semantic statement is stable and versioned;
2. the theorem uses a relation from §2;
3. assumptions, exclusions and numeric domain are explicit;
4. the Lean proof contains no unreviewed axioms or placeholders;
5. any floating-point implementation claim has a Layer B result;
6. the Rust implementation has Layer C refinement evidence;
7. negative/counterexample fixtures exercise the theorem boundary;
8. claim registry, specs and limitations are updated;
9. a reviewer can rerun the proof and refinement command from a clean checkout.

## 12. First implementation queue

This queue may begin before D1 activation because it targets already-published v0 and P2.3 behavior:

1. FM1.1a claim-registry schema.
2. FM1.1b assurance-relation ADR.
3. FM1.1c Lean spike for planar transform composition and deterministic list-fold semantics.
4. FM1.1d pinned proof toolchain and CI command.
5. FM1.2 quantity dimension algebra.
6. FM1.2 L2 v0 logical syntax and well-formedness.
7. FM1.3 current planar feature expansion semantics.
8. FM1.7 logical JSON/DRY0/DRY1 round-trip model.

Do not begin full-frame, capability, generic backend or lift proofs until the corresponding D1 public
contracts are frozen.

## 13. Stop/go gates

Pause and revise the proof or language design if:

- a theorem needs to assume the property it is supposed to establish;
- a real-number theorem is being presented as an `f64` theorem without a bound;
- the proof model omits observable state changed by the implementation;
- a pass cannot state which observations it preserves;
- model-generated fixtures and Rust agree only because they share implementation code;
- proof maintenance repeatedly depends on private, unstable Rust structure instead of public semantics;
- opaque target behavior is treated as recovered or verified;
- a “clean report” theorem cannot account for skipped checks and missing models;
- the selected target cannot state a bounded recoverable observation set;
- proof complexity reveals an unnecessarily modal or ambiguous public language contract.

The preferred response is to narrow the supported subset or simplify the language semantics, not to add
an undocumented assumption.
