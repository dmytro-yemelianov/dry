# Core Hardening Audit — findings backlog

Date: 2026-07-31 · Scope: `crates/core` (units, resolve/engine, emit/gcode, verify, generate/tpms)
Method: five parallel read-only audits hunting the seven defect classes found and fixed in `generate/pocket.rs` (commits `7071ce1`, `e31080b`). Every finding below was confirmed by the auditor against the code, most by execution.

Defect classes: **D1** degenerate input silently accepted · **D2** guard mode-blind/unreachable/bounds the wrong quantity · **D3** range property pinned at one sample · **D4** assertion satisfiable by an echoed input (mutation survives) · **D5** doc/message outruns behavior · **D6** `pub` API silently misusable · **D7** NaN/inf/overflow unhandled

---

## Status (updated 2026-07-31)

| Slice | State |
|---|---|
| **H1.1 emit safety gate** | **Closed.** Non-finite words, five-axis orientation, RS-274 prologue leakage, `CncFrame` validation, arc-without-endpoint. Two review rounds; the second found the refusal had not been followed to the CLI (truncated file on disk), the STEP-NC sidecar (written first, ungated), or the bindings (empty array read as success by `web/viewer.js` and `sdk/ts`). |
| **H1.2 ingress validation** | **Closed.** All five ingress paths admit no non-finite quantity. Three rounds: round 2 found the accept clause still false on the `resolve_checked` path nobody had checked; round 3 found two of this document's own follow-up notes had drifted ahead of the code. Decisions recorded in [ADR 0002](../../adr/0002-numeric-ingress-and-emission-gates.md). |
| **H1.3 verify strengthening** | Open — [#185](https://github.com/dmytro-yemelianov/dry/issues/185). The largest remaining item and the gate on any product claim that leans on "verify is clean". |
| **H1.4 TPMS hardening** | Open — [#186](https://github.com/dmytro-yemelianov/dry/issues/186), plan at `docs/superpowers/plans/2026-07-31-tpms-hardening.md`. T4 (`maxFieldSamples` sentinel) is blocked on a wire-contract decision, since `sdk/ts` encodes `Infinity → 0`. |
| **H1.5 formal-model speed divergence** | Open — surfaced by H1.2. |

**Two corrections to this document**, both found by implementers working from it — recorded because it is the source spec for the open slices:

1. `Area::sqrt` was described as having zero production call sites. It does not: `resolve.rs` calls it inside `pub fn dist`. The `optimize/arc.rs` sites cited alongside it are a local closure over `libm::hypot` that never touches `Area`.
2. That same fallback was then described as unreachable ("a sum of squares is never negative"). Also false — `Area::sqrt` returns `None` for NaN as well as for a negative area, and a release-build spline with ~1e308 control points reaches it.

The pattern in both is the one this audit exists to find: a claim that outran the behaviour it described. Treat the remaining entries as hypotheses to verify, not as established fact.

---

## The unifying finding

**Non-finite quantities reach metal.** `num()` (`emit/gcode.rs:93`) is `format!("{v:.6}")` plus trimming; Rust renders NaN as `NaN` and infinities as `inf`. Confirmed output: `G1 FNaN Xinf YNaN E0`. A `finite` rule exists in `verify.rs:210`, but `dry emit` streams IR from disk straight to the emitter (`cli/src/main.rs:862`) and never calls it. Three of the five audits reached this independently.

Known ingress paths, all confirmed:
1. **Binary codec** — `Reader::f64` (`codec/util.rs:29`) is bare `from_le_bytes`, no validation; `columnar.rs:226` wraps results directly into `Length::mm`. `DecodeLimits` bounds sizes, never values — hostile input is already in the threat model.
2. **G-code import** — `flow_ratio_from_percent` (`gcode.rs:622`) detects non-finite then *returns it anyway*; `M221 S1e400` → `inf` → `0.0 * inf = NaN` → segment emits `E NaN`.
3. **`ResolveParams`** — `retraction_distance`/`retraction_speed` are never validated (`resolve.rs:187`), consumed at `:648`. Negative distance flips a retract into an unretract; NaN propagates into every metric. Live on every binding (wasm/py deserialize from caller JSON).
4. **Feedrate** — `Feedrate(0.0)` / negative accepted; `simulate` returns `None` for zero-speed segments (`engine.rs:31`), so they contribute nothing to any metric *and* are immune to the max-flow ceiling.

`units.rs` underpins this: `Length::mm` (fan-in 41) carries no invariant — no `try_mm`, no `debug_assert`, `pub` tuple fields plus `value()` make the newtype wall porous. It is an excellent *dimensional* foundation (unit confusion is a compile error, genuinely load-bearing) and a weak *numeric* one.

---

## Critical

| # | Finding | Location |
|---|---|---|
| C1 | Non-finite values emitted verbatim as G-code words (the above) | `emit/gcode.rs:93` |
| C2 | Five-axis: a non-unit or zero orientation silently moves the **linear** axes to the wrong point. `Ac`/`Bc` recover tilt via `acos(k)` assuming ‖v‖=1; `Ab` uses `atan2` and is scale-invariant, so the three models disagree on identical input. Confirmed: `[0,0,0.5]` → `Z-8.660254 B60` (wrong point *and* angle); `[0,0,0]` accepted silently | `emit/kinematics.rs:135,164` |
| C3 | `modal_rewrite_prologue` splices `M83`/`M82`/`G92 E` into **RS-274** programs — flavor-blind, three lines below the `cnc_frame` guard written to prevent exactly this class. Unknown M-code aborts on LinuxCNC/Fanuc. The regression test asserts only that *frame* lines are absent, so it passes with the bug present (D4) | `gcode/lift.rs:321,324` |
| C4 | No segment-continuity rule anywhere in `verify`; the emitter writes endpoints only. A gap between segment *i*'s end and *i+1*'s start produces **no repositioning move** — the machine cuts a straight line across it, along a path no rule inspected. Consequence: `monotonic-z` is intra-segment only, so a vase-mode path that plunges 0.6 mm between segments verifies clean | `verify.rs:559-868`, `emit/gcode.rs:193` |
| C5 | No rule relates deposited material to geometry. `volume ≈ length×width×height`, `filament ≈ volume/(πd²/4)`, arc `length ≈ r×sweep` — none checked. An 8000× under-extrusion passes every rule | `verify.rs:590,642` |

## Important

**Emit** — C-frame `wcs`/`spindle_rpm` validated only in `profile`, not on the `pub`/`Deserialize` path (`wcs:0` → emits `G0` where `G54` belongs; `spindle_rpm:0` → `S0 M3` before a cutting move) `emit/gcode.rs:149` · Arc with `end:[None,None]` emits bare `G3 I-10 J0` = **full 360° circle**; the importer refuses the same construct, so round-trip tests can't see it `emit/gcode.rs:251` · RobotKRL emits two `C` words on one line (rotary vs arc offset collide) and `CIRC` loses arc direction — CCW indistinguishable from CW `emit/gcode.rs:271,295`

**Resolve/engine** — `ResolveParams` retraction unvalidated (above) `resolve.rs:187` · zero/negative feedrate silently dropped or negative-time `engine.rs:31` · `Arc` before any positioning move fabricates a `(0,0)` start, producing IR that `resolve_checked` blesses and `verify` then errors on `resolve.rs:491` · zero bead accepted: `width`/`height` default `ZERO`, never required before an extruding move `resolve.rs:383` · `pub fn resolve` panics on invalid input and is the more attractive name; ~40 in-tree call sites already pick it, and an unwind across PyO3/wasm is an abort `resolve.rs:373`

**Verify** — sign and zero unchecked (negative length/volume/speed all pass; zero-length extruding move turns off three rules at once) `verify.rs:570` · `junction-velocity` measures a *speed difference*, not a direction change — misses the constant-speed 90° corner it is named for, and disagrees with `adaptive_speed.rs` which computes the right thing under the same name `verify.rs:841` · `Contracts::default()` leaves only 5 of 18 rules able to fail, yet 8 call sites use `report.ok()` under it as an assurance claim · a vacuous pass is byte-identical to a real one — `Report` records neither segments inspected nor contracts in force `verify.rs:306`

**TPMS** (carries an analogue of **all seven** pocket defects; two are worse) — `isoLevel ≥ 1.5` or a large `minPathLength` yields a 4-op program with **zero moves** that resolves, verifies with zero findings, and simulates to zero volume; `isoLevel` is never set by any test `tpms.rs:235` · `maxFieldSamples ≤ 0`/NaN silently disables the DoS backstop, reachable from raw JSON on wasm+PyO3 — and `sdk/ts` deliberately encodes `Infinity → 0`, so fixing it is a coordinated 4-target change `tpms.rs:632` · dedupe threshold `1e-7` vs emission grid `1e-6` produces coincident extruding moves **at default options** on 3 of 10 surfaces `tpms.rs:863` · adaptive budget over-estimates 15× (2001 layers charged vs 133 actual), rejecting jobs that are legal without `adaptive` `tpms.rs:635` · **adaptive layers extrude at the fixed base `beadHeight`** — measured 8× over-extrusion on a 0.05 mm layer; no verifier can catch it because the IR faithfully records the wrong bead `tpms.rs:422`

## Minor / test-strength

Units: over half the macro-generated operators have no test; the `atan2` assertion is satisfied by echoing its own input (`units.rs:228`); `Area::sqrt` returned `NaN` for negative area and is *not* dead code, contrary to this audit's first draft: `resolve.rs:356` calls it inside `pub fn dist`, which is used at `resolve.rs:459,602` (H1.2 made `sqrt` return `Option<Length>`; `dist` still falls back to `Length(f64::NAN)` on `None`, so the hazard moved rather than closed — H1.3 territory). Emit: `non_extruder_flavors_emit_no_e_words` covers 2 of 6 flavors in one direction; the 5-axis arc-I/J test is satisfied by an implementation that ignores orientation entirely; Klipper dwell saturating-casts (`-1.0 s` → `G4 P0`). Relative-E (the default) quantizes each extrusion independently at 6 decimals with no residual carry, drifting systematically downward on micro-segment paths. TPMS: budget boundary test uses an integral ratio so `ceil→floor` survives (same shape as the pocket bug just fixed).

## Artifact obligations

`proofs/` is **accurate, not stale** — `claims.toml:825,866,629` explicitly exclude non-finite refinement and "an output-finiteness rejection gate", and `:1354` excludes "invalid or zero speed behavior". The formal artifacts assume a runtime gate that C1 shows does not exist. But: `FM1.VERIFIER_SOUNDNESS` (`:1410`) assumes verifier predicates hold iff physical constraints do, excludes refinement against `verify.rs`, and is marked `refinement = "not-applicable"` — that should read `"pending"`. `FM1.UNIT.NORMALIZE_CONVERT` (`:1094`) names `units.rs` as its Rust source, but `units.rs` contains no unit conversion at all (the only mm/inch conversion is `gcode/lift.rs:510`). `emit` and TPMS are outside the claims corpus entirely. `spec/` has no schema for `ResolveParams` and no `minimum` on `Segment.speed`.

## Suggested slicing

1. **Emit safety gate** (C1–C3 + CncFrame/arc-endpoint validation) — the last line before metal; CNC-relevant.
2. **Ingress validation** (codec `Reader::f64`, `M221`, `ResolveParams`, feedrate sign/zero) — closes the paths that feed C1.
3. **Verify strengthening** (C4, C5, sign checks, junction direction) — design work; changes what "clean" means.
4. **TPMS parity hardening** — mirrors `7071ce1`/`e31080b`; needs 4-target coordination for the `maxFieldSamples` sentinel.
