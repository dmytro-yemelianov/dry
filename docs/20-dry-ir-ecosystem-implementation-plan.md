# Dry IR language and ecosystem — implementation plan

This document turns the deferred D1 initiative into an implementation-ready program. It is a plan, not
an activation decision: the current P0–P6 queue remains authoritative until the D1 entry gate in
[`02-roadmap.md`](02-roadmap.md) is met.

The implementation must evolve the existing compiler instead of replacing it. In particular, resolved
L2 `Toolpath` v0, its JSON/`DRY0`/`DRY1` codecs, the current CLI workflows and the Python/TypeScript/wasm
surfaces remain supported while the new language contracts are built and independently conformance
tested.

## 1. Outcome

D1 is complete when Dry has a public, versioned language stack and the ecosystem needed to implement and
operate it:

```
L0 intent ──lower──► L1 explicit path ──resolve──► L2 absolute motion
                                                        │
                          verify against capabilities ◄─┤
                                                        │
                    target artifact ◄── L3 backend ◄────┘
                           │
                           └──parse/lift──► recoverable L2 + losses + opaque regions
```

The first production proof consists of two vertical workflows:

1. FFF authoring through at least Marlin and Klipper controller dialects.
2. Exactly one non-FFF workflow selected from the accepted P5.3/P5.4 prototype:
   CNC profile/pocket/drill through a pinned RS-274 controller, or laser profile/raster through a pinned
   GRBL controller.

Robot support is not bundled into the first D1 proof unless it independently meets the same activation
and reference-machine gates. Supporting more processes is follow-on work, not a reason to make the first
language version universal.

## 2. Activation packet

D1.1 starts only after P2.3, P4.3 and one of P5.3/P5.4 meet their acceptance criteria. The activation
record is a committed ADR containing:

- the selected non-FFF workflow and why its P5 evidence is sufficient;
- one FFF and one non-FFF reference program;
- the exact reference machines, tools, controllers and controller versions;
- the P5 artifacts and limitations carried into D1;
- the process invariants that must survive lowering, emission and lifting;
- the hardware owner, safe operating envelope and evidence-retention location;
- the planned public schema identifiers and compatibility window;
- work-packet estimates based on the chosen target.

If that packet cannot name a bounded non-FFF workflow, D1 remains deferred. “Generic CNC”, “all robots”
or “support arbitrary G-code” are not valid activation scopes.

## 3. Engineering constraints

These constraints apply to every D1 work packet.

### 3.1 Compatibility before cleanup

- Existing L2 v0 vectors must continue to decode and produce their current semantic results.
- Existing `resolve`, `simulate`, `verify`, `optimize`, `emit` and G-code review workflows stay green.
- A new representation is introduced under a new dialect/schema version; an existing version is never
  silently reinterpreted.
- Migrations return diagnostics and a loss report. They do not drop unknown fields, frames, channels or
  target commands without saying so.
- Current fixed FFF fields become compatibility projections of the new model before they are deprecated.

### 3.2 Explicit semantics

- Public L1 has no hidden modal dependency. Every operation resolves from document state that is present
  in that document.
- Every coordinate is associated with a named frame; transforms are explicit and validated.
- Every public numeric field has a declared dimension. SDK convenience numbers are accepted only where a
  documented default unit is unambiguous.
- Unknown capabilities and unsupported operations fail closed before emission.
- Opaque target commands are preserved and quarantined; a verifier never claims to have checked their
  effects.

### 3.3 Determinism and bounded claims

- The same input documents, profiles, registry and compiler version produce the same semantic result.
- Numeric tolerances are named, versioned and included in conformance metadata.
- A clean verification report names the checks performed, the models used and any uncovered regions.
- Hardware qualification proves only the pinned workflow and configuration. It is not product
  certification and does not generalize to an untested machine.

### 3.4 Proof-aware semantics

The FM1 workstream in
[`21-mathematical-assurance-plan.md`](21-mathematical-assurance-plan.md) formalizes stable language
contracts. Every D1 work packet that adds or changes public semantics must classify its proof impact and
state whether its contract expects exact equality, trace equivalence, tolerance-bounded approximation,
capability refinement or deliberate loss. D1 does not wait for all FM1 proofs to land, but it must not
publish stronger assurance wording than the current proof/numeric/refinement status supports.

## 4. Current seams and planned evolution

The implementation should use the codebase's existing seams:

