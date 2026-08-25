# KMET Crate Split and Repo Graduation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `crates/core` into four crates along the §1.1 portfolio layers, graduate all four layers into clean private repositories, and freeze the original repository intact as the authorship archive.

**Architecture:** The split happens *inside the current repository* where CI and the drift-gated conformance suite already work; only after each crate is green does it graduate to a clean repo. Two production dependency cycles are broken first (`optimize → verify`, and three `pub(crate)` symbols `verify` reaches into). `dry-core` then becomes a thin re-export facade so all six downstream crates keep compiling unchanged. Graduation runs in reverse topological order — `trace`, `verify`, `kernel`, then layer 4 — because nothing depends on `trace`, so it detaches most cheaply. Graduated code is never deleted from the original repository; it is archived in place, so `dry` ends as a complete frozen snapshot rather than a hollowed-out shell.

**Tech Stack:** Rust 2021, rustc 1.88, Cargo workspace, `serde`/`serde_json`/`miniz_oxide`/`libm`. No new dependencies are introduced by this plan.

**Spec:** `docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md` — §1.1 (six layers), §4 and §5.2 (evidence capture and retaining the original), §5.7 (the split as a filing prerequisite), §6.1 (three registrable works).

## Global Constraints

- **Rust edition 2021, `rust-version = "1.88"`** — inherited via `version.workspace = true` etc. Every new crate inherits from `[workspace.package]`; never hard-code these.
- **`publish = false`** on every crate. Registry publication is disabled by policy (`docs/17` §0).
- **`#![forbid(unsafe_code)]`** at the top of every new crate's `lib.rs`, matching `crates/core/src/lib.rs`.
- **Clippy is `-D warnings` with NO `#[allow]` silencing** — fix structurally. This is the standing discipline from `CONTRIBUTING.md`.
- **The core must keep compiling to `wasm32-unknown-unknown` unmodified.** No crate in this plan may add a dependency that is not wasm-friendly.
- **Zero behaviour change.** This is a move-refactor. Every conformance vector, report golden and CNC/KRL golden must remain byte-identical. If a golden changes, the split is wrong — do not regenerate it.
- **New crates are named `kmet-*` from the outset.** The `dry` → `KMET` product rename (spec §5.6) is gated on trademark clearance and is *not* part of this plan; new crates are simply born with their final names. Residual risk: if clearance kills `KMET`, renaming private crates is a mechanical `sed`.
- **`proofs/` numeric-boundary pins are part of every task's verification.** They are sha256 pins over
  kernel source files, enforced by Python in the separate `formal-assurance` CI job
  (`.github/workflows/ci.yml:277`) — **`cargo test --all` does not reach them.** Five of the six pins in
  the repo cover files this plan moves:

  | Pin file | Pinned source | Broken by |
  |---|---|---|
  | `proofs/emit-numeric-boundaries-v0.toml` | `emit/kinematics.rs` (whole file) | T3, T4 |
  | `proofs/verify-numeric-boundaries-v0.toml` | `verify.rs` (whole file) | T3, T5 |
  | `proofs/feature-numeric-boundaries-v0.toml` | `features.rs` (file) + `resolve.rs` (slice) | T4 |
  | `proofs/resolve-clothoid-numeric-boundaries-v0.toml` | `clothoid.rs` (file) + `resolve.rs` (slice) | T4 |

  - **Editing** a pinned file — even one doc comment — requires re-pinning the sha256 with a comment
    describing the reviewed diff. The convention is at `proofs/verify-numeric-boundaries-v0.toml:10-34`.
    Re-pin only after confirming the change is genuinely contract-neutral; never loosen a predicate or
    bound while re-pinning.
  - **Moving** a pinned file additionally changes its `path` key, which must be updated in the same
    commit. This fails differently and less obviously — a missing-file error rather than a hash
    mismatch. Every `[[source]].path` and `source_path` in `proofs/` is workspace-relative into
    `crates/core/src/...`.
  - The slice pins on `resolve.rs` anchor on literal strings (`pub enum Op {` and
    `\n\n/// Intermediate samples emitted per Catmull-Rom span`) that the validator requires to occur
    **exactly once**. Any edit to `resolve.rs` must preserve that uniqueness.
- **Verification command after every task:**
  ```sh
  cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --all
  for f in proofs/*numeric-boundaries*.toml; do python3 "tools/validate_numeric_boundaries.py" "$f" || exit 1; done
  python3 tools/validate_proof_claims.py && python3 tools/validate_spec_claim_links.py
  ```
- **From Task 4 onward, rebuild the four excluded binding crates** (`crates/wasm`, `crates/cloud`, `py/`,
  `containers/verify-runner`) before claiming done — CLAUDE.md requires it, and from Task 4 modules
  actually move, so the "additive change cannot break a consumer" argument stops holding.

---

## File Structure

**New crates** (all under `crates/`, all workspace members):

| Crate | Layer | Responsibility | Depends on |
|---|---|---|---|
| `kmet-contracts` | 1 | The shared vocabulary: contract structs, `RuleId`, `Severity`, `Kinematics`, tolerance constants. No logic. | — |
| `kmet-kernel` | 1 | resolve, ir, features, emit, gcode, codec, profile, units, frame, clothoid, engine, optimize, generate, sdk | `kmet-contracts` |
| `kmet-verify` | 2 | The rule registry, `verify`, `verify_stream`, `catalog`, and the verify-gated optimize wrapper | `kmet-contracts`, `kmet-kernel` |
| `kmet-trace` | 3 | trace, report, forensics, compare, explain, recommend, reverse | `kmet-contracts`, `kmet-kernel`, `kmet-verify` |
| `dry-core` | — | **Facade only.** Re-exports the four above so `cli`/`wasm`/`py`/`cloud`/`llm`/`verify-runner` compile unchanged. | all four |

**Why `kmet-contracts` exists at all:** `RotaryContracts.model` is typed `crate::emit::Kinematics`, and the kernel reads `RuleId`, `Severity` and `Contracts` from `verify`. Without a crate below both, `kmet-kernel` and `kmet-verify` are mutually dependent and cannot be separate crates.

**Test relocation.** `crates/core/tests/` holds 46 integration tests. They follow their subject; cross-layer tests stay with the facade because that is the only crate that can see the whole stack:

| Destination | Tests |
|---|---|
| `kmet-kernel/tests/` | `arc_fit.rs` `optimize.rs` `optimize_l2.rs` `travel_reorder.rs` `clothoid.rs` `codec_roundtrip.rs` `features.rs` `ir_contracts.rs` `spline.rs` `meta.rs` `channels.rs` `emit_rejects_unrepresentable.rs` `gcode_import_slicer_dialects.rs` `machine_kinematics.rs` `kinematics.rs` `wasm_native_math.rs` `deposition_refinement.rs` `feature_refinement.rs` `orientation.rs` `orientation_refinement.rs` `resolve_channels_refinement.rs` `resolve_orientation_refinement.rs` `simulate_metrics_refinement.rs` `tpms_options_schema.rs` `ingress_validation.rs` |
| `kmet-verify/tests/` | `verify_contracts.rs` `h13_rule_probe.rs` `rewrite_safe_gate.rs` `rewrite_balanced_max_gate.rs` |
| `kmet-trace/tests/` | `trace_analytics.rs` `compare_golden.rs` `report_goldens.rs` |
| `dry-core/tests/` (facade — the drift gates) | `conformance_gcode.rs` `conformance_resolve.rs` `conformance_roundtrip.rs` `conformance_simulate.rs` `spec_vectors.rs` `cnc_pocket_e2e.rs` `cnc_frame_emit.rs` `krl_program_structure.rs` `five_axis.rs` `five_axis_import.rs` `five_axis_singular_cone.rs` `non_planar_e2e.rs` `profile_matrix.rs` `memory_scale.rs` |

---

# Phase A — Break the cycles (still one crate)

## Task 1: Widen kernel visibility across the future crate boundary

`kmet-verify` will live in another crate — and eventually another repository — but reaches three `pub(crate)` symbols today. An integration test compiles as an external crate, so it fails by construction while they are crate-private. That is the failing test.

