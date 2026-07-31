# TPMS Hardening Implementation Plan (H1.4)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Every documented TPMS parameter range either produces material or is refused, adaptive layers deposit the bead they actually occupy, and no emitted path contains coincident moves.

**Architecture:** `generate/tpms.rs` is a pure L1 generator: options → validation → field sampling → marching-squares contours → `Vec<Op>`. All fixes are validation-side or emission-side within that file; the one exception is the `maxFieldSamples` sentinel, which is a wire contract shared with `sdk/ts`. TPMS is the **most-exposed** generator (wasm + PyO3 + TS), so acceptance changes are breaking-ish and need the four-target sweep every task lists.

**Tech Stack:** Rust (`crates/core`), TypeScript (`sdk/ts`), conformance JSON.

**Source:** `docs/superpowers/specs/2026-07-31-core-hardening-audit.md` → TPMS section. Every trigger value below was **measured** by the auditor against the real engine, not inferred.

## Global Constraints

- Binding parity is live and non-negotiable: after each task, `cargo check` in `crates/wasm`, `py/`, `crates/cloud`, `containers/verify-runner` from their own directories, and check `sdk/ts` for a mirrored contract.
- `conformance/gallery/gyroid_infill.json` is the only TPMS fixture. Tasks 2 and 3 **change emitted output for valid input** and require regenerating it — regenerate exactly once, in Task 3, and diff the op-count delta in the commit message. Tasks 1, 4, 5 must leave it byte-identical.
- Rejections use the existing `TpmsError` style and must name the offending option and its value.
- No new dependencies. Gate before each commit: `cargo fmt --all && nice cargo clippy --workspace --all-targets -j 4 -- -D warnings && nice cargo test -p dry-core -j 4`.
- Do not weaken any documented range to make a fix easier — refuse or fix the geometry, never silently narrow the contract.

---

### Task 1: Refuse vacuous programs

**Files:** Modify `crates/core/src/generate/tpms.rs` (validation at `:235`, `:318`, `:322`; a post-`build_layer_slices` check near `:526`)

**Interfaces:** Produces `TpmsError` for three input classes that currently return `Ok` with an empty program. No change to valid-input output.

The defect: `isoLevel` is checked only for finiteness. A value outside the surface's field range yields **4 ops, 0 moves** — and `resolve_checked` succeeds, `verify` returns **zero findings**, `simulate` reports **zero volume**. The user gets a file that heats the nozzle and prints nothing. The valid range is surface-dependent (gyroid saturates ≈±1.5, Schwarz-P ±3, Neovius/IWP/FRD differ) and is documented nowhere.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn iso_level_outside_the_field_range_is_rejected() {
    for iso in [1.5_f64, 5.0, 1e6, -1e6] {
        let opts = TpmsOptions { cells_x: 1, cells_y: 1, cells_z: 1, iso_level: iso, ..base_opts() };
        let err = try_tpms_ops(&opts).expect_err(&format!("isoLevel {iso} must be refused"));
        assert!(err.to_string().contains("isoLevel"), "message names the option: {err}");
    }
}

#[test]
fn min_path_length_that_filters_every_path_is_rejected() {
    let opts = TpmsOptions { min_path_length: 1e9, ..base_opts() };
    let err = try_tpms_ops(&opts).expect_err("a min_path_length above the contour scale must be refused");
    assert!(err.to_string().contains("minPathLength"), "{err}");
}

#[test]
fn perimeter_inset_collapsing_the_rectangle_is_rejected() {
    let opts = TpmsOptions { perimeter: true, perimeter_inset: 1e9, cells_x: 1, cells_y: 1, cells_z: 1, ..base_opts() };
    let err = try_tpms_ops(&opts).expect_err("an inset >= width/2 must be refused, not clamped");
    assert!(err.to_string().contains("perimeterInset"), "{err}");
}