| Current seam | Planned responsibility | Migration rule |
|---|---|---|
| `resolve::Design` / `resolve::Op` | Legacy authoring adapter into versioned L1 | Keep behavior; translate legacy inherited state into explicit L1 and report assumptions |
| `ir::Toolpath` / `ir::Segment` | L2 v0 compatibility model | Freeze v0; add a versioned L2 logical model and lossless v0 adapters |
| `units.rs` wrappers | Canonical engine quantities | Add typed public inputs and schema dimensions without weakening internal types |
| `profile::Profile` v1 | Legacy FFF profile | Migrate to capability-oriented profile v2; retain deterministic v1 → v2 conversion |
| `verify::Contracts` / `Finding` | Existing FFF rules and diagnostic shape | Reuse stable rule ids; extend locations, coverage and model metadata additively |
| `emit::gcode::EmitParams` | FFF backend options | Move behind a backend configuration adapter; do not expose firmware quirks in generic IR |
| `gcode::lift::ImportedGcode` | Existing FFF lift and source map | Generalize to `LiftResult` while preserving unmodelled commands and source-line maps |
| `codec/` | L2 v0 binary compatibility | Do not mutate `DRY0`/`DRY1`; add new encodings only after logical schemas stabilize |
| Python / TypeScript / wasm adapters | Thin public language surfaces | Generate or mechanically validate shared schema shapes; keep lowering in Rust |
| `conformance/` | Existing oracle and v0 fixtures | Add public D1 suites that do not depend on FullControl or private Rust behavior |

`resolve_checked` and the g-code emitter are already high-fan-in boundaries used by Python, wasm and
downstream workflows. They should be wrapped, not signature-broken. G-code lifting is also consumed by
review, rewrite, compare, forensics and LLM-report flows, so its generalized result must retain a
compatibility view for those consumers.

## 5. Planned module and artifact layout

The first implementation stays inside `dry-core`; splitting crates before the contracts stabilize would
make migrations harder. Planned API names may be refined by D1.1, but their responsibilities should not
move.

```
crates/core/src/
  language/
    document.rs       schema id/version, provenance, source ids
    diagnostic.rs     spans, severity, coverage and loss records
    quantity.rs       dimension and unit normalization at public boundaries
    frame.rs          frame ids, rigid transforms, frame graph
    channel.rs        channel definitions, values and compatibility policy
    capability.rs     requirements, profiles and matching reports
    intent.rs         versioned L0 nodes
    path.rs           explicit, frame-aware L1 nodes
    motion.rs         versioned L2 logical model + Toolpath v0 adapters
  lower/
    intent_to_path.rs
    path_to_motion.rs
  target/
    backend.rs        backend contract and registry
    artifact.rs       emitted bytes/text, manifest and source map
    fff.rs            adapter over current Marlin/Klipper/Duet emission
    selected.rs       selected non-FFF backend
  lift/
    result.rs         recovered semantics, losses, diagnostics and opaque nodes
    fff.rs
    selected.rs
```

Existing `verify.rs`, `profile/`, `emit/`, `gcode/` and `codec/` modules remain in place while their
public behavior is adapted. They can be reorganized only after compatibility tests prove the move is
mechanical.

New public artifacts are versioned independently:

- `dry.intent/1` — L0 document;
- `dry.path/1` — L1 document;
- `dry.motion/1` — future L2 logical document; L2 v0 remains a supported legacy contract;
- `dry.profile/2` — capability-oriented machine/process/controller profile;
- `dry.target-manifest/1` — target identity, backend version, profile hashes and source map;
- `dry.lift-report/1` — recovered boundary, losses, tolerances and opaque regions;
- `dry.verify-report/2` — findings plus coverage/model metadata.

One integer must not serve as all of these version axes. Each document carries a schema id and schema
version, and release SemVer remains separate.

### 5.1 Compiler API shape

Profiles, registries, tolerances and target choices are explicit inputs; no pass reads ambient machine
state. The Rust surface is organized around these operations:

```text
validate(document, registry) -> ValidationReport
lower_intent(intent, context) -> PassOutcome<PathProgram>
resolve_path(path, context) -> PassOutcome<MotionProgram>
verify_motion(motion, profile, models) -> VerificationReport
emit_target(motion, profile, backend) -> TargetArtifact
lift_target(artifact, profile, backend) -> LiftResult
```