**Files:**
- Create: `crates/core/tests/crate_boundary.rs`
- Modify: `crates/core/src/emit/kinematics.rs:125` (`RotaryState`)
- Modify: `crates/core/src/engine.rs:56` (`segment_motion_time`)
- Modify: `crates/core/src/optimize/adaptive_speed.rs:34` (`get_tangents`)
- Modify: `crates/core/src/optimize/mod.rs:24` (the `pub(crate) use` re-export)

**Interfaces:**
- Consumes: nothing.
- Produces: `dry_core::emit::RotaryState` (pub struct), `dry_core::engine::segment_motion_time(&Segment) -> Option<Time>` (pub fn), `dry_core::optimize::get_tangents(&Segment) -> Option<([f64; 3], [f64; 3])>` (pub fn). Tasks 4 and 5 rely on all three being `pub`.

- [ ] **Step 1: Write the failing test**

```rust
// crates/core/tests/crate_boundary.rs
//! Symbols `kmet-verify` reaches across the future crate boundary (plan Task 1).
//!
//! An integration test compiles as a separate crate, so anything `pub(crate)` fails to resolve here.
//! That is the point: this file is the compile-time contract that the layer-2 boundary stays open.
//! See docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md §5.7.

use dry_core::emit::RotaryState;
use dry_core::engine::segment_motion_time;
use dry_core::optimize::get_tangents;
use dry_core::{resolve, Design, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn straight_run() -> Design {
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    )
}

#[test]
fn segment_motion_time_is_reachable_from_another_crate() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let moving = tp.segments.iter().find(|s| s.length.value() > 0.0).unwrap();
    assert!(segment_motion_time(moving).is_some());
}

#[test]
fn get_tangents_is_reachable_from_another_crate() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    // A straight line is not an arc, so there are no tangents — `None` is the correct answer.
    // The assertion under test is that the symbol resolves at all.
    let seg = tp.segments.last().unwrap();
    let _ = get_tangents(seg);
}

#[test]
fn rotary_state_is_nameable_from_another_crate() {
    // Naming the type in a signature is enough to prove it is `pub`.
    fn _accepts(_s: &RotaryState) {}
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dry-core --test crate_boundary`
Expected: FAIL to compile — `struct 'RotaryState' is private`, `function 'segment_motion_time' is private`, `function 'get_tangents' is private`.

- [ ] **Step 3: Widen the three symbols**

In `crates/core/src/emit/kinematics.rs:125` change `pub(crate) struct RotaryState {` to `pub struct RotaryState {`.

In `crates/core/src/engine.rs:56` change `pub(crate) fn segment_motion_time(` to `pub fn segment_motion_time(`.

In `crates/core/src/optimize/adaptive_speed.rs:34` change `pub(crate) fn get_tangents(` to `pub fn get_tangents(`.

In `crates/core/src/optimize/mod.rs:24` change `pub(crate) use self::adaptive_speed::get_tangents;` to `pub use self::adaptive_speed::get_tangents;`.

Each of the three needs a doc comment, because `-D warnings` with `missing_docs` in scope will reject a bare `pub` item. Add above each:

```rust
/// Exposed across the crate boundary for `kmet-verify` (plan Task 1); not part of the stable
/// authoring surface.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dry-core --test crate_boundary`
Expected: PASS, 3 tests.

- [ ] **Step 5: Verify nothing else broke**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --all`
Expected: all green, no golden changes.

- [ ] **Step 6: Commit**

```bash
git add crates/core/tests/crate_boundary.rs crates/core/src/emit/kinematics.rs \
        crates/core/src/engine.rs crates/core/src/optimize/adaptive_speed.rs \
        crates/core/src/optimize/mod.rs
git commit -m "refactor(core): widen RotaryState/segment_motion_time/get_tangents for the layer-2 boundary"
```

---

## Task 2: Invert the `optimize → verify` production call

`optimize::apply_gated` calls `verify::verify` in its body (`crates/core/src/optimize/mod.rs:127`). This is the single production logic edge across the future kernel/verify boundary. The fix is a dependency inversion: the kernel keeps the *mechanism* (run the pipeline, diff the error-rule sets, accept or reject), and the caller supplies the *policy* (what counts as an error rule).

**Files:**
- Modify: `crates/core/src/optimize/mod.rs:107-163`
- Test: `crates/core/tests/optimize_gate_inversion.rs` (create)

**Interfaces:**
- Consumes: `GatedResult { toolpath: Toolpath, accepted: bool, new_error_rules: Vec<String> }`, `OptimizeMode`, `MachineKinematics` — all already public.
- Produces:
  ```rust
  pub fn apply_gated_with<F>(
      tp: &Toolpath,
      mode: OptimizeMode,
      kinematics: Option<&MachineKinematics>,
      error_rules: F,
  ) -> GatedResult
  where
      F: Fn(&Toolpath) -> std::collections::BTreeSet<String>;
  ```
  Task 5 moves `apply_gated` and `apply_safe_gated` into `kmet-verify` as wrappers over this.

- [ ] **Step 1: Write the failing test**

```rust
// crates/core/tests/optimize_gate_inversion.rs
//! `apply_gated_with` is the kernel-side gate mechanism: it runs the pipeline and accepts the result
//! only when the caller's policy reports no *new* error rule. Policy lives in `kmet-verify`
//! (plan Task 5); the kernel must not know what a rule is.

use std::collections::BTreeSet;

use dry_core::optimize::apply_gated_with;
use dry_core::{resolve, Design, OptimizeMode, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

fn straight_run() -> Design {
    design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    )
}

#[test]
fn accepts_when_policy_reports_no_new_rules() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    let out = apply_gated_with(&tp, OptimizeMode::Safe, None, |_| BTreeSet::new());
    assert!(out.accepted);
    assert!(out.new_error_rules.is_empty());
}

#[test]
fn rejects_and_returns_input_when_policy_reports_a_new_rule() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    // Policy: the input is clean, anything else has introduced "Bounds".
    let input_len = tp.segments.len();
    let out = apply_gated_with(&tp, OptimizeMode::Safe, None, |candidate| {
        let mut s = BTreeSet::new();
        if candidate.segments.len() != input_len {
            s.insert("Bounds".to_string());
        }
        s
    });
    assert!(!out.accepted);
    assert_eq!(out.new_error_rules, vec!["Bounds".to_string()]);
    // On rejection the input is returned verbatim.
    assert_eq!(out.toolpath.segments.len(), input_len);
}

