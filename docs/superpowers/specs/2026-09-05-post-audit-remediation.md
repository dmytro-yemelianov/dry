# Post-Audit Remediation — findings, versioning and roadmap alignment

Date: 2026-09-05 · Baseline: `main` @ `cf3216e` (tracked tree clean) · Scope: crate/module split, cross-target integration, unit/integration test definition, Lean 4 proof coverage.

Method: four parallel review-only audits (`architect`, `reviewer`, `qa-assurance` ×2) under the AGENTS.md Shared Agent Contract, each forbidden to edit or to run `cargo`/`lake`. Every finding carried below was independently re-verified by the dispatcher against the source before it was written down; findings that did not survive that check are not here. `codebase-memory-mcp` was unavailable, so all discovery fell back to Grep/Glob/Read plus `python3` static analysis — recorded per AGENTS.md §"Codebase Knowledge Graph".

Live gate evidence at `cf3216e`: `cargo test -p dry-core -p dry-cli` → 756 passed / 0 failed / 1 ignored across 100 test binaries; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `tools/validate_vectors.py` (14 vectors), twelve Python assurance gates and `scripts/check-version.sh v0.10.0` (20 manifests) all exit 0. The single ignored test is `crates/core/tests/h13_rule_probe.rs:293`, a deliberate report-only corpus probe.

---

## Versioning and release alignment

**Release state.** `Cargo.toml` and all 20 tracked manifests read `0.10.0`. `CHANGELOG.md` carries `## [0.10.0] - 2026-09-05`, whose own text states *"the compiler, IR, and report schemas are unchanged"* — it is a licensing and registry-publication release. **Tag `v0.10.0` does not exist**; the newest tag is `v0.9.1`. The publish runbook (owner-gated: confirm the release date, tag, wait for the release pipeline, `cargo login`, three publish waves) is still pending.

**Consequence for this work.** Every remediation below changes engine, binding, CI or assurance behaviour. None of it can honestly land under the `[0.10.0]` heading, which declares the compiler unchanged.

**Decision recorded here:**

1. Remediation PRs accumulate under `## [Unreleased]` and **do not touch any version manifest**. Each PR stays independent of the release decision.
2. The version bump plus the dated CHANGELOG heading is one release-prep PR at cut time, not part of any remediation PR.
3. **Recommendation to the owner:** cut `v0.10.0` from the current green `main` first — the `[0.10.0]` entry is accurate for that tree — then release the remediation as **`v0.11.0`**. If `v0.10.0` is never cut, the alternative is to fold this work into `0.10.0` and rewrite its blurb; that is the owner's call, and this plan is written so either choice works without rework.

**Semver classification of the remediation: MINOR (`v0.11.0`).** Two public API additions (`check_compatibility_json` in the PyO3 module, `checkMachineCompatibility` in `sdk/ts/src/engine.ts`) plus a behavioural change in which inputs that previously returned `compatible: true` from the Python and TypeScript SDKs now correctly return `compatible: false`. On a `0.x` line that is a minor bump, not a patch — the change is a bug fix but it is observable and it can fail a consumer's pipeline that was passing on an unsafe program.

## Roadmap alignment

Mapped against `docs/02-roadmap.md` §"Version Horizons & Milestones Mapping" and the open milestones in `docs/04-tasks.md`:

| Finding cluster | Roadmap anchor | Nature |
|---|---|---|
| Capability parity broken in `py`/`sdk/ts`; parity gate cannot detect it | **v0.9.0 / Phase 8** — *"full CLI/SDK capability parity **and a gate that enforces it**"*; **D1.6** — *"target selection fails closed with located diagnostics"* | **Regression against a released exit gate.** Not new work: the horizon claimed this and the claim is false. |
| `crates/cloud` fails open on a malformed contracts header | **D1.6** (fail closed); **v0.7.0** 3-tier verification architecture | Defect against an existing accepted design. |
| `crates/wasm` unit tests unrun; release-only test branch never compiled | AGENTS.md Core Rule 1; `v0.7.x` patch-horizon pattern *"test coverage expansions"* | Gate hygiene. No roadmap deliverable changes. |
| Lean claim ledger flattens four proof strengths into `proved` | **FM1.9** *Publication and maintenance* (remaining: independent external review); **FM1.6** acceptance criterion — *"no pass uses an unqualified 'semantics-preserving' claim"* | The FM1.6 criterion already forbids this class of overclaim; it is unenforced. |
| Two refinement fixtures are self-referential; `model_checks` hardcoded `true` | **FM1.5** *Resolve, simulate and verifier proofs* (remaining: refinement for the simulation/verifier models) | Named as remaining work; the defect is that the ledger reads as if it were done. |
| `SegmentKind` wire spelling wrong in `sdk/ts` | **Phase 4 exit gate** — *"a fixed design authored in Python, TS, and Rust produces byte-equal Dry IR"* | Breach of a claimed-complete exit gate. |
| `crates/cloud` `POST /verify` contradicts ADR 0003 | ADR 0003 (Accepted) | Governance: amend the ADR or revert the route. Owner decision. |
| `resolve` doubles as the spline geometry kernel; `generate/` labelled L0 in two docs and L1 in the code | **D1.1** *(open, `[ ]`)* — freeze a reality-based dialect architecture | D1.1 is unstarted, and its absence is exactly why the doc contradiction has no arbiter. |
| ABB RAPID has no golden and no external oracle | **v0.9.0 / Phase 8** — *"Industrial CNC & robot post-processors (… ABB RAPID)"* | The horizon's own deliverable is unpinned. |