`PassOutcome<T>` carries the result, diagnostics and provenance map. A failure does not return a partial
program unless the API explicitly marks it non-executable.

The corresponding CLI grows additively:

- `dry validate <document>` validates any recognized public dialect;
- `dry lower --to path|motion` exposes deterministic intermediate forms;
- `dry compile --target <id> --profile <file>` runs validate → lower → verify → emit;
- `dry lift --target <id>` returns recovered semantics plus a lift report.

Current `emit`, `import-gcode`, `review-gcode` and SDK convenience methods remain compatibility
front-ends over these operations.

## 6. Core model decisions

These are the default design decisions. D1.1 may overturn one only with an ADR and replacement
conformance case.

### 6.1 Quantities

- Rust continues to use dimensional newtypes and canonical units internally.
- Public L0/L1/profile inputs accept a unit-tagged quantity form and normalize at the boundary.
- Schema fields identify their required dimension; dimensionless ratios are explicit.
- L2 v0 keeps its existing canonical bare-number encoding.
- Binary encodings remain numeric and compact; their schema fixes the dimension and canonical unit.
- Python and TypeScript expose constructors such as `mm`, `inch`, `mm_per_min`, `rpm` or equivalent
  typed values, while documented legacy numeric overloads remain compatibility helpers.

### 6.2 Frames and poses

- Frame ids are stable document-local strings with reserved semantic names including `design`,
  `workpiece`, `fixture`, `tool` and `machine`; each document declares exactly one root.
- The frame graph is a directed acyclic graph of rigid transforms.
- Coordinates are right-handed. A stored transform maps child-frame coordinates into its parent frame;
  composition proceeds toward the root.
- Quaternions serialize as `[x, y, z, w]`, represent active rotation and are normalized with a canonical
  sign so equivalent rotations have one deterministic wire form.
- A pose carries position plus a normalized quaternion. The current tool-direction vector is a lossy
  compatibility projection because it cannot represent tool roll.
- Lowering resolves every L1 pose into an explicitly selected L2/reference frame.
- Undefined parents, cycles, non-finite matrices, non-rigid transforms and ambiguous roots are errors.
- Downgrading a full pose to L2 v0 fails if required orientation information cannot be represented.

### 6.3 Channels

- Channel definitions use stable namespaced ids. `dry.*` is reserved; external publishers use a
  reverse-domain namespace.
- A definition declares value type, dimension, default, interpolation/propagation behavior, valid range
  and target compatibility behavior.
- Existing `temperature`, `fan`, `flow`, `tool`, `power` (spindle/laser `S`) and orientation fields map
  to reserved built-in channels.
- Unknown channels round-trip. A backend must either consume, explicitly lower, preserve as opaque
  metadata, or reject each one.
- The first codec representation favors correctness and deterministic ordering; columnar specialization
  is a later optimization backed by benchmarks.

### 6.4 Capabilities and requirements

- Programs declare requirements; profiles declare capabilities.
- Requirements distinguish `required`, `preferred` and `optional`.
- Matching produces a located `CompatibilityReport`, not a boolean.
- A profile separates machine geometry/kinematics, controller dialect/features, installed tools,
  process envelopes and site policy.
- Backend selection is the intersection of program requirements, profile capabilities and backend
  features. Missing required capability is an error before emission.

### 6.5 Diagnostics, provenance and losses

- Every L0/L1 node has a stable source id.
- Each lowering pass emits an input → output provenance map.
- Diagnostics can locate a document/node, L2 segment, source target line or profile field.
- A `LossRecord` names the source construct, destination boundary, reason, severity and recoverability.
- Verification reports include a `Coverage` section listing executed, skipped and inapplicable models.

## 7. Work breakdown

The D1 items in [`04-tasks.md`](04-tasks.md) are epics. Their `M`/`L` labels are not scheduling estimates
until decomposed against the selected target. The work packets below are the units to estimate, assign
and merge.

### Milestone A — freeze contracts and compatibility (`D1.1`)

Goal: make architectural choices before new schemas enter code.

Work packets:

1. **D1.1a — activation evidence.** Commit the activation packet and reference inputs/outputs.
2. **D1.1b — dialect ADR.** Specify document envelopes, version axes, ownership of L0–L3 semantics and
   the public diagnostic model.
3. **D1.1c — compatibility matrix.** Enumerate every current Rust, CLI, Python, TypeScript, wasm, JSON,
   `DRY0`, `DRY1`, profile and report surface with keep/adapt/deprecate decisions.