#[test]
fn a_valid_job_still_deposits_material() {
    let ops = try_tpms_ops(&base_opts()).expect("default options are valid");
    assert!(ops.iter().any(|o| matches!(o, Op::Move { .. })), "control: valid options still emit moves");
}
```

- [ ] **Step 2: Run to verify failure**

`nice cargo test -p dry-core -j 4 generate::tpms -- --nocapture` — expect the first three to fail (currently `Ok`), the control to pass.

- [ ] **Step 3: Implement**

Post-slice emptiness check (covers `isoLevel` *and* `minPathLength` without hardcoding per-surface ranges — the surface-dependent range is exactly why a computed check beats a table): after `build_layer_slices()`, if no slice contributes a path and `perimeter` is off, return a `TpmsError` naming whichever option is implicated (report `isoLevel` with the surface name when the contour set is empty; report `minPathLength` when contours existed before filtering). Track the pre-filter contour count so the two cases are distinguishable — a message that blames the wrong option is worse than none.

For `perimeterInset` (`:322`), **reject instead of clamping**. The current `.min((width/2.0 - EPS).max(0.0))` silently produces a rectangle spanning `2e-9` — measured 44 zero-length extrusions presented as a perimeter wall.

- [ ] **Step 4: Verify green, then binding sweep + conformance**

Tests pass; `conformance/gallery/gyroid_infill.json` byte-identical (`git status` clean under `conformance/`); four `cargo check`s clean.

- [ ] **Step 5: Commit**

`fix(core): refuse TPMS option sets that emit no material (H1.4, #186)`

---

### Task 2: Adaptive layers deposit the bead they occupy

**Files:** Modify `crates/core/src/generate/tpms.rs` (`Op::Geometry` emission at `:422`, slice loop at `:440`, `base_layer_zs` at `:493`)

**Interfaces:** Consumes Task 1's validated options. Produces one `Op::Geometry` per slice instead of one per program. **Changes emitted output for adaptive jobs.**

The defect: `Op::Geometry` is pushed **once**, before any slice, carrying `bead_height` (which defaults to `layer_height`), and is never updated. `resolve.rs:448` uses it as deposited volume (`length × width × height × flow`). Measured with `layerHeight: 0.4, adaptive: true, adaptiveMinLayerHeight: 0.05`: **one** `Op::Geometry { height: 0.4 }` against actual layer gaps of **`[0.05, 0.1, 0.2, 0.4]`** — an **8× over-extrusion** on the thinnest layer. No verifier can catch this: the IR faithfully records the wrong bead, so `bead`, `max-flow` and every other rule see a self-consistent lie. This defeats the entire purpose of adaptive slicing.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn adaptive_layers_declare_the_bead_height_they_occupy() {
    let opts = TpmsOptions {
        cells_x: 1, cells_y: 1, cells_z: 1, samples_per_cell: 8, layer_height: 0.4,
        adaptive: true, adaptive_min_layer_height: 0.05,
        adaptive_max_length_delta: 0.01, adaptive_max_point_delta: 0.01,
        ..base_opts()
    };
    let ops = try_tpms_ops(&opts).expect("valid adaptive options");

    // Walk the op stream pairing each declared bead height with the Z gap it covers.
    let declared = declared_bead_heights_by_layer(&ops); // helper: (z, height) per slice
    for w in declared.windows(2) {
        let (z_prev, _) = w[0];
        let (z, height) = w[1];
        let gap = z - z_prev;
        assert!(
            (height - gap).abs() <= 1e-9,
            "layer at z={z} spans {gap} but declares bead height {height}"
        );
    }
    assert!(declared.len() > 2, "the fixture must actually refine: {declared:?}");
}
```

- [ ] **Step 2: Run to verify failure**

Expect a failure naming a 0.05 gap against a 0.4 declared height (the measured 8× case).

- [ ] **Step 3: Implement**

Emit a fresh `Op::Geometry { height: Some(actual_gap), .. }` at the start of each slice in the emission loop (`:440`). The first layer's gap is its Z above the build plate (or `layer_height` if that is the convention elsewhere — match `resolve`'s first-layer handling rather than inventing one).

Also fix `base_layer_zs` (`:493`), which unconditionally appends a final layer at exactly `height`: with `cellSize: 12, cellsZ: 1, layerHeight: 1.9999` that produces layers **0.6 µm apart**, both extruding a 1.9999 mm bead. Drop or merge the final clamped layer when its gap falls below a small fraction of `layer_height`.

- [ ] **Step 4: Verify green + non-adaptive regression**

Add/confirm a test that a **non-adaptive** job still emits exactly one `Op::Geometry` with `height == layer_height` — the fix must not churn the common path. Four `cargo check`s.

- [ ] **Step 5: Commit** (do NOT regenerate conformance yet — Task 3 does it once)

`fix(core): declare per-layer bead height for adaptive TPMS slices (H1.4, #186)`

---

### Task 3: Align dedupe with the emission grid

**Files:** Modify `crates/core/src/generate/tpms.rs` (`dedupe_consecutive` at `:863`, or `point_key`/`round` at `:25`/`:830`); regenerate `conformance/gallery/gyroid_infill.json`

**Interfaces:** Consumes Tasks 1–2. **Changes emitted op counts for valid input** — this is the task that regenerates the conformance fixture.

The defect: `dedupe_consecutive` drops points closer than `1e-7`, but `move_op` rounds to `1e-6`. Points separated by roughly `(1e-7, 1.4e-6)` survive dedupe and then round to identical coordinates. Measured **at default options**: Schwarz-D `spc=8` → **4** coincident consecutive extruding moves; Neovius `spc=12` → 1; Fischer-Koch-Y `spc=4` and `spc=12` → 1 each. Same root cause as the pocket `I0 J0` arc: `scrub_zero` pins near-zero field values to `±1e-9`, so `interpolate` can place a crossing ~`1e-6·dx` from a grid corner.

- [ ] **Step 1: Write the failing test** — a grid over the surfaces and sample rates the auditor measured

```rust
#[test]
fn no_surface_emits_coincident_extruding_moves() {
    for (surface, spc) in [("schwarz-d", 8), ("neovius", 12), ("fischer-koch-y", 4), ("fischer-koch-y", 12)] {
        let opts = TpmsOptions { surface: surface.parse().unwrap(), samples_per_cell: spc,
                                 cells_x: 1, cells_y: 1, cells_z: 1, ..base_opts() };
        let ops = try_tpms_ops(&opts).expect("valid options");
        let coincident = count_coincident_consecutive_moves(&ops); // compare AFTER emission rounding
        assert_eq!(coincident, 0, "{surface} @ spc={spc} emitted {coincident} coincident moves");
    }
}
```

The comparison must apply the same rounding `move_op` does — comparing raw f64 is what let this through.

- [ ] **Step 2: Run to verify failure** — expect 4/1/1/1 respectively, matching the audit.

- [ ] **Step 3: Implement**

Preferred: dedupe on the **rounded** `point_key`, so dedupe and emission share one quantum by construction and cannot drift apart again. Fallback: raise the threshold to `>= 1.5e-6`. State which you chose and why in the commit body.

> **Premise correction (2026-07-31).** T1/T2's implementer reports `conformance/gallery/gyroid_infill.json`
> is a **FullControl-oracle fixture** (201 ops, `width 0.6`/`height 0.3`, uniform Z), not
> `generate/tpms.rs` output at all — so this task's assumption that it must be regenerated is probably
> wrong. Re-derive whether any fixture actually depends on the generator before touching one; if none
> does, drop the regeneration step rather than performing it to satisfy the plan.

- [ ] **Step 4: Regenerate conformance and quantify the delta**

Regenerate `conformance/gallery/gyroid_infill.json`, then record in the commit message: op count before → after, and confirm the geometry is unchanged apart from removed duplicates (the delta should be exactly the coincident pairs). If the delta is larger than that, STOP — the fix removed real geometry.

- [ ] **Step 5: Commit**

`fix(core): dedupe TPMS points on the emission grid, regen gallery fixture (H1.4, #186)`

---

### Task 4: sample-budget hardening

> **Split (2026-07-31).** Reviewing what actually needs a human decision showed that only the sentinel
> *semantics* do. Everything else here is unambiguous and must not wait on it. Do **T4a** now; leave
> **T4b** until the wire-contract question below is answered.
>
> **T4a — unblocked, do now:** reject `NaN` and negative `maxFieldSamples` outright (nobody intends
> those, and rejecting them closes most of the hostile-JSON hole on its own); fix the adaptive budget
> over-estimate; fix the mutation-loose boundary test; validate `adaptiveMaxDepth`; and correct the
> public field doc, which today says *"Use a large value for trusted offline generation"* while the code
> silently accepts `0` as unlimited — a doc/behaviour mismatch of exactly the D5 class this audit hunts.
> T4a changes no legitimate caller's behaviour and needs no TS change.
>
> **T4b — blocked on a decision:** should `0` — and therefore the TS SDK's `Infinity` — keep meaning
> *unlimited* across an untrusted boundary? Keeping it leaves the DoS guard disableable by anyone who can
> send `0`, which on wasm is any browser page taking user input. Removing it breaks a released TS API.
> Clamping at the binding was considered and rejected: the same wasm path serves both a browser page
> (untrusted) and a Node script doing trusted offline generation, so there is no clean split by surface.
> This is a product question — is unlimited a supported use case over an untrusted boundary — not an
> engineering one. Do not decide it inside the slice.


**Files:** Modify `crates/core/src/generate/tpms.rs` (`assert_budget` at `:632`, layer estimate at `:310`/`:635`, boundary test at `:923`, `adaptive_max_depth` validation at `:285`); `sdk/ts/src/generators/tpms.ts:234`; `docs/07-tpms-codegen.md:70`

**Interfaces:** Changes a wire contract shared between Rust and TypeScript. **This task cannot be completed in Rust alone.**

Two defects, one file:

**(a) The DoS backstop is switchable off from untrusted JSON.** `if !max.is_finite() || max <= 0.0 { return Ok(()) }`. Measured at 8,120,601 field samples against a 6e6 budget: default → correctly rejected; `maxFieldSamples: 0` → **accepted**; `-1` → **accepted**; `NaN` → **accepted**; via raw JSON `{"maxFieldSamples":0}` → **accepted**. `crates/wasm/src/lib.rs:107` and `py/src/lib.rs:99` deserialize caller JSON directly, so this is live in browser/worker/Python contexts. With the budget off, `stride * (ny + 1)` at `:537` can overflow `usize`. The public field doc (`:97`) never says `0` means unlimited — only the private `assert_budget` doc does.
**The complication:** `sdk/ts/src/generators/tpms.ts:234` *deliberately* maps `Infinity → 0` as the unbounded sentinel. `0` is a contract, not an accident. Changing the Rust side alone silently diverges the targets.

**(b) The adaptive estimate bounds the wrong quantity.** `:310` uses `slice_height = layer_height.min(adaptive_min_layer_height)`, charging the job as if *every* interval refined to the floor. Measured: **2001 layers estimated vs 133 actual — a 15× over-estimate** — so `{adaptive: true}` makes a job illegal that is legal without it. `adaptiveMaxDepth` is the real limiter and does not appear in the estimate at all.

- [ ] **Step 1: Decide the sentinel contract, then write failing tests**

Recommended: reject `NaN` and negatives outright; accept a dedicated `None`/`null` (not `0`) as unlimited; keep `0` accepted-as-unlimited for one release **only if** `sdk/ts` cannot be changed in lockstep — and if so, log it as a deprecation in `CHANGELOG.md`. Whichever you choose, Rust and TS must agree in the same commit.

```rust
#[test]
fn budget_sentinel_rejects_nonsense_and_honours_only_the_documented_unlimited_form() {
    let over = over_budget_opts(); // 8_120_601 samples vs the 6e6 default
    assert!(try_tpms_ops(&over).is_err(), "control: the default budget still rejects");
    for bad in [0.0_f64, -1.0, f64::NAN] {
        let o = TpmsOptions { max_field_samples: bad, ..over.clone() };
        assert!(try_tpms_ops(&o).is_err(), "maxFieldSamples {bad} must not disable the guardrail");
    }
}

#[test]
fn adaptive_budget_charges_reachable_refinement_not_the_floor() {
    // Measured: 133 actual layers, 2001 charged under the old estimate.
    let opts = TpmsOptions { cell_size: 10.0, cells_x: 2, cells_y: 2, cells_z: 2,
                             samples_per_cell: 30, layer_height: 0.2,
                             adaptive: true, adaptive_min_layer_height: 0.01, ..base_opts() };
    try_tpms_ops(&opts).expect("a job legal without adaptive must stay legal with it");
}
```

- [ ] **Step 2: Fix the mutation-loose boundary test** (same defect class as the pocket `ceil`/`floor` bug just fixed)

`budget_guardrail_triggers_at_threshold` (`:923`) uses `height = 10, layerHeight = 1.0` — an **integral** ratio, so `ceil(10/1)+1` and `floor(10/1)+1` are both 11 and mutating `libm::ceil` at `:635` survives. Switch to a non-integral ratio (e.g. `layerHeight: 0.75`) and assert on the **computed** value (`msg.contains("1694 field samples")`), mirroring the pocket fix at `pocket.rs:1177`. Verify by applying the `ceil`→`floor` mutant and confirming only this test fails.

- [ ] **Step 3: Implement** — sentinel + estimate (`base_layers × 2^adaptive_max_depth`, capped by `ceil(height/min_layer_height)+1`) + `at_least("adaptiveMaxDepth", v, 0)` with a sane upper cap (16); `adaptive_max_depth` is currently the only integer option with no bound.

- [ ] **Step 4: Four-target sweep** — Rust tests, `sdk/ts` build + its own tests, four `cargo check`s, conformance byte-identical, docs updated at `docs/07-tpms-codegen.md:70` and the public field doc at `tpms.rs:97`.

- [ ] **Step 5: Commit** — `fix(core,sdk): make the TPMS sample budget un-disablable and adaptive-aware (H1.4, #186)`

---

### Task 5: Correct stale claims; establish `proofs/`+`spec/` coverage

**Files:** Modify `crates/core/src/generate/mod.rs:9`, `crates/core/src/generate/tpms.rs:13,16,665`; add TPMS entries to `proofs/claims.toml` and a `TpmsOptions` schema under `spec/`

**Interfaces:** Documentation and formal artifacts only; no behavior change.

- [ ] **Step 1: Fix the stale claims** — `generate/mod.rs:9` and `tpms.rs:16` both say the PyO3/wasm/TS exposures are "deferred follow-ups"; **all three ship** (`py/src/lib.rs:99`, `crates/wasm/src/lib.rs:69,107`, `sdk/ts/src/generators/tpms.ts:11`). `tpms.rs:13` claims an invariant "…verifies, deposits material" that Task 1 proves was false across a documented range — restate it as the property Task 1 now enforces. `adaptiveMaxLayerHeight` (`:665`) is advertised as a bound but is abandoned once `maxDepth` is exhausted; document or enforce it.

- [ ] **Step 2: Establish formal coverage** — `grep -rn -i tpms proofs spec` currently returns **nothing**, despite TPMS being the only generator exposed on all three SDKs. At minimum register: the emission-grid quantum vs dedupe threshold (Task 3's invariant — exactly the kind of numeric boundary `proofs/` exists to pin), the sample-budget bound (Task 4), and a `TpmsOptions` JSON schema mirroring the validation. Follow the existing claim format; mark refinement status honestly.

- [ ] **Step 3: Verify the drift gates** — `python tools/validate_vectors.py conformance/vectors`, the `formal-assurance` CI job's local equivalent, and `cd docs/site && npm run reference` if any manifest source changed.

- [ ] **Step 4: Commit** — `docs(proofs,spec): register TPMS numeric boundaries; correct stale exposure claims (H1.4, #186)`

---

## Self-review notes

- **Audit coverage:** TPMS findings 1→T1, 2→T4a, 3→T3, 4→T4b, 5→T2, 6→T4 Step 2, 7→T1, 8→T5. All eight are assigned.
- **Sequencing rationale:** T1 changes no valid output and can land immediately. T2 and T3 both change valid output; T3 owns the single conformance regeneration so the fixture is not churned twice. T4 is gated on a human decision about the wire sentinel and touches TypeScript, so it is deliberately not first. T5 is documentation and can land any time after T1–T4 settle the facts it documents.
- **Known judgment calls left to the implementer:** whether the vacuous-program check reports `isoLevel` or `minPathLength` in ambiguous cases (T1 Step 3); first-layer bead-height convention (T2 Step 3); dedupe-on-rounded-key vs raised-threshold (T3 Step 3); the sentinel contract itself (T4 Step 1) — that one should be escalated, not decided silently.
- **Tracked as [#186](https://github.com/dmytro-yemelianov/dry/issues/186)**; sibling hardening issues are [#184](https://github.com/dmytro-yemelianov/dry/issues/184) (H1.2 ingress validation) and [#185](https://github.com/dmytro-yemelianov/dry/issues/185) (H1.3 verify strengthening).