**Roadmap edits this work implies** (one PR, at the end): add a `v0.11.0` row to the Version Horizons table describing post-audit parity and assurance-honesty remediation; annotate the v0.9.0 row to record that its capability-parity gate was found non-enforcing and repaired in v0.11.0. Do not silently rewrite the v0.9.0 status — the roadmap already sets the precedent (2026-08-30 note) that "merged" and "released" are different claims, and "gated" and "enforced" are too.

---

## Findings

Severity is the dispatcher's, after re-verification. `path:line` is at `cf3216e`.

### C1 — blocker · Python and TypeScript SDKs pass programs the engine refuses

`py/python/dry/__init__.py:422` and `sdk/ts/src/design.ts:535` each hand-roll the machine-compatibility pre-flight instead of calling the engine. Each implements 5 of the engine's 7 rule codes, omitting `ARC_OUT_OF_BOUNDS_X` and `ARC_OUT_OF_BOUNDS_Y`.

```
$ git ls-files | xargs grep -ln 'ARC_OUT_OF_BOUNDS'
crates/core/src/profile/capability.rs
crates/core/tests/retrospective_audit.rs
```

Both local loops read only `seg.end`; neither looks at an arc's centre or radius. `crates/core/src/profile/capability.rs:187-220` bounds an arc by its **full circle** and raises `Severity::Error`. So an arc whose endpoints sit inside the build envelope while its circle leaves it returns `compatible: true` in Python and TypeScript and `compatible: false` in Rust and wasm.

The direction is not incidental. `capability.rs:189-192` states it: *"refusing a safe program is recoverable, passing an unsafe one is not."* The two SDKs invert exactly that.

`crates/wasm/src/lib.rs:446` does the right thing — `dry_core::check_compatibility(&tp, &caps)`. There is no PyO3 binding for the capability check at all (`grep -n compat py/src/lib.rs` is empty), which is why the Python side grew a copy.

**Aggravating:** `crates/core/tests/retrospective_audit.rs:93` asserts `f.code == "ARC_OUT_OF_BOUNDS_X" || f.code == "OUT_OF_BOUNDS_X"`. The disjunction is satisfied by the plain bounds rule, so the arc rule is pinned by no test in the repository.

**Wire-shape note for the fix.** The three surfaces disagree on the capability document shape and the fix must not break callers:

| | axis range | feedrate ceiling | spindle ceiling |
|---|---|---|---|
| Rust (`MachineCapabilities`, serde) | `x_range: {min, max}` | `max_feedrate_mm_min` | `max_spindle_rpm` |
| Python (current public kwarg) | `x_range: [min, max]` | `max_feedrate` | `max_spindle_rpm` |
| TypeScript (current public interface) | `xRange: {min, max}` | `maxFeedrate` | `maxSpindleRpm` |

Both bindings therefore need a boundary adapter, not a signature change. The public Python and TypeScript shapes stay exactly as they are; only the result changes, by gaining the two arc codes.

Both surfaces compare the raw IR `speed` field against their ceiling, as the engine does, so there is **no** unit divergence to fix alongside this.

### C2 — major · The parity gate cannot see this class of defect

`tools/check_capability_parity.py:99` is `present = symbol in target.read_text(...)`. The manifest is genuinely bidirectional (`reachable` must appear, `absent` must not, and an `absent` cell needs a `note`), which is better than a presence check — but a substring test cannot distinguish *delegating to the engine* from *reimplementing the engine locally*, and nothing asserts the manifest is complete.