4. **D1.1d — frozen baseline.** Add fixtures for legacy authoring → L2, profile → contracts,
   L2 → FFF and G-code → imported L2 before refactoring.
5. **D1.1e — module scaffolding.** Add schema/version types and empty language modules without changing
   behavior.

Exit evidence:

- the ADR has no unresolved decision that changes a public schema;
- all current tests and frozen baseline fixtures pass;
- every later work packet has an owner, dependency and estimated effort;
- selected target and hardware facts are pinned by content hash.

### Milestone B — quantity and diagnostic foundation (`D1.2`)

Goal: make invalid dimensions unrepresentable inside the compiler and reject them at every public
boundary.

Work packets:

1. **D1.2a — quantity vocabulary.** Inventory dimensions needed by both reference workflows, including
   length, angle, time, linear/rotary feed, speed, acceleration, temperature, power, spindle speed,
   volume, area and ratios.
2. **D1.2b — normalization API.** Implement unit parsing/conversion and structured errors without
   changing current internal unit types.
3. **D1.2c — schema integration.** Add dimensional schemas and independent validation fixtures.
4. **D1.2d — SDK parity.** Expose equivalent Python and TypeScript quantity constructors and serialize
   identical documents.
5. **D1.2e — compatibility adapters.** Convert legacy numeric `Op`, `ResolveParams` and profile v1 fields
   with explicit canonical-unit assumptions.
6. **D1.2f — negative corpus.** Add cross-SDK fixtures for valid mixed units, wrong dimensions,
   overflow/non-finite values and unsupported units.

Exit evidence:

- equivalent unit inputs produce semantically equal canonical documents;
- dimension mismatches fail before lowering with the same stable diagnostic id across SDKs;
- L2 v0 JSON and binary goldens are unchanged.

### Milestone C — explicit frames and non-modal L1 (`D1.3`)

Goal: replace inherited coordinate assumptions with an explicit path document.

Work packets:

1. **D1.3a — frame primitives.** Implement `FrameId`, rigid transform, quaternion/pose and validated
   `FrameGraph`.
2. **D1.3b — L1 schema.** Define explicit path nodes for line, arc, spline, dwell, tool/process state and
   opaque/manual operations. Every motion node carries a frame and complete endpoint state.
3. **D1.3c — legacy expansion.** Translate current modal `resolve::Op` sequences into explicit L1 while
   recording inherited-value assumptions.
4. **D1.3d — path resolver.** Resolve a validated L1 document into L2 with provenance.
5. **D1.3e — pose downgrade.** Define and test when full poses can project to the current L2 v0
   orientation vector.
6. **D1.3f — frame corpus.** Cover work offsets, nested fixtures, tool offsets, cycles, ambiguous roots
   and two placements of the same path.

Exit evidence:

- explicit L1 and legacy authoring produce equal L2 for the frozen baseline;
- the same path under two workpiece transforms resolves deterministically;
- no L1 motion result depends on an omitted prior command;
- unrepresentable pose loss fails with a located diagnostic.

### Milestone D — language, channels and capabilities (`D1.4`, `D1.5`, `D1.6`)

Goal: build the smallest useful public language surface for the two reference workflows.

The three streams may proceed in parallel after Milestones B/C, but their schemas stabilize together.

#### D1.4 — intent vertical slices

1. **D1.4a — common intent shell.** Define shared `ProcessPlan`, `Setup`, tool selection and
   region/contour references.
2. **D1.4b — FFF intent.** Implement the bounded FFF reference intent, including deposition strategy and
   declared bead/process invariants.
3. **D1.4c — selected-process intent.** Implement only the selected non-FFF intents:
   CNC profile/pocket/drill, or laser profile/raster/mark.
4. **D1.4d — intent lowering.** Lower L0 → explicit L1 with provenance and preserved declared
   invariants.
5. **D1.4e — intent corpus.** Add hand-audited golden L1 for each reference intent.

#### D1.5 — channel registry

1. **D1.5a — registry model.** Implement built-in channel definitions and the registry validator.
2. **D1.5b — legacy bridge.** Bridge fixed L2 v0 fields to reserved channel ids.
3. **D1.5c — channel semantics.** Define deterministic propagation/interpolation and unknown-channel
   behavior.
4. **D1.5d — SDK extension proof.** Add an extension channel in Rust, Python and TypeScript without
   changing the generic segment type.