#[test]
fn preexisting_rules_do_not_block() {
    let tp = resolve(&straight_run(), &ResolveParams::default());
    // Policy reports the same rule for input and candidate — pre-existing, so not "new".
    let out = apply_gated_with(&tp, OptimizeMode::Safe, None, |_| {
        let mut s = BTreeSet::new();
        s.insert("MaxFlow".to_string());
        s
    });
    assert!(out.accepted);
    assert!(out.new_error_rules.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p dry-core --test optimize_gate_inversion`
Expected: FAIL to compile — `cannot find function 'apply_gated_with' in module 'dry_core::optimize'`.

- [ ] **Step 3: Add `apply_gated_with` and make `apply_gated` delegate**

In `crates/core/src/optimize/mod.rs`, replace the body of `apply_gated` (currently lines 121-160) with a delegation, and add the generic above it:

```rust
/// Run the pipeline for `mode` and accept the result only if `error_rules` reports no rule id for the
/// candidate that it did not already report for the input. Pre-existing input errors do not block. On
/// rejection the input is returned verbatim, with the offending rule ids in `new_error_rules`.
///
/// The kernel owns the mechanism; the caller owns the policy. `kmet-verify` supplies the policy that
/// makes this the verification gate (`apply_gated`).
pub fn apply_gated_with<F>(
    tp: &Toolpath,
    mode: OptimizeMode,
    kinematics: Option<&MachineKinematics>,
    error_rules: F,
) -> GatedResult
where
    F: Fn(&Toolpath) -> std::collections::BTreeSet<String>,
{
    let before = error_rules(tp);
    let candidate = match mode {
        OptimizeMode::Safe => safe_pipeline(tp),
        OptimizeMode::Balanced => balanced_pipeline(tp),
        OptimizeMode::Max => match kinematics {
            Some(k) => max_pipeline_with_kinematics(tp, k),
            None => max_pipeline(tp),
        },
    };
    let after = error_rules(&candidate);
    let new_error_rules: Vec<String> = after.difference(&before).cloned().collect();
    if new_error_rules.is_empty() {
        GatedResult { toolpath: candidate, accepted: true, new_error_rules }
    } else {
        GatedResult { toolpath: tp.clone(), accepted: false, new_error_rules }
    }
}

/// The verification-gated pipeline: `apply_gated_with` with `verify` as the policy.
///
/// Moves to `kmet-verify` in plan Task 5; kept here meanwhile so callers are undisturbed.
pub fn apply_gated(
    tp: &Toolpath,
    contracts: &Contracts,
    mode: OptimizeMode,
    kinematics: Option<&MachineKinematics>,
) -> GatedResult {
    use crate::verify::{verify, Severity};
    apply_gated_with(tp, mode, kinematics, |candidate| {
        verify(candidate, contracts)
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.rule.to_string())
            .collect()
    })
}
```

**Before writing this, read the existing `apply_gated` body at `crates/core/src/optimize/mod.rs:121-160`** and mirror its exact pipeline selection and its exact rule-id string derivation. The block above reproduces the documented contract; the existing body is the authority on how `mode` maps to a pipeline and how a `Finding` becomes a rule-id string. Any divergence will show up as a changed report golden in Step 5, which is the signal to go back and match it.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p dry-core --test optimize_gate_inversion`
Expected: PASS, 3 tests.

- [ ] **Step 5: Prove zero behaviour change**

Run: `cargo test --all`
Expected: all green. In particular `rewrite_safe_gate.rs`, `rewrite_balanced_max_gate.rs` and `report_goldens.rs` must pass **without regenerating any golden**. A golden diff here means the delegation is not faithful — fix the delegation, never the golden.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/optimize/mod.rs crates/core/tests/optimize_gate_inversion.rs
git commit -m "refactor(optimize): invert the verify gate into apply_gated_with"
```

---

# Phase B — Create the crates (still one repo)

## Task 3: Extract `kmet-contracts`

**Files:**
- Create: `crates/contracts/Cargo.toml`, `crates/contracts/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members`)
- Modify: `crates/core/Cargo.toml` (add the dependency)
- Modify: `crates/core/src/verify.rs` (remove the moved items, re-export them)
- Modify: `crates/core/src/emit/kinematics.rs` (remove `Kinematics`, re-export)
- Test: `crates/contracts/tests/vocabulary.rs`

**Interfaces:**
- Produces: crate `kmet_contracts` exporting `Contracts`, `KinematicContracts`, `RotaryContracts`, `RotaryTravelRanges`, `ContractParseError`, `parse_bounds_csv`, `parse_speed_range_csv`, `Severity`, `RuleId`, `Kinematics`, `ARC_RADIUS_TOLERANCE_MM`. Tasks 4, 5 and 6 all depend on this crate.

**What moves,** by current location in `crates/core/src/verify.rs`: `Contracts` (line 29), `KinematicContracts` (81), `RotaryContracts` (105), `RotaryTravelRanges` (141), `ContractParseError` (170), `parse_bounds_csv` (216), `parse_speed_range_csv` (226), `Severity` (236), `RuleId` (249), `ARC_RADIUS_TOLERANCE_MM` (616). Plus `Kinematics` from `crates/core/src/emit/kinematics.rs:7`, because `RotaryContracts.model` is typed with it.

**What does NOT move:** `Rule` (308), `catalog` (541), `Finding` (554), `Report` (571), `verify` (1545), `verify_stream` (900). Those are layer 2 and go to `kmet-verify` in Task 5.

- [ ] **Step 1: Write the failing test**

```rust
// crates/contracts/tests/vocabulary.rs
//! `kmet-contracts` is the vocabulary shared by the kernel and the verifier. It must compile with no
//! dependency on either — that is the whole reason it exists (plan Task 3, spec §5.7).

use kmet_contracts::{
    parse_bounds_csv, parse_speed_range_csv, Contracts, Kinematics, RotaryContracts, RuleId,
    Severity, ARC_RADIUS_TOLERANCE_MM,
};

#[test]
fn contracts_default_is_permissive() {
    let c = Contracts::default();
    assert!(c.bounds.is_none());
    assert!(c.max_flow.is_none());
    assert!(!c.monotonic_z);
}

#[test]
fn bounds_csv_round_trips() {
    let b = parse_bounds_csv("0,200,0,200,0,250").unwrap();
    assert_eq!(b[0], [0.0, 200.0]);
    assert_eq!(b[2], [0.0, 250.0]);
}

#[test]
fn speed_range_csv_round_trips() {
    let s = parse_speed_range_csv("300,9000").unwrap();
    assert_eq!(s, [300.0, 9000.0]);
}

#[test]
fn severity_and_rule_id_are_serialisable() {
    assert_eq!(serde_json::to_string(&Severity::Error).unwrap(), "\"error\"");
    let json = serde_json::to_string(&RuleId::Bounds).unwrap();
    assert!(!json.is_empty());
}

#[test]
fn rotary_contracts_carry_a_kinematic_model() {
    let rc = RotaryContracts {
        model: Kinematics::Ac { pivot_offset: [0.0; 3], rotary_offset: [0.0; 2] },
        travel_deg: None,
        max_rotary_feed_deg_min: None,
        envelope_mm: None,
        a: None,
        b: None,
        c: None,
    };
    assert!(rc.travel_deg.is_none());
}

#[test]
fn arc_tolerance_is_exposed() {
    assert_eq!(ARC_RADIUS_TOLERANCE_MM, 1e-6);
}
```

**Note on `Severity::Error` serialising to `"error"`:** confirm the actual `#[serde(rename_all = ...)]` attribute on the enum at `crates/core/src/verify.rs:236` before running, and match the assertion to it.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kmet-contracts`
Expected: FAIL — `error: package ID specification 'kmet-contracts' did not match any packages`.

- [ ] **Step 3: Create the crate**

`crates/contracts/Cargo.toml`:

```toml
[package]
name = "kmet-contracts"
description = "KMET shared vocabulary: verification contracts, rule ids, severities, kinematic models. No logic, no dependencies on the kernel."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
repository.workspace = true
publish = false

[dependencies]
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
serde_json = "1"
```

`crates/contracts/src/lib.rs`:

```rust
//! # kmet-contracts — the shared vocabulary
//!
//! The types the kernel and the verifier both name: verification contracts, rule identifiers,
//! severities, and the kinematic model enum. Deliberately logic-free and deliberately below both, so
//! `kmet-kernel` and `kmet-verify` can be separate crates without a cycle.
//!
//! See `docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md` §5.7.

#![forbid(unsafe_code)]

// Move verbatim from crates/core/src/verify.rs — lines 29, 81, 105, 141, 170, 216, 226, 236, 249, 616
// and from crates/core/src/emit/kinematics.rs line 7. Keep every doc comment and serde attribute.
// `ARC_RADIUS_TOLERANCE_MM` changes from `pub(crate) const` to `pub const`.
```

Move the listed items verbatim, preserving all doc comments and `#[serde(...)]` attributes. Change `ARC_RADIUS_TOLERANCE_MM` from `pub(crate) const` to `pub const`.

Add `"crates/contracts"` to `members` in the root `Cargo.toml`.

- [ ] **Step 4: Re-export from `dry-core` so nothing downstream breaks**

Add to `crates/core/Cargo.toml` under `[dependencies]`:

```toml
kmet-contracts = { path = "../contracts" }
```

At the top of `crates/core/src/verify.rs`, replace the removed definitions with:

```rust
pub use kmet_contracts::{
    parse_bounds_csv, parse_speed_range_csv, ContractParseError, Contracts, KinematicContracts,
    RotaryContracts, RotaryTravelRanges, RuleId, Severity, ARC_RADIUS_TOLERANCE_MM,
};
```

In `crates/core/src/emit/kinematics.rs`, replace the `Kinematics` definition with `pub use kmet_contracts::Kinematics;`.

`crates/core/src/lib.rs` needs no change — its existing `pub use verify::{...}` and `pub use emit::{...}` blocks now re-export the moved types transitively.

- [ ] **Step 5: Run tests**

Run: `cargo test -p kmet-contracts && cargo test --all`
Expected: 6 new tests pass; everything else green with no golden changes.

- [ ] **Step 6: Correct the spec's crate count**

Spec §5.7 tabulates **three** new crates. This task proves a fourth is required: `RotaryContracts.model`
is typed `emit::Kinematics`, so without a crate below both, `kmet-kernel` and `kmet-verify` are mutually
dependent and cannot be separated at all.

In `docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md` §5.7, add a
`kmet-contracts` row to the crate table above `kmet-kernel`:

| New crate | Layer | Drawn from |
|---|---|---|
| `kmet-contracts` | 1 | `Contracts`, `KinematicContracts`, `RotaryContracts`, `RotaryTravelRanges`, `ContractParseError`, `parse_bounds_csv`, `parse_speed_range_csv`, `Severity`, `RuleId`, `ARC_RADIUS_TOLERANCE_MM` (from `verify.rs`); `Kinematics` (from `emit/kinematics.rs`) |

Add one sentence recording why, so the discrepancy reads as a finding rather than drift: the cycle is
real, it was discovered by dependency analysis on 2026-08-25, and the vocabulary crate is what breaks it.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/contracts crates/core/Cargo.toml crates/core/src/verify.rs \
        crates/core/src/emit/kinematics.rs \
        docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md
git commit -m "refactor: extract kmet-contracts, the vocabulary shared by kernel and verify"
```

---

## Task 4: Extract `kmet-kernel`

**Files:**
- Create: `crates/kernel/Cargo.toml`, `crates/kernel/src/lib.rs`
- Move: `crates/core/src/{clothoid.rs,codec/,emit/,engine.rs,features.rs,frame.rs,gcode/,gcode.rs,generate/,ir.rs,optimize/,profile/,resolve.rs,sdk.rs,units.rs}` → `crates/kernel/src/`
- Move: the 25 kernel tests listed in **File Structure** → `crates/kernel/tests/`
- Modify: `Cargo.toml` (workspace `members`), `crates/core/Cargo.toml`, `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `kmet_contracts::{Contracts, KinematicContracts, RotaryContracts, RotaryTravelRanges, RuleId, Severity, Kinematics, ARC_RADIUS_TOLERANCE_MM}` from Task 3.
- Produces: crate `kmet_kernel` with the same public surface these modules have today, including the Task 1 widenings (`emit::RotaryState`, `engine::segment_motion_time`, `optimize::get_tangents`) and the Task 2 addition (`optimize::apply_gated_with`). Tasks 5 and 6 depend on it.

- [ ] **Step 1: Write the failing test**

```rust
// crates/kernel/tests/kernel_surface.rs
//! The kernel stands alone: resolve → simulate → emit, with no verifier and no analysis layer.
//! If this compiles, layer 1 is genuinely separable (plan Task 4, spec §1.1).

use kmet_kernel::{emit, resolve, simulate, Design, EmitParams, ResolveParams};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn resolve_simulate_emit_without_verify_or_trace() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    assert!(!tp.segments.is_empty());

    let m = simulate(&tp);
    assert!(m.total_time_s > 0.0);

    #[allow(deprecated)]
    let g = emit(&tp, &EmitParams::default());
    assert!(g.contains("G1"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kmet-kernel`
Expected: FAIL — package not found.

- [ ] **Step 3: Create the crate and move the modules**

`crates/kernel/Cargo.toml`:

```toml
[package]
name = "kmet-kernel"
description = "KMET layer 1: the IR, resolve, lowering, optimisation, generation and emission. Dependency-light; compiles to wasm32-unknown-unknown unmodified."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
repository.workspace = true
publish = false

[dependencies]
kmet-contracts = { path = "../contracts" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["float_roundtrip"] }
miniz_oxide = "0.8"
libm = "0.2"

[dev-dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
num-bigint = "0.4"
num-traits = "0.2"
```

Copy the `serde_json` `float_roundtrip` comment from `crates/core/Cargo.toml` verbatim — it documents a conformance requirement, not a preference.

Move the module files with `git mv` so history follows them:

```bash
mkdir -p crates/kernel/src crates/kernel/tests
git mv crates/core/src/clothoid.rs crates/core/src/engine.rs crates/core/src/features.rs \
       crates/core/src/frame.rs crates/core/src/gcode.rs crates/core/src/ir.rs \
       crates/core/src/resolve.rs crates/core/src/sdk.rs crates/core/src/units.rs \
       crates/kernel/src/
git mv crates/core/src/codec crates/core/src/emit crates/core/src/gcode \
       crates/core/src/generate crates/core/src/optimize crates/core/src/profile \
       crates/kernel/src/
```

`crates/kernel/src/lib.rs` takes the module declarations and `pub use` blocks for exactly these modules, lifted from `crates/core/src/lib.rs`. Add `#![forbid(unsafe_code)]`.

Within the moved files, rewrite `crate::verify::X` → `kmet_contracts::X` for the moved vocabulary. The affected sites are known: `resolve.rs:17`, `optimize/mod.rs:22`, `profile/mod.rs:9`, `profile/mod.rs:410`, `profile/mod.rs:688-690`, and the `#[cfg(test)]` blocks in `generate/tpms.rs:1240` and `generate/pocket.rs:1085-1094`.

The two `#[cfg(test)]` blocks call `verify::verify`, which is layer 2 and is **not** available here. Move `generate/tpms.rs`'s and `generate/pocket.rs`'s verify-dependent test modules into `crates/core/tests/` as facade-level integration tests, since the facade can see both layers. Name them `generate_tpms_verified.rs` and `generate_pocket_verified.rs`.

Remove `pub fn apply_gated` and `pub fn apply_safe_gated` from `crates/kernel/src/optimize/mod.rs` —
they need the verifier, which the kernel must not depend on. `apply_gated_with` stays.

**They need an interim home, or this task does not build.** `crates/core/src/lib.rs` re-exports both
names, so deleting them without relocation breaks Step 6's `cargo test --all`. Move their bodies —
**cut and paste from `optimize/mod.rs`, do not retype from this document** — into a new
`crates/core/src/gated.rs`, adjusting only the crate paths (`crate::verify` stays; `crate::optimize::…`
becomes `kmet_kernel::optimize::…`). `dry-core` still owns `verify.rs` at this point, so it can host
them. Task 5 relocates them to `kmet-verify` and deletes the file.

Declare `mod gated;` in `crates/core/src/lib.rs` and re-export `apply_gated` / `apply_safe_gated` from
it instead of from `optimize`.

Add `"crates/kernel"` to workspace `members`.

- [ ] **Step 4: Move the kernel tests**

```bash
git mv crates/core/tests/{arc_fit,optimize,optimize_l2,travel_reorder,clothoid,codec_roundtrip,features,ir_contracts,spline,meta,channels,emit_rejects_unrepresentable,gcode_import_slicer_dialects,machine_kinematics,kinematics,wasm_native_math,deposition_refinement,feature_refinement,orientation,orientation_refinement,resolve_channels_refinement,resolve_orientation_refinement,simulate_metrics_refinement,tpms_options_schema,ingress_validation}.rs crates/kernel/tests/
```

In each moved test, rewrite `use dry_core::` → `use kmet_kernel::`. Any test that reaches a verify or trace symbol does **not** belong here — move it to `crates/core/tests/` instead and leave it importing `dry_core::`.

- [ ] **Step 5: Point `dry-core` at the kernel**

In `crates/core/Cargo.toml` add `kmet-kernel = { path = "../kernel" }`. In `crates/core/src/lib.rs` replace the moved `pub mod` declarations and their `pub use` blocks with:

```rust
pub use kmet_kernel::{clothoid, codec, emit, engine, features, frame, gcode, generate, ir, optimize, profile, resolve, sdk, units};
```

followed by the identical flat `pub use kmet_kernel::{...}` re-export list that `lib.rs` exposes today for those modules, so every downstream `use dry_core::Toolpath` style import keeps resolving.

- [ ] **Step 6: Run the full suite**

Run: `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --all`
Expected: green, **no golden regeneration**. `spec_vectors.rs`, `report_goldens.rs`, `cnc_pocket_e2e.rs` and `krl_program_structure.rs` are the drift gates — they prove the move changed no bytes.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: extract kmet-kernel (layer 1); dry-core becomes a partial facade"
```

---

## Task 5: Extract `kmet-verify`

**Files:**
- Create: `crates/verify/Cargo.toml`, `crates/verify/src/lib.rs`
- Move: `crates/core/src/verify.rs` → `crates/verify/src/lib.rs`
- Move: `crates/core/tests/{verify_contracts,h13_rule_probe,rewrite_safe_gate,rewrite_balanced_max_gate}.rs` → `crates/verify/tests/`
- Modify: `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `kmet_contracts::{Contracts, RuleId, Severity, ARC_RADIUS_TOLERANCE_MM}`; `kmet_kernel::{emit::RotaryState, engine::segment_motion_time, optimize::{get_tangents, apply_gated_with, OptimizeMode, GatedResult}, resolve::{catmull_rom, SAMPLES}, ir::{Segment, SegmentKind, Toolpath}, units::Length, profile::MachineKinematics}`.
- Produces: crate `kmet_verify` exporting `verify`, `verify_stream`, `catalog`, `Rule`, `Finding`, `Report`, `apply_gated`, `apply_safe_gated`. Task 6 depends on it.

- [ ] **Step 1: Write the failing test**

```rust
// crates/verify/tests/gate_uses_kernel_mechanism.rs
//! `apply_gated` is the verification policy bound to the kernel's gate mechanism
//! (`kmet_kernel::optimize::apply_gated_with`, plan Task 2). This proves the two halves rejoin
//! correctly after the split.

use kmet_contracts::Contracts;
use kmet_kernel::{resolve, Design, OptimizeMode, ResolveParams};
use kmet_verify::apply_gated;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn clean_toolpath_passes_the_safe_gate() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":5,"y":0,"z":0.2},
            {"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let out = apply_gated(&tp, &Contracts::default(), OptimizeMode::Safe, None);
    assert!(out.accepted);
    assert!(out.new_error_rules.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kmet-verify`
Expected: FAIL — package not found.

- [ ] **Step 3: Create the crate**

`crates/verify/Cargo.toml`:

```toml
[package]
name = "kmet-verify"
description = "KMET layer 2: the verification rule registry and the verify-gated rewrite. Machine-checked correctness claims live alongside in proofs/ and formal/."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
repository.workspace = true
publish = false

[dependencies]
kmet-contracts = { path = "../contracts" }
kmet-kernel = { path = "../kernel" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["float_roundtrip"] }

[dev-dependencies]
serde_json = "1"
```

```bash
mkdir -p crates/verify/src crates/verify/tests
git mv crates/core/src/verify.rs crates/verify/src/lib.rs
git mv crates/core/tests/{verify_contracts,h13_rule_probe,rewrite_safe_gate,rewrite_balanced_max_gate}.rs crates/verify/tests/
```

In `crates/verify/src/lib.rs`: add the crate doc comment and `#![forbid(unsafe_code)]`; rewrite the imports at lines 13-18 from `crate::` to `kmet_kernel::`; add `use kmet_contracts::{...}` for the vocabulary; keep the `pub use kmet_contracts::{...}` re-export block added in Task 3 so `kmet_verify::Contracts` still resolves for callers.

Relocate the two wrappers from `crates/core/src/gated.rs` (where Task 4 parked them):

```bash
git mv crates/core/src/gated.rs crates/verify/src/gated.rs
```

**Move the bodies verbatim — never retype them from this document.** Adjust only the crate paths:
`crate::verify::` becomes local, `kmet_kernel::optimize::` and `kmet_contracts::` as appropriate. Declare
`mod gated;` in `crates/verify/src/lib.rs` and `pub use gated::{apply_gated, apply_safe_gated};`.

> **Why verbatim matters here.** An earlier draft of this plan contained an illustrative version of
> `apply_gated` that was not merely non-compiling but *behaviourally wrong*: it routed `kinematics` into
> `Max`, whereas the real `pipeline_for` routes kinematics into `Balanced` and `Max` ignores them
> entirely. Retyping from prose is how a semantics-preserving refactor stops preserving semantics.

Add `"crates/verify"` to workspace `members`. In `crates/core/Cargo.toml` add `kmet-verify = { path = "../verify" }`; in `crates/core/src/lib.rs` replace `pub mod verify;` with `pub use kmet_verify as verify;` and keep the existing flat `pub use verify::{...}` list, adding `apply_gated` and `apply_safe_gated` to it (they previously came from `optimize`).

- [ ] **Step 4: Run tests**

Run: `cargo test -p kmet-verify && cargo test --all`
Expected: green, no golden changes.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: extract kmet-verify (layer 2) with the gate policy rejoined"
```

---

## Task 6: Extract `kmet-trace`

**Files:**
- Create: `crates/trace/Cargo.toml`, `crates/trace/src/lib.rs`
- Move: `crates/core/src/{trace.rs,report.rs,forensics.rs,compare.rs,explain.rs,recommend.rs,reverse.rs}` → `crates/trace/src/`
- Move: `crates/core/tests/{trace_analytics,compare_golden,report_goldens}.rs` → `crates/trace/tests/`
- Modify: `Cargo.toml`, `crates/core/Cargo.toml`, `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `kmet_contracts`, `kmet_kernel`, `kmet_verify` (`recommend.rs:119` calls `parse_speed_range_csv`; `report.rs` renders `Finding`).
- Produces: crate `kmet_trace` exporting the module set currently re-exported from `dry_core` for `trace`, `report`, `forensics`, `compare`, `explain`, `recommend`, `reverse`.

**Note on the layering correction:** the spec's §1.1 placed `report.rs` in layer 2. It imports `trace::TraceSummary` (`report.rs:10`), so it belongs in layer 3. Update §1.1 and §5.7 of the spec in Step 5 rather than leaving the document contradicting the code.

- [ ] **Step 1: Write the failing test**

```rust
// crates/trace/tests/analysis_surface.rs
//! Layer 3 stands on the kernel and the verifier and is depended on by nothing — which is why it
//! graduates to its own repository first (plan Task 8).

use kmet_kernel::{resolve, Design, ResolveParams};
use kmet_trace::trace_summary;

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn trace_summary_runs_over_a_resolved_toolpath() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp = resolve(&d, &ResolveParams::default());
    let s = trace_summary(&tp, 1.0).unwrap();
    assert_eq!(s.window_s, 1.0);
    assert!(s.total_time_s > 0.0);
    assert!(s.segment_count > 0);
}
```

Confirm `trace_summary`'s exact signature at `crates/core/src/trace.rs` before running; the plan assumes `trace_summary(&Toolpath, f64) -> Result<TraceSummary, TraceError>`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p kmet-trace`
Expected: FAIL — package not found.

- [ ] **Step 3: Create the crate**

`crates/trace/Cargo.toml`:

```toml
[package]
name = "kmet-trace"
description = "KMET layer 3: trace analytics, review reports, forensics, comparison, explanation, recommendation and reverse inference."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license-file.workspace = true
repository.workspace = true
publish = false

[dependencies]
kmet-contracts = { path = "../contracts" }
kmet-kernel = { path = "../kernel" }
kmet-verify = { path = "../verify" }
serde = { version = "1", features = ["derive"] }
serde_json = { version = "1", features = ["float_roundtrip"] }

[dev-dependencies]
serde_json = "1"
```

```bash
mkdir -p crates/trace/src crates/trace/tests
git mv crates/core/src/{trace.rs,report.rs,forensics.rs,compare.rs,explain.rs,recommend.rs,reverse.rs} crates/trace/src/
git mv crates/core/tests/{trace_analytics,compare_golden,report_goldens}.rs crates/trace/tests/
```

Write `crates/trace/src/lib.rs` with `#![forbid(unsafe_code)]`, the seven `pub mod` declarations, and the `pub use` blocks for `trace`, `report`, `forensics`, `compare`, `explain`, `recommend`, `reverse` lifted verbatim from `crates/core/src/lib.rs`. Rewrite `crate::` imports in the moved files to `kmet_kernel::` / `kmet_verify::` / `kmet_contracts::` as appropriate.

Add `"crates/trace"` to workspace `members`; add `kmet-trace = { path = "../trace" }` to `crates/core/Cargo.toml`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p kmet-trace && cargo test --all`
Expected: green. `report_goldens.rs` must pass without regeneration.

- [ ] **Step 5: Correct the spec's layering**

In `docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md`, move `report.rs` from the layer-2 row to the layer-3 row in the §1.1 table, and from the `kmet-verify` row to the `kmet-trace` row in the §5.7 table. Add one sentence recording why: `report.rs` imports `trace::TraceSummary`, so it sits above trace.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: extract kmet-trace (layer 3); correct report.rs layering in the spec"
```

---

## Task 7: Reduce `dry-core` to a pure facade

**Files:**
- Modify: `crates/core/src/lib.rs` (should now contain only re-exports)
- Modify: `crates/core/Cargo.toml` (drop dependencies the facade no longer uses directly)
- Test: `crates/core/tests/facade_surface.rs`

**Interfaces:**
- Produces: `dry_core` re-exporting the union of `kmet_contracts`, `kmet_kernel`, `kmet_verify`, `kmet_trace`. The six downstream crates (`crates/cli`, `crates/llm`, `crates/wasm`, `crates/cloud`, `py/`, `containers/verify-runner`) must compile with **no changes**.

- [ ] **Step 1: Write the failing test**

```rust
// crates/core/tests/facade_surface.rs
//! `dry-core` is now a facade. This test names one symbol from each of the four crates through it —
//! if all four resolve, no downstream crate needs to change (plan Task 7).

use dry_core::{
    forensics_analyze, resolve, simulate, trace_summary, verify, Contracts, Design, ResolveParams,
    Toolpath,
};

fn design(ops: &str) -> Design {
    serde_json::from_str(&format!("{{\"ops\":{ops}}}")).unwrap()
}

#[test]
fn all_four_layers_are_reachable_through_the_facade() {
    let d = design(
        r#"[{"op":"geometry","width":0.6,"height":0.2},{"op":"extruder","on":true},
            {"op":"move","x":0,"y":0,"z":0.2},{"op":"move","x":10,"y":0,"z":0.2}]"#,
    );
    let tp: Toolpath = resolve(&d, &ResolveParams::default()); // kernel
    let _ = simulate(&tp); // kernel
    let _ = verify(&tp, &Contracts::default()); // verify + contracts
    let _ = trace_summary(&tp, 1.0); // trace
    let _ = forensics_analyze; // trace
}
```

- [ ] **Step 2: Run test to verify it fails or passes**

Run: `cargo test -p dry-core --test facade_surface`
Expected: PASS if Tasks 3-6 re-exported completely; any FAIL names exactly the missing re-export. Add it and re-run.

- [ ] **Step 3: Strip the facade's direct dependencies**

`crates/core/src/lib.rs` should now contain only the module doc comment and `pub use` statements. `crates/core/Cargo.toml` keeps only the four `kmet-*` path dependencies plus whatever the remaining facade tests need as dev-dependencies (`serde_json`, `sha2`, `criterion`, `num-bigint`, `num-traits`). Remove `serde`, `miniz_oxide`, `libm` from `[dependencies]` — the facade does not use them directly.

Move `[[bench]] engine_codec` to `crates/kernel/Cargo.toml` along with `benches/`, since it benchmarks kernel code.

- [ ] **Step 4: Verify every downstream crate still builds untouched**

```bash
cargo test --all
cd crates/wasm && cargo check && cd ../..
cd crates/cloud && cargo check && cd ../..
cd containers/verify-runner && cargo check && cd ../../..
cd py && cargo check && cd ..
```
Expected: all green with **zero edits** to those crates. Any edit needed there means a re-export is missing — fix the facade, not the consumer.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: reduce dry-core to a pure re-export facade over the four kmet crates"
```

---

# Phase C — Graduate to clean repositories

**Prerequisite, non-negotiable:** spec §4 Phase 1 (evidence capture) must be **complete and verified**
before the first repository is created. Clean repositories carry no history, so the 460-commit
authorship record survives only in the evidence bundle and in the retained `dry` repository. Do not
start Task 8 until every `MANIFEST.json` hash has been re-derived from stored payloads on a second
machine.

## The end state

**Four clean private repositories, and `dry` frozen intact as the archive.**

| Repository | Layer | Contents | History |
|---|---|---|---|
| `kmet-kernel` | 0 + 1 | `contracts/`, `kernel/`, `spec/` schemas, `conformance/vectors/` | clean |
| `kmet-verify` | 2 | `verify/`, `proofs/`, `formal/`, assurance tooling | clean |
| `kmet-trace` | 3 | `trace/`, `conformance/reports/`, `tools/validate_reports.py` | clean |
| `kmet-tools` | 4 + X | CLI, bindings, SDKs, `web/`, `services/`, the `dry-core` facade, the cross-layer drift gates, and the oracle-derived corpora | clean |
| `dry` | archive | **Everything, untouched, frozen.** Full 460-commit history. Private forever. | full |

**Nothing is deleted from `dry`.** Graduated code is *archived in place*: removed from the Cargo
workspace `members` so it stops building, moved under `archive/`, and left there. The final state of
`dry` is a complete, coherent snapshot of the project as it stood at the cutover — which is far better
evidence than a hollowed-out shell whose last four commits are deletions.

**Why the encumbered material goes to `kmet-tools` and nowhere else.** `conformance/{gcode,gallery,golden,profiles,roundtrip,simulate}/` are FullControl-oracle output (layer X, spec §1.1) and the
cross-layer drift gates test against them. They must not enter a registrable work's repository — that is
the entire point of the quarantine. `kmet-tools` is where the whole stack is assembled anyway, so the
facade, the drift gates and the corpora belong together there, leaving layers 1-3 clean of encumbered
material. When the oracle is retired (spec §5.5) and KMET's own outputs become the reference, this
constraint dissolves.

`conformance/oracle/` itself (the GPLv3 generator) graduates **nowhere**. It stays in the `dry` archive
only.

## Placement of the non-crate directories

| Source | Destination | Why |
|---|---|---|
| `spec/`, `conformance/vectors/`, `tools/validate_vectors.py` | `kmet-kernel` | Layer 0 — the published IR contract is the kernel's public face |
| `proofs/`, `formal/`, `tools/{validate_proof_claims,check_proof_fixtures,generate_assurance_report,validate_numeric_boundaries,check_feature_mutations}.py` | `kmet-verify` | Layer 2 — the assurance work product |
| `conformance/reports/`, `tools/validate_reports.py` | `kmet-trace` | Layer 3 — report goldens follow the report code |
| `crates/{cli,llm,moonraker,license}`, `crates/{wasm,cloud}`, `py/`, `sdk/ts`, `containers/verify-runner`, `web/`, `services/`, `examples/`, `docs/site/`, `tools/slicer_corpus/`, `crates/core` (facade) | `kmet-tools` | Layer 4 |
| `conformance/{gcode,gallery,golden,profiles,roundtrip,simulate}/`, `conformance/slicer-corpus/` | `kmet-tools` | Layer X — quarantined out of layers 1-3 |
| `conformance/oracle/`, `tools/license-issuer/`, `docs/superpowers/`, `docs/marketing/`, `docs/adr/`, `docs/0*.md`–`2*.md` | **`dry` archive only** | GPLv3 generator, layer 5, and the internal record |

**Layer 5 is deliberately left in the archive.** `tools/license-issuer/` and the `prod-1` key material
are the highest-value secret in the portfolio (spec §1.1) and putting them in `kmet-tools` — the widest-access repository — would defeat the compartmentalisation that motivated separate repositories at
all. Giving them a fifth private repository is the likely answer but is a decision this plan does not
make; see **Open decisions**.

## Cross-repo dependencies

Git+SSH against private repositories, pinned by tag:

```toml
kmet-kernel = { git = "ssh://git@github.com/<org>/kmet-kernel.git", tag = "v0.7.0" }
```

Pin by `tag`, never `branch` — an unpinned git dependency makes builds unreproducible, which would
undermine the release attestation in `release.yml`.

## The archival procedure, used by Tasks 8-11

Every graduation ends with the same three moves in `dry`. They are written once here; each task
references them rather than repeating them.

```bash
# 1. Stop it building: remove the member line from the root Cargo.toml `members` array.
# 2. Archive in place, preserving history via git mv:
mkdir -p archive/crates
git mv crates/<name> archive/crates/<name>
# 3. Record what happened, next to the archived code:
cat > archive/crates/<name>/ARCHIVED.md <<'EOF'
# Archived — graduated to its own repository

This code was extracted to `<org>/kmet-<name>` at tag `vX.Y.Z` on <date>.

It is retained here unmodified as part of the frozen authorship record (see
`docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md` §4, §5.2).
It is excluded from the Cargo workspace and is not built, tested, or shipped.

Successor repository: ssh://git@github.com/<org>/kmet-<name>.git
Extraction commit in this repository: <sha>
EOF
```

`<sha>` is the exact commit the extraction was taken from. It is the only link between a clean
repository and the authorship record, and §7.1's `ip/ledger.toml` references it.

---

## Task 8: Create the repositories and graduate `kmet-trace`

**Files:**
- Create: private repositories `kmet-kernel`, `kmet-verify`, `kmet-trace`, `kmet-tools`
- Create: `kmet-trace` repo contents from `crates/trace/`, `conformance/reports/`, `tools/validate_reports.py`
- Modify: root `Cargo.toml` (drop `crates/trace` from `members`), `crates/core/Cargo.toml` (path dep → git dep)
- Move: `crates/trace/` → `archive/crates/trace/`

- [ ] **Step 1: Create four empty private repositories**

```bash
gh repo create <org>/kmet-kernel --private --description "KMET layer 1 — the toolpath compiler kernel and the IR contract"
gh repo create <org>/kmet-verify --private --description "KMET layer 2 — verification, proofs and assurance"
gh repo create <org>/kmet-trace  --private --description "KMET layer 3 — trace analytics, review reports and forensics"
gh repo create <org>/kmet-tools  --private --description "KMET layer 4 — CLI, SDKs, bindings and the web surface"
```

`<org>` is the organisation chosen in spec §5.1 — `github.com/kmet` is taken, so this is the alternative
recorded there. All four are private and none may ever be made public (spec §0, visibility decision).

- [ ] **Step 2: Seed `kmet-trace` with a single clean initial commit**

```bash
cd /tmp && git clone ssh://git@github.com/<org>/kmet-trace.git && cd kmet-trace
mkdir -p conformance tools
cp -R <repo>/crates/trace/. .
cp -R <repo>/conformance/reports conformance/
cp <repo>/tools/validate_reports.py tools/
cp <repo>/LICENSE <repo>/NOTICE .
git add -A
git commit -m "Initial commit: KMET layer 3 — trace analytics, reports, forensics

Extracted from the KMET monorepo at <sha>. Authorship history for this code is preserved in the
pre-cutover evidence bundle (spec §4) and in the retained \`dry\` archive repository."
git push
```

- [ ] **Step 3: Add CI**

Copy the `core` job from `.github/workflows/ci.yml` into `kmet-trace/.github/workflows/ci.yml`, reduced
to: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and
`python tools/validate_reports.py .`. It must resolve sibling private git dependencies — add a deploy
key, or check out with a token that can read them.

- [ ] **Step 4: Tag**

```bash
git tag v0.7.0 && git push --tags
```

- [ ] **Step 5: Switch the monorepo to the git dependency and archive**

In `crates/core/Cargo.toml` replace `kmet-trace = { path = "../trace" }` with the pinned git
dependency. Then apply **The archival procedure** above with `<name>` = `trace`. Archive
`conformance/reports/` and `tools/validate_reports.py` together (`git mv conformance/reports
archive/conformance/ && git mv tools/validate_reports.py archive/tools/`) — the goldens and their
validator must move as a pair, or `dry`'s CI validates an archived directory. Delete the
`python tools/validate_reports.py .` step from `.github/workflows/ci.yml` in the same commit; that
check now runs in the `kmet-trace` repository.

- [ ] **Step 6: Verify**

Run: `cargo test --all`
Expected: green, resolving `kmet-trace` over SSH. If the fetch fails, fix SSH access now — the same
mechanism carries Tasks 9-11.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "build: graduate kmet-trace to its own repository; archive it here"
```

---

## Task 9: Graduate `kmet-verify`

**Files:**
- Create: `kmet-verify` repo from `crates/verify/`, `proofs/`, `formal/`, five assurance tools
- Modify: root `Cargo.toml`, `crates/core/Cargo.toml`
- Move: `crates/verify/` → `archive/crates/verify/`; `proofs/`, `formal/` → `archive/`

- [ ] **Step 1: Record the temporary standalone breakage**

Between Tasks 9 and 10 the `kmet-verify` repository cannot build alone: it depends on `kmet-contracts`
and `kmet-kernel`, which are still path deps in the monorepo. Put this in its `README.md` rather than
leaving an unexplained red badge:

```markdown
> **Status:** extraction in progress. This crate depends on `kmet-contracts` and `kmet-kernel`, which
> graduate in the next step. Standalone CI is expected to fail until then.
```

- [ ] **Step 2: Seed the repository**

```bash
cd /tmp && git clone ssh://git@github.com/<org>/kmet-verify.git && cd kmet-verify
mkdir -p tools
cp -R <repo>/crates/verify/. .
cp -R <repo>/proofs <repo>/formal .
cp <repo>/tools/{validate_proof_claims.py,check_proof_fixtures.py,generate_assurance_report.py,validate_numeric_boundaries.py,check_feature_mutations.py} tools/
cp -R <repo>/tools/tests tools/
cp <repo>/LICENSE <repo>/NOTICE .
git add -A
git commit -m "Initial commit: KMET layer 2 — verification, proofs and assurance

Extracted from the KMET monorepo at <sha>. Authorship history for this code is preserved in the
pre-cutover evidence bundle (spec §4) and in the retained \`dry\` archive repository."
git push
```

- [ ] **Step 3: Add CI**

Reduce the `core` job as in Task 8, plus the `formal-assurance` job from `.github/workflows/ci.yml`
(Lean 4 / lake), plus `python tools/validate_proof_claims.py`.

- [ ] **Step 4: Tag, switch the dependency, archive**

Tag `v0.7.0`. In `crates/core/Cargo.toml` swap the path dep for the git dep. Apply **The archival
procedure** with `<name>` = `verify`, and archive `proofs/` and `formal/` alongside it.

- [ ] **Step 5: Verify**

Run: `cargo test --all`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "build: graduate kmet-verify to its own repository; archive it here"
```

---

## Task 10: Graduate `kmet-kernel` and `kmet-contracts`

`kmet-contracts` ships inside the `kmet-kernel` repository as a second workspace member. It is layer 1's
vocabulary and a few hundred lines; a separate repository would be overhead with no compartmentalisation
benefit. The IR contract (layer 0) ships here too, because the published spec and vectors are the
kernel's public face.

**Files:**
- Create: `kmet-kernel` repo — a two-member workspace (`contracts/`, `kernel/`) plus `spec/`, `conformance/vectors/`, `tools/validate_vectors.py`
- Modify: root `Cargo.toml`, `crates/core/Cargo.toml`; and in the `kmet-verify` and `kmet-trace` repos, path deps → git deps
- Move: `crates/contracts/`, `crates/kernel/`, `spec/`, `conformance/vectors/` → `archive/`

- [ ] **Step 1: Seed the two-member workspace**

```bash
cd /tmp && git clone ssh://git@github.com/<org>/kmet-kernel.git && cd kmet-kernel
mkdir -p contracts kernel conformance tools
cp -R <repo>/crates/contracts/. contracts/
cp -R <repo>/crates/kernel/. kernel/
cp -R <repo>/spec .
cp -R <repo>/conformance/vectors conformance/
cp <repo>/tools/validate_vectors.py tools/
cp <repo>/LICENSE <repo>/NOTICE .
```

Write a root `Cargo.toml` with `members = ["contracts", "kernel"]` and a `[workspace.package]` block
copied verbatim from `<repo>/Cargo.toml` — `version = "0.7.0"`, `edition = "2021"`,
`rust-version = "1.88"`, `license-file = "LICENSE"`, and `repository` pointing at the new URL.

```bash
git add -A
git commit -m "Initial commit: KMET layer 1 — kernel, contracts and the IR contract

Extracted from the KMET monorepo at <sha>. Authorship history for this code is preserved in the
pre-cutover evidence bundle (spec §4) and in the retained \`dry\` archive repository."
git push && git tag v0.7.0 && git push --tags
```

- [ ] **Step 2: Add CI**

The `core` job reduced to this workspace, plus `python tools/validate_vectors.py conformance/vectors`,
plus the `wasm32-unknown-unknown` build check — the kernel's wasm-cleanliness is a Global Constraint and
this is the repository that must enforce it.

- [ ] **Step 3: Repoint the sibling repositories**

In `kmet-verify` and `kmet-trace`, replace the `kmet-contracts` / `kmet-kernel` path deps with the
pinned git deps and push. `kmet-verify`'s CI should now be green — delete the temporary README notice
from Task 9 Step 1.

- [ ] **Step 4: Switch the monorepo and archive**

`crates/core/Cargo.toml` now points at all four git dependencies. Apply **The archival procedure** for
`contracts` and `kernel`, and archive `spec/` and `conformance/vectors/` alongside.

- [ ] **Step 5: Verify the whole stack across four repositories**

```bash
cargo test --all
cd crates/wasm && cargo check && cd ../..
cd crates/cloud && cargo check && cd ../..
cd containers/verify-runner && cargo check && cd ../../..
cd py && cargo check && cd ..
```
Expected: all green. The cross-layer drift gates still living in `crates/core/tests/` passing against
four separately-fetched repositories is the proof that the split changed no behaviour.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "build: graduate kmet-kernel + kmet-contracts; archive them here"
```

---

## Task 11: Graduate layer 4 to `kmet-tools`

Everything that remains buildable in `dry` moves out together: it is one layer, it releases as one unit,
and splitting it further would multiply CI for code that is thin by design (spec §1.1, layer 4).

**Files:**
- Create: `kmet-tools` repo from `crates/{cli,llm,moonraker,license,wasm,cloud,core}`, `py/`, `sdk/ts`, `containers/verify-runner`, `web/`, `services/`, `examples/`, `docs/site/`, `tools/slicer_corpus/`, and the layer-X conformance corpora
- Move: all of the above → `archive/` in `dry`

- [ ] **Step 1: Seed the repository**

```bash
cd /tmp && git clone ssh://git@github.com/<org>/kmet-tools.git && cd kmet-tools
mkdir -p crates conformance tools docs
cp -R <repo>/crates/{cli,llm,moonraker,license,wasm,cloud,core} crates/
cp -R <repo>/{py,sdk,containers,web,services,examples} .
cp -R <repo>/docs/site docs/
cp -R <repo>/tools/slicer_corpus tools/
cp -R <repo>/conformance/{gcode,gallery,golden,profiles,roundtrip,simulate,slicer-corpus} conformance/
cp <repo>/LICENSE <repo>/NOTICE .
cp -R <repo>/third_party .
```

Rename `crates/core` to `crates/facade` and set its package `name = "kmet"` — layer 4 is where the
single umbrella crate belongs, and `dry-core` was only ever a transitional name. Update the six
dependents' `Cargo.toml` accordingly, and their `use dry_core::` imports to `use kmet::`.

Move the cross-layer drift gates in with it: `conformance_gcode.rs`, `conformance_resolve.rs`,
`conformance_roundtrip.rs`, `conformance_simulate.rs`, `spec_vectors.rs`, `cnc_pocket_e2e.rs`,
`cnc_frame_emit.rs`, `krl_program_structure.rs`, `five_axis*.rs`, `non_planar_e2e.rs`,
`profile_matrix.rs`, `memory_scale.rs`, `facade_surface.rs`, `generate_tpms_verified.rs`,
`generate_pocket_verified.rs` → `crates/facade/tests/`.

```bash
git add -A
git commit -m "Initial commit: KMET layer 4 — CLI, SDKs, bindings, web surface

Extracted from the KMET monorepo at <sha>. Includes the umbrella \`kmet\` crate, the cross-layer
conformance drift gates, and the oracle-derived corpora those gates test against — quarantined here
rather than in layers 1-3 (spec §1.1, layer X). Authorship history is preserved in the pre-cutover
evidence bundle (spec §4) and in the retained \`dry\` archive repository."
git push && git tag v0.7.0 && git push --tags
```

- [ ] **Step 2: Port the remaining CI jobs**

`python-sdk`, `wasm`, `ts-sdk`, `docs-site`, `krl`, `cloud`, `verify-runner` from
`.github/workflows/ci.yml`, plus `release.yml` in full — this is the repository that produces customer
artifacts, so the SBOM, the Rekor attestation and the `prod-1` signature all belong here.

- [ ] **Step 3: Verify the full customer surface builds**

```bash
cargo test --all
cd crates/wasm && cargo check && cd ../..
cd crates/cloud && cargo check && cd ../..
cd containers/verify-runner && cargo check && cd ../../..
cd py && maturin build && cd ..
cd sdk/ts && npm ci && npm run build && npm test && cd ../..
python tools/validate_vectors.py conformance/vectors || true  # vectors now live in kmet-kernel
```
Expected: green. Note that `validate_vectors.py` moved to `kmet-kernel` in Task 10 — the vectors are
fetched as part of that dependency, so drop this line from `kmet-tools` CI rather than working around it.

- [ ] **Step 4: Archive everything in `dry`**

Apply **The archival procedure** for each moved crate and directory. When this step completes, `dry`'s
Cargo workspace has no members and nothing in the repository builds — which is correct. It is now an
archive.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "build: graduate layer 4 to kmet-tools; archive it here"
```

---

## Task 12: Freeze `dry` as the archive

**Files:**
- Create: `ARCHIVE.md` at the repository root
- Modify: `README.md`, root `Cargo.toml`
- Modify: `.github/workflows/` (disable)
- Modify: `ip/ledger.toml` (add the four `[[evidence_pack]]` extraction records)

- [ ] **Step 1: Write the archive notice**

```markdown
<!-- ARCHIVE.md -->
# Frozen archive

This repository is the complete, unmodified record of the project through 2026, retained as the
authorship archive described in
`docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md` §4 and §5.2.

Nothing here is built, tested, shipped, or maintained. Active development lives in:

| Layer | Repository |
|---|---|
| 0 + 1 · kernel and IR contract | `<org>/kmet-kernel` |
| 2 · verification and assurance | `<org>/kmet-verify` |
| 3 · trace analytics and forensics | `<org>/kmet-trace` |
| 4 · CLI, SDKs, bindings, web | `<org>/kmet-tools` |

Retained here and graduated nowhere: `archive/conformance/oracle/` (GPLv3, spec §1.1 layer X) and
`archive/tools/license-issuer/` (spec §1.1 layer 5).

## Licence boundary — a permanent fact

Commits `14685a1` (2026-06-18) through `4701c11` (2026-07-25, tree `51259365a04ddf48d902b6ad02f34ee4f62625b8`) were published under Apache-2.0, and release `v0.3.0`
(2026-06-29) was distributed under it. That grant is irrevocable. Everything from `a40d151`
(2026-07-25) onward is proprietary. See spec §2.
```

- [ ] **Step 2: Empty the workspace and disable CI**

Set `members = []` in the root `Cargo.toml`. Rename `.github/workflows/ci.yml` and `release.yml` to
`.disabled` suffixes so no scheduled run fires against a repository nothing maintains.

- [ ] **Step 3: Record the extractions in the ledger**

Add one `[[evidence_pack]]` entry per graduation to `ip/ledger.toml` (spec §7.1) with: the successor
repository, its tag, the extraction SHA, and the archive path. This is what links each clean repository
back to the 460-commit record.

- [ ] **Step 4: Make it private and stop touching it**

Per spec §5.2 the repository is made private, not deleted. Confirm it is private, confirm the four
successors are private, and confirm the evidence bundle from spec §4 is readable on a second machine.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: freeze this repository as the KMET authorship archive"
```

---

## Open decisions this plan does not make

- **The GitHub organisation name.** `github.com/kmet` is taken (spec §5.1). Task 8 Step 1 needs the
  alternative before anything else runs.
- **Where layer 5 lives.** `tools/license-issuer/` and the `prod-1` key material stay in the `dry`
  archive under this plan, which preserves them but leaves them undevelopable. A fifth private
  repository (`kmet-licensing`) is the likely answer and is the strongest compartmentalisation case in
  the portfolio (spec §1.1, layer 5) — but it is a decision, not a default.
- **Trademark clearance on `KMET`** (spec §6.3, §10.2). Crates are named `kmet-*` on the assumption
  clearance succeeds; a failure forces a mechanical rename across four repositories.
- **Oracle retirement** (spec §5.5). Until it happens, the layer-X corpora constrain `kmet-tools` to
  hold the cross-layer drift gates. After it happens, those gates can move to the layer they test.
- **The §5.7 spec table** lists three crates; this plan implements four, because `kmet-contracts` proved
  necessary to break the `RotaryContracts.model: emit::Kinematics` cycle. Task 3 Step 6 makes that
  correction to the spec as part of the work.
