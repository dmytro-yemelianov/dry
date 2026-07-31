# ADR 0002 — numeric ingress and emission gates

- **Status:** Accepted
- **Date:** 2026-07-31
- **Workstream:** H1 (core hardening)

## Context

A five-subsystem audit of `crates/core` on 2026-07-31 (`docs/superpowers/specs/2026-07-31-core-hardening-audit.md`) found that non-finite quantities could travel from untrusted input all the way into emitted G-code. `num()` in the emitter is `format!("{v:.6}")` plus trimming; Rust renders NaN as `NaN` and infinities as `inf`, so a `Toolpath` carrying either produced machine-facing words like `G1 FNaN Xinf`. A `finite` rule existed in `verify`, but `dry emit` streams IR from disk straight to the emitter and never calls it — so the emitter was the last gate and had none.

Three of the five audits reached that conclusion independently, from different subsystems. That convergence, rather than any single finding, is what made it architectural rather than a bug.

Two facts shaped every decision below.

**The conformance corpora cannot see this class of defect.** Every corpus is generated from the FullControl oracle and is therefore well-formed by construction. Nothing in the suite exercises malformed, degenerate, or hostile input, so a fully green CI told us nothing about robustness. This is recorded as a risk in `docs/02-roadmap.md`.

**`units.rs` is a dimensional foundation, not a numeric one.** The typed newtypes make unit confusion a compile error across 41 call sites — genuinely load-bearing — but `Length::mm` was a rename of `f64`: no checked constructor, `pub` tuple fields, and a `value()` escape hatch. Every call site had to remember to validate, and the compensation was not uniform.

## Decision

### 1. Validate at ingress; gate at emission; treat the gate as the last resort, not the design

Every path that constructs an IR quantity from data the engine did not produce validates that quantity. The emitter additionally refuses to render what it cannot faithfully represent. The two are defence in depth: the emit gate exists because `dry emit` runs no verifier, not because ingress validation is optional.

Five ingress paths are in scope and all are now closed: the binary codec, the JSON codec (which `serde_json` already made safe — it rejects both the `NaN`/`Infinity` literals and out-of-range magnitudes), G-code import, 3MF import, and design JSON through `resolve_checked`.

### 2. At `resolve`, use a postcondition on the produced toolpath — not a magnitude bound on the input

`validate_design` checks that coordinates are finite but imposes no magnitude bound, so finite inputs can still produce non-finite IR through arithmetic: `d*d` overflows in `dist`, `π(dia/2)²` underflows to exactly zero and makes `volume/area` infinite, and so on.

The rejected alternative was a magnitude bound in `validate_design`. It fails because the bound would have to be *jointly* sufficient across `dist`'s squares (bound ≈1.3e154), the four-way product in `volume = length·width·height·flow`, `filament = volume/area` (which needs a joint bound with `dia`), `radius·swept`, and `Op::Spline`'s `total_length`, which sums `SAMPLES × spans` terms with no bound on `points.len()`. Any bound loose enough to admit real designs is insufficient for the products; any bound tight enough for the products refuses legitimate work. It is strictly worse on both axes.

`require_finite_toolpath` runs after lowering and rejects iff the produced IR is genuinely non-finite. The rejection boundary was measured, not argued: 20,000-segment prints, 50,000-point splines, TPMS gyroids, every real filament diameter, and coordinates up to 1e154 mm are all accepted; rejection begins where `f64` itself gives out. It is not a magnitude policy in disguise.

**Consequences.** One extra O(n) pass over an already-O(n) lowering. Errors name the segment and field (`segments[1].length resolved to inf`) rather than the offending op — worse than the G-code path's source-line errors, and accepted as the price of not inventing a magnitude policy. The postcondition also covers `generate/` for free, since TPMS and pocket are L1 sugar that resolves through the same call.

**Known limit.** `resolve_checked` is not total in debug builds: a pathological spline can panic on `Length::mm`'s `debug_assert` inside `resolve_unchecked` before the postcondition converts it to an `Err`. Release returns the error cleanly. A panic puts nothing in the IR, so the accept clause holds, but debug-built bindings abort where release raises.

### 3. `emit()` keeps its signature; new work uses the fallible `emit_stream`

`emit()` is infallible and re-exported through four out-of-workspace crates, so making it fallible would break all of them at once. It keeps its signature, refuses the whole program rather than emitting a partial one, and is now `#[deprecated]` in favour of `emit_stream`. Both bindings were migrated in the same slice.