5. **D1.5e — channel corpus.** Add logical and codec round-trip fixtures, including registry-version
   mismatch.

#### D1.6 — capability profiles

1. **D1.6a — profile v2.** Publish its schema and deterministic profile v1 → v2 migration.
2. **D1.6b — capability domains.** Model machine envelope/kinematics, controller features, tools and
   process limits separately.
3. **D1.6c — requirement extraction.** Derive program requirements from L0/L1/L2.
4. **D1.6d — matcher.** Produce located missing/degraded/unused capability results.
5. **D1.6e — reference profiles.** Pin profiles and their source/provenance records.

Exit evidence:

- both reference intent programs lower to hand-auditable L1;
- an external channel round-trips and is deliberately handled or rejected by each reference backend;
- unsupported required capabilities stop before target emission;
- profile v1 workflows retain their previous resolve/verify/emit behavior.

### Milestone E — target-aware verification (`D1.7`)

Goal: make the assurance boundary explicit and executable.

Work packets:

1. **D1.7a — verifier staging.** Split checks into structural, dialect-semantic, capability, kinematic,
   geometric/collision and process stages while preserving current rule ids.
2. **D1.7b — coverage report.** Record model/profile versions, executed checks, skipped checks and opaque
   regions.
3. **D1.7c — geometry envelope.** Check tool/fixture/machine envelopes for the bounded reference
   geometries; document geometric approximations.
4. **D1.7d — kinematics.** Check reference-machine reachability, joint/axis limits and singularities
   appropriate to the selected machine model.
5. **D1.7e — process rules.** Add only evidence-backed thermal/power/spindle/process envelopes required
   by the two reference workflows.
6. **D1.7f — adversarial corpus.** Create one positive and multiple negative fixtures for every new rule
   and every skipped-model case.

Exit evidence:

- every new rule has a stable id, located fixtures and documented assurance limits;
- an opaque command prevents a general clean/safe claim;
- native and wasm reports agree semantically;
- old FFF report goldens either remain unchanged or migrate through a reviewed report-version change.

### Milestone F — pluggable target backends (`D1.8`)

Goal: make target emission a capability-aware compiler stage.

The backend contract has four operations:

1. report backend identity and supported capabilities;
2. validate a program/profile pair;
3. lower generic motion into target operations;
4. emit a `TargetArtifact` containing program text/bytes, manifest, diagnostics and source map.

Work packets:

1. **D1.8a — backend trait and registry.** Add deterministic backend discovery by stable target id.
2. **D1.8b — artifact manifest.** Hash the input, compiler, schemas, profile, backend and target options.
3. **D1.8c — FFF adapter.** Put current Marlin/Klipper/Duet emission behind the contract with no output
   drift on legacy fixtures.
4. **D1.8d — selected backend.** Promote the chosen P5 emitter, removing prototype shortcuts and
   hard-coded machine assumptions.
5. **D1.8e — source maps.** Map emitted target statements to L2 segments and originating L0/L1 nodes.
6. **D1.8f — CLI/SDK surface.** Add one target-selection flow shared by native, Python, TypeScript and
   wasm where the backend is available.

Exit evidence:

- one FFF L1 program emits through Marlin and Klipper under declared profiles;
- the non-FFF reference program emits through its pinned backend/profile;
- missing capabilities and unhandled channels fail before bytes are produced;
- legacy FFF goldens remain green.

### Milestone G — parsing and semantic lifting (`D1.9`)

Goal: make target import honest about what can and cannot be recovered.

`LiftResult` contains:

- the highest recovered Dry dialect and document;
- source statement → recovered node/segment mapping;
- numeric tolerance policy;
- `LossRecord` entries;
- preserved opaque statements;
- parser/backend/profile identities and hashes.

Work packets:

1. **D1.9a — general result model.** Adapt `ImportedGcode` without breaking review/rewrite/forensics
   consumers.
2. **D1.9b — FFF recovery contract.** Classify current G/M/T commands as recovered, preserved opaque,
   ignored-by-policy or rejected.
3. **D1.9c — selected-target parser/lifter.** Implement the same classification for the non-FFF
   controller dialect.
4. **D1.9d — semantic comparator.** Compare declared recoverable invariants instead of target text.
5. **D1.9e — adversarial round trips.** Exercise modal changes, units, offsets, arcs, unsupported
   commands, controller extensions and numeric formatting.