`conformance/capability-parity.toml` declares 12 capabilities. `crates/core/src/lib.rs` exposes 35 `pub mod`s and `crates/wasm/src/lib.rs` 36 exports. There is no `machine-compatibility` row at all, and the manifest's `python`/`ts` cells point at `py/src/lib.rs` and `sdk/ts/src/engine.ts` — neither of which is where C1 lives.

### C3 — major · `crates/cloud` fails open on a malformed contracts header

`crates/cloud/src/lib.rs:52`:

```rust
Some(contracts_str) => serde_json::from_str(&contracts_str).unwrap_or_default(),
```

A malformed `X-Dry-Contracts` header degrades silently to `Contracts::default()`, documented at `crates/cloud/src/lib.rs:15` as *"all contract-driven checks disabled"*, and the caller receives a clean-looking report with HTTP 200. `containers/verify-runner` rejects bad input with a 4xx (`Stage::InputInvalid`).

### C4 — major · Tests that exist and never run

- **`crates/wasm`'s 4 unit tests are executed by no CI job.** The crate is excluded from the workspace (`Cargo.toml:13`), so `ci.yml:76 cargo test --workspace` cannot reach it, and the `wasm` job (`ci.yml:232-296`) has no `cargo test` step. Across every workflow there are exactly six `cargo test` invocations and none runs in `crates/wasm`. Separately, `crates/wasm/src/lib.rs:786` is a `#[cfg(target_arch = "wasm32")] #[test]` with no wasm test runner configured anywhere — unreachable by construction.
- **`crates/core/tests/emit_rejects_unrepresentable.rs` has a release-only branch that is never compiled.** The file states at `:63-69` that it must be run under `cargo test --release`; the sole release step, `ci.yml:119`, is scoped to `--test emit_refuses_non_finite` — a different file. The shipping-release behaviour of `emit()` for out-of-contract `CncFrame` values and endpoint-less arcs is verified nowhere.

### C5 — major · Two Lean refinement fixtures check nothing, and say they do

Verified verbatim.

`formal/Dry/Tests/ResolveChannelsFixtures.lean:131` derives the fixture's `expected` block from `resolve` itself, and the entire registered Lean backing of `FM1.RESOLVE_CHANNELS.NATIVE.REFINE.CORPUS` is `:153`:

```lean
def resolveChannelsFixtureChecks : Bool := decide (cases.length = 6)
```

`formal/Dry/Tests/SimulateMetricsFixtures.lean:41` does the same — `expectedMetrics` is `foldMetrics` applied to the model — and `:78` checks `cases.length = 7 && … segmentCount ≤ 10` (the maximum actual segment count is 4).

Both emit `("model_checks", Json.bool true)` — a hardcoded constant (`ResolveChannelsFixtures.lean:146`, `SimulateMetricsFixtures.lean:130`) — while the Rust consumers assert on that field (`crates/core/tests/resolve_channels_refinement.rs:129`, `crates/core/tests/simulate_metrics_refinement.rs:101`). Both assertions are unfalsifiable. The other nine fixtures emit `Json.bool modelChecks` correctly.

### C6 — major · The claim ledger flattens four proof strengths into one word

The formal layer is stronger than expected: **0 `sorry`, 0 `admit`, 0 custom `axiom`** across 42 modules / 9,340 lines / 192 theorems, all 42 registered claims resolve to real declarations, and all 8 source hash pins are current. The problem is the summary layer.

Six claims are discharged by `native_decide` — compiled evaluation, outside the kernel, at `OrientationContractFixtures.lean:102`, `ResolveOrientationFixtures.lean:119`, `NestedApplicationFixtures.lean:163`, `SimulateMetricsFixtures.lean:79`, `NativeNumericFixtures.lean:202`, `CompositionShapeFixtures.lean:121`, `DepositionFixtures.lean:61`. A repo-wide grep for `native_decide` across `*.md`, `*.toml` and `*.yml` returns **zero** hits: `proofs/claims.schema.json` has no proof-strength field and `docs/assurance/01-assurance-sitemap.md` prints `proved` for all 42.

Two claim titles outrun their theorems:

- `FM1.NUMERIC.SCURVE.BOUNDS` — titled *"7-Phase S-curve velocity and acceleration profiles strictly respect positive bounds and finite time duration"*; `formal/Dry/Numeric/SCurve.lean:31` proves five sign conditions. No phases, no `v(t)`, no `a(t)`, no duration.
- `FM1.GEOMETRY.BREP.NORMAL` — titled *"Analytical B-Rep quadric surface normal evaluation"*; `formal/Dry/Geometry/Brep.lean:25` proves that three constant vectors have unit length.