That migration was not optional polish. Before it, refused IR surfaced on wasm and PyO3 as an **empty array**, reachable from three ops of ordinary JSON — and `web/viewer.js` and `sdk/ts` both rendered that as a successful blank program. Deferring would have left those surfaces in the least coherent of the three possible states, having already changed them from `NaN` lines to `[]`.

### 4. Refuse; do not clamp, and do not silently emit nothing

Where an input cannot produce a meaningful program, return a named error rather than clamping it into range or emitting an empty/vacuous result. A generator that accepts `tool_diameter == width` and emits a plunge-and-retract with no cutting moves, or a `perimeterInset` clamped into a 2e-9-wide rectangle, has produced a confidently wrong artifact — strictly worse than an error.

The same rule applies to *tests*: a vacuous pass must be distinguishable from a real one. `tools/linuxcnc_check.sh` rejects a file that the interpreter accepts but from which it derives zero canonical operations.

### 5. An independent oracle is required for any "this output is valid" claim

Dry checking Dry — its own parser, its own frozen goldens — cannot establish that emitted output is valid for a real controller. `tools/linuxcnc_check.sh` runs emitted RS-274 through `rs274`, the reference LinuxCNC interpreter, which re-reads the program and emits canonical machine operations; a file passes only if LinuxCNC agrees it describes real motion. This runs in CI in a Debian container (`linuxcnc-uspace` is a stock Debian package and is absent from the Ubuntu archives entirely).

This is syntactic and semantic-interpretation evidence, not physical evidence. Under ADR 0001's layering it does not reach layer 4; controller/machine qualification remains outside it.

### 6. Formal artifacts are authority; code changes yield to them

When a change conflicts with a registered claim or its corpus, the change is reverted and the decision pinned in a test — the model is not quietly edited to match new code. This happened once here: making `simulate` account for zero-speed segments failed the `FM1.SIMULATE_METRICS` refinement corpus, because the Lean model specifies the swallowing behaviour. The Rust change was reverted.

Where the two genuinely diverge, that is a finding against the artifacts, tracked as its own task rather than resolved unilaterally (H1.5, negative feedrate).

## Consequences

- Acceptance narrowed on published surfaces. `resolve_*`, `decode`, and `import_gcode*` now reject inputs they previously lowered into non-finite IR; `emit` refuses rather than rendering it. All are recorded as BREAKING in `CHANGELOG.md`. Nothing that previously *worked* is refused.
- ~~No binding-level test covers the new rejections.~~ **Closed** ([#192](https://github.com/dmytro-yemelianov/dry/issues/192)). `sdk/ts/test/h1-rejections.test.ts`, `py/tests/test_h1_rejections.py` and two tests in `containers/verify-runner/tests/handler.rs` now exercise the refusals from the published side, against the real compiled engine. They assert the same contract in both SDKs, so a divergence between the two bindings fails a test rather than reaching a user. Writing them settled a question this ADR had left open: `emit` **normalises** a non-unit orientation rather than refusing it (`unit_orientation`, `emit/kinematics.rs:49`) — the stronger choice, since a non-unit direction vector is unambiguous — while `verify` still reports `orientation-not-unit` on the IR that carries it. The two surfaces have different jobs and the tests pin both. `crates/cloud` is deliberately not covered: it is a 105-line feasibility spike that returns timing JSON rather than a `Report`, and testing its `worker::Request` handlers needs a Workers runtime.
- Degenerate-input vectors belong in `conformance/`. Oracle-generated corpora cannot produce them, so robustness needs its own fixtures.
- `verify` remains a strong *contract* checker and a weak *well-formedness* checker. It has no segment-continuity rule and no material-consistency rule, and under `Contracts::default()` only 5 of 18 rules can fail — yet several call sites use `report.ok()` as an assurance claim. This ADR does not fix that; H1.3 does, and until it lands, "verify is clean" certifies less than it reads.

## Corrections to the audit

The audit is a working document and was wrong in one place: it recorded `Area::sqrt` as having zero production call sites. `resolve.rs` calls it inside `pub fn dist`. The `optimize/arc.rs` call sites cited alongside it are a local closure over `libm::hypot` and never touch `Area`. Corrected in place; noted here because the audit is the source spec for H1.3 and H1.4.