Exit evidence:

- emit → lift preserves every declared recoverable L2 invariant within the named tolerance;
- no lifter fabricates L0 intent from target motion;
- losses and opaque statements are source-located;
- current source-preserving G-code rewrite behavior stays green.

### Milestone H — public ecosystem conformance (`D1.10`)

Goal: prove the contracts are implementable without `dry-core`.

Publish:

```
spec/
  dry-intent-v1.schema.json
  dry-path-v1.schema.json
  dry-motion-v1.schema.json
  dry-profile-v2.schema.json
  dry-target-manifest-v1.schema.json
  dry-lift-report-v1.schema.json
  dry-verify-report-v2.schema.json
conformance/language/
  quantities/
  frames/
  intent_to_path/
  path_to_motion/
  capabilities/
  verify/
  emit/
  lift/
  negative/
```

Work packets:

1. **D1.10a — contract freeze.** Freeze schemas, compatibility metadata and tolerance profiles.
2. **D1.10b — public vectors.** Publish positive vectors and expected diagnostics for negative vectors.
3. **D1.10c — runner.** Add a language-conformance runner with machine-readable results.
4. **D1.10d — drift gates.** Check schemas against Rust, Python and TypeScript surfaces.
5. **D1.10e — independent implementation.** Build a clean-room reader/lowerer/round-trip
   implementation that does not link `dry-core`.
6. **D1.10f — ecosystem policy.** Document extension registration, compatibility and deprecation rules.

Exit evidence:

- the independent implementation consumes the public schemas and passes the required vectors;
- current and previous supported schema versions have explicit reader/writer results;
- all public diagnostics used by fixtures are catalogued;
- release CI packages the specs, vectors and runner together.

No dialect is called stable before this milestone passes.

### Milestone I — controlled hardware qualification (`D1.11`)

Goal: execute the reference workflows on pinned hardware with reproducible evidence.

Work packets:

1. **D1.11a — protocol schema.** Define machine identity, calibration prerequisites, environment,
   program, expected observations, measurement tolerances, safety owner, abort criteria and result.
2. **D1.11b — dry run.** Exercise the full protocol against simulation/controller validation without
   energizing the process.
3. **D1.11c — FFF qualification.** Run the pinned FFF artifact and archive machine-readable plus human
   observations.
4. **D1.11d — non-FFF qualification.** Run the selected workflow under its approved safety procedure.
5. **D1.11e — reproducibility.** Repeat each protocol or have a second operator execute it from the
   archived inputs.
6. **D1.11f — release gate.** Verify all input/output/profile/controller/report hashes and publish the
   bounded support statement.

Exit evidence:

- every run has a comparable pass/fail result and complete hashes;
- deviations are linked to issues and invalidate the gate until resolved or explicitly re-baselined;
- the support matrix names exactly the qualified combinations;
- no document generalizes the result beyond those combinations.

## 8. Dependency and parallelism plan

```
D1.1
 ├──► D1.2 ──┬──► D1.4 ───────────────┐
 │           ├──► D1.5 ──┐            │
 │           └──► D1.6 ──┼──► D1.7 ──┼──► D1.10 ──► D1.11
 └──► D1.3 ───────────────┤            │
                          └──► D1.8 ──► D1.9 ───────┘
```

After D1.1, three engineering lanes can proceed:

- **Language lane:** quantities, frames, L0/L1 and lowering.
- **Target lane:** channels, capabilities, verification, backend and lift.
- **Ecosystem lane:** schemas, SDK parity, fixtures, independent runner and documentation.

The ecosystem lane starts with schema/fixture tooling but cannot freeze outputs before the relevant
language or target milestone exits. Hardware work may prepare protocols early, but execution is the final
gate.

## 9. Provisional effort and staffing

These ranges are engineering effort, not elapsed calendar promises. D1.1 replaces them with estimates
based on the selected P5 target. Hardware booking, fabrication, calibration and safety review are not
included.