The per-claim `exclusions` prose is unusually honest and names most of these gaps itself, so this is a summary-layer defect, not concealment.

### C7 — minor · `Geometry/Kinematics.lean` models a branch that does not exist

`formal/Dry/Geometry/Kinematics.lean:44-48` returns `c := c_prev` in **both** arms of its `if`, so `singularity_hold_c_preserved` (`:51`) holds unconditionally and the hold it claims to verify is never exercised. The Rust resolver assigns `state.c = libm::atan2(j, i)` outside the cone (`crates/core/src/emit/kinematics.rs:154-156`). The thresholds also differ: Lean `singularityThreshold := 1e-5` (`:36`) against Rust `SINGULAR_CONE_SIN_TILT: f64 = 1e-9` (`crates/core/src/emit/kinematics.rs:117`).

The module backs no registered claim, so the ledger is not lying — the module is.

### C8 — minor · `sdk/ts` declares a `SegmentKind` that never matches the wire

`crates/core/src/ir.rs` carries `#[serde(rename_all = "lowercase")]` on `SegmentKind`, so the JSON value is `manualgcode`; `as_str`/`from_wire` use `manual_gcode` for the binary forms only, and `spec/dry-ir-v0.schema.json:52` documents the split explicitly. `sdk/ts/src/ops.ts:92` declares `'manual_gcode'`, so `seg.kind === 'manual_gcode'` never matches a decoded IR document.

Adjacent, same file family: `sdk/ts/src/machine.ts:14` declares a firmware-flavor union containing `'reprap'` (which `FirmwareFlavor::named()` rejects) and missing `duet`, `siemens`, `heidenhain`, `haas`, `rapid`; a second, correct 15-name union already exists at `sdk/ts/src/engine.ts:159-174` and is the one `index.ts` re-exports.

### C9 — minor · Codecov is an artifact upload, not a gate

`.github/workflows/codecov.yml` computes `lcov.info` and uploads it. The `codecov/codecov-action` step is commented out and there is no threshold, so a coverage regression cannot fail CI. Either assert a floor or say in the workflow header that it is informational.

### C10 — minor · ABB RAPID is the only emitter with no oracle

`conformance/reports/robot/` contains exactly one golden (`reference-five-axis.src`, KRL) and `conformance/reports/cnc/` exactly one (`pocket-rect-rs274.ngc`). RAPID has neither a golden nor an external parser, and its only test is `crates/core/tests/cnc_industrial_flavors.rs:95`, which reaches one of three quaternion branches. Unreached: the antipodal case at `crates/core/src/emit/rapid.rs:16-18` (a 180° X rotation — a robot flip if wrong), the general case at `:19-26`, `Dwell`/`WaitTime` at `:54-59`, `Arc`/`MoveC` at `:91-99`. Separately `crates/core/src/emit/rapid.rs:32` takes `_params: &EmitParams` and ignores it entirely, so `cnc_frame` work offsets are silently dropped.

### C11 — minor · Four CLI subcommands have no test

`unpack`, `explain`, `schema` and `fleet` appear zero times in `crates/cli/tests/*.rs`. `pack` is tested, so the `pack` → `unpack` round trip is open.

### C12 — info · `resolve` is both a lowering pass and the spline geometry kernel

`SAMPLES`, `catmull_rom` and `dist` are `pub` out of the L1→L2 pass (`crates/core/src/resolve.rs:126,669,686`) and three layers consume them — `crates/core/src/clothoid.rs:50` (L1), `crates/core/src/verify.rs:17` (L2), `crates/core/src/emit/spline.rs:2` (L3). Each of the three re-writes the same Catmull-Rom windowing loop rather than calling one sampler (`resolve.rs:1096-1105`, `verify.rs:850-859`, `emit/spline.rs:57-66`). The only cross-copy gate is `crates/core/tests/spline.rs:41`, `assert_eq!(gcode.len(), 49)` — cardinality, not coordinates, so a divergent end-clamp keeps the count and passes.

**The three copies currently agree.** This is a structural risk with no present defect, and it belongs to D1.1 rather than to this remediation.

### C13 — info · Two normative docs label `generate/` L0; the code says L1

`docs/27-system-capabilities-and-architecture-graph.md:23` and `crates/core/README.md:13` place TPMS, pocket, drape, lathe, thread-mill and B-Rep at L0. Against that: `crates/core/src/generate/mod.rs:1,7` ("pure **L1** sugar"), `crates/core/src/generate/pocket.rs:4`, and ADR 0002 `:35`. The code settles it — `crates/core/src/generate/tpms.rs:19` imports `crate::resolve::{Design, Op}` and produces L1 `Op`s; true L0 is `crates/core/src/features.rs`.