| Milestone | Provisional effort | Primary skills |
|---|---:|---|
| A — contracts and compatibility | 2–3 engineer-weeks | language design, compatibility, test infrastructure |
| B — quantities and diagnostics | 3–5 engineer-weeks | Rust types, schemas, Python/TypeScript |
| C — frames and explicit L1 | 4–7 engineer-weeks | geometry/math, lowering, property testing |
| D — intent/channels/capabilities | 8–13 engineer-weeks | language design, process domain, schemas |
| E — target-aware verification | 5–9 engineer-weeks | computational geometry, kinematics, process rules |
| F — target backends | 5–9 engineer-weeks | controllers, emission, SDK/CLI integration |
| G — semantic lifting | 4–7 engineer-weeks | parsers, source maps, semantic comparison |
| H — ecosystem conformance | 4–7 engineer-weeks | independent implementation, release/CI |
| I — qualification tooling | 2–4 engineer-weeks | test protocols, evidence tooling; excludes machine time |

The unrefined total is 37–64 engineer-weeks, excluding machine lead time. It is intentionally a range,
not a commitment; the selected non-FFF workflow is the largest source of uncertainty.

Three owners are the practical minimum for parallel execution: language/schema, compiler/targets and
conformance/SDK. A named machine-safety owner must approve Milestone I and should not be the sole author
of the backend being qualified.

The critical path is A → B/C → D → {E in parallel with F → G} → H → I. Adding engineers does not bypass
contract freeze, independent conformance or controlled-hardware gates.

## 10. CI and test strategy

### Per pull request

- all existing Rust, Python, TypeScript, wasm and legacy conformance tests;
- format/lint/schema validation;
- unit and property tests for the changed language component;
- affected positive and negative D1 vectors;
- deterministic serialization and diagnostic snapshots;
- no-diff legacy L2 v0 and FFF emission gates where applicable.

### Nightly

- the complete cross-SDK language corpus;
- fuzzing of document decoders, frame graphs, unit inputs, registry extensions and target parsers;
- all backend/lift semantic round trips;
- large-document memory and performance checks;
- compatibility reads across every supported schema version.

### Release

- public schema/vector drift validation;
- independent implementation conformance;
- packaged artifact installation tests;
- support-matrix and compatibility-matrix checks;
- manually triggered hardware evidence verification when a release claims a qualified workflow.

Hardware execution is never an automatic unstaffed CI job.

## 11. Pull-request and rollout policy

- Deliver one work packet per PR where practical; avoid a long-lived “D1 rewrite” branch.
- Land specs, schemas, code, negative fixtures and migration notes together.
- Keep new language APIs under an explicitly experimental namespace until D1.10.
- Keep legacy APIs as adapters through at least one stable language release.
- Deprecation requires usage guidance, a deterministic migration and a removal version.
- Backend promotion proceeds `prototype → experimental → conformance-gated → qualified`.
- A backend can be conformance-gated without being hardware-qualified; the support matrix must show the
  distinction.
- Feature flags may control optional heavy geometry/kinematics dependencies, but must not change the
  meaning of a serialized document.

## 12. Definition of done for every work packet

A work packet is done only when:

1. its public semantics are documented;
2. schemas and examples match the implementation;
3. errors have stable ids and useful locations;
4. positive, negative and migration tests exist;
5. Rust, Python, TypeScript and wasm impact is either implemented or explicitly marked inapplicable;
6. compatibility with supported old artifacts is demonstrated;
7. provenance, tolerance and loss behavior are explicit;
8. proof impact and intended semantic relation are recorded for FM1;
9. the support/limitations pages are updated if the user-visible boundary changed.

Code completion alone is not acceptance.

## 13. First implementation queue after activation

The first queue is deliberately foundation-only:

1. D1.1a activation evidence and reference artifact hashes.
2. D1.1b dialect/versioning ADR.
3. D1.1c compatibility matrix.
4. D1.1d frozen legacy baseline.
5. D1.1e schema/diagnostic scaffolding.
6. D1.2a quantity vocabulary for the two selected workflows.
7. D1.3a frame math conventions and property-test corpus.

Do not start a new backend, collision engine or universal L0 ontology in this queue. Those depend on the
contracts and evidence above.

## 14. Program-level stop/go gates

Pause and revise the plan if any of these occur:

- the selected P5 target needs process semantics that cannot be represented without redesigning the
  proposed shared L1;
- preserving L2 v0 would require silently changing its wire meaning;
- frame/pose projection loses information required for the reference target;
- a capability cannot be tested or traced to profile evidence;
- verification cannot report an important unmodelled region;
- the independent implementation needs private Rust behavior to pass;
- hardware evidence cannot be reproduced from archived inputs.

The correct response is a scoped ADR or narrower reference workflow, not an undocumented exception.