There is no dialect ADR to arbitrate, because **D1.1 is still `[ ]`** in `docs/04-tasks.md:131`.

### C14 — info · `crates/cloud`'s product route contradicts ADR 0003

ADR 0003 (Accepted) says `crates/cloud` *"is not a candidate and stays archived"*; `crates/cloud/README.md:31` says *"no tests, no authentication and no error contract. Do not build on it."*; `crates/cloud/src/lib.rs:2` still reads *"This crate is NOT product code"*. Commit `591ccf0` nevertheless added a product `POST /verify` returning a real `dry_core::Report`, and `.github/workflows/verify-runner.yml:45` names a job "Build Check (Tier 2 Edge Worker)". `docs/27-verification-deployment-architecture.md:82` additionally claims Tier 2 is *"Tested in `crates/cloud/src/lib.rs`"*, a file with zero `#[cfg(test)]`.

Tier 2 also uses `review_import_params()` (forcing `line_width = 0.45`, `layer_height = 0.2`) and `Contracts::default()`, while Tier 3 uses `profile.gcode_import_params()` and `profile.contracts()` — `containers/verify-runner/src/lib.rs:9-12` names that exact substitution as the byte-identity breaker.

**This is an owner decision, not an engineering one:** amend or supersede ADR 0003 to describe the Tier 2 that exists, or revert the route. C3 (fail-open) is a defect either way and is fixed independently.

---

## What was verified and found correct

Recorded so the remediation does not "fix" things that are already right.

- **The workspace/binding split is real.** All four excluded roots carry their own `Cargo.toml` + `Cargo.lock` + an empty `[workspace]` table with the reason written down; every path dep resolves and every excluded lock records `dry-core 0.10.0`. The root `Cargo.lock` contains exactly the five members — no binding transitives.
- **`crates/core` is binding-free**, and not conditionally so: zero occurrences of `wasm_bindgen`/`pyo3`/`js_sys`/`web_sys`/`napi`/`worker::`, **no `[features]` table at all**, and no `target_arch`/`target_os` escape hatch.
- **80 source files, 80 reachable, 0 orphans, 0 stubs** in `crates/core/src`, including the `#[path]` indirections.
- **The lowering direction holds.** `generate/` imports only `crate::resolve::{Design, Op}` in production code. The two backward edges (`resolve` → `verify::ARC_RADIUS_TOLERANCE_MM`, `verify` → `emit::RotaryState`) are each justified at the import site as deliberate single-source-of-truth anti-drift.
- **No core module is untested.** 253 unit tests across 31 files plus 410 integration tests across 94 files; the two modules whose symbols appear in no test by name (`emit/spline.rs`, `emit/rapid.rs`) are reached through `emit()`.
- **Goldens are compared, never regenerated in CI.** All six regeneration paths are env-gated (`UPDATE_GOLDEN`, `UPDATE_REPORTS`, `UPDATE_VECTORS`, `UPDATE_PROFILE_MATRIX`, `DRY_REGEN`) and no workflow sets any of them. The `krl` and `linuxcnc` jobs exist specifically to make abuse of those variables detectable by a third-party parser.
- **`containers/verify-runner` is the model consumer** — no duplicated logic, and byte-identity to the CLI pinned by a test that shells out to the compiled binary.
- **`tools/check_proof_fixtures.py` is stronger than the dispatcher initially assumed.** It does not inspect only the return code: `main()` (`:576-588`) byte-compares every fixture's stdout against its committed snapshot and JSON-Schema-validates nine of them. The weakness is in the fixtures (C5), not the harness.
- **Version lockstep has no manifest gap.** `scripts/check-version.sh` covers 19 manifests plus the CHANGELOG heading and a computed four-year BUSL Change Date; the per-crate `LICENSE`/`NOTICE` copies are covered by the separate byte-identity gate in `tools/check_license_headers.py`.

## Known gaps in this audit

- Nothing here is runtime-verified on the binding side. C1's divergence is established by reading three implementations plus the rule-code inventory; the plan therefore opens with an executable reproducer rather than assuming it.
- No `lake` was run during the audit, so no `#print axioms` evidence exists for any theorem. The `sorry`-free grep plus a clean `--wfail` build is strong but is not an axiom audit; a `native_decide` claim's dependency on `Lean.ofReduceBool` is inferred from the tactic, not from the kernel.
- `web/`, `services/cloud/src/*.ts` and `sdk/mcp` were not audited for engine-logic reimplementation. Given C1 and C2, those are the highest-value places to look next.
