# Dry Core/CLI Hardening Handover (2026-07-30)

## Scope completed

Task focus: stabilize 5-axis fallback behavior so that the CLI has a deterministic BC reference model by
default when emitting 5-axis motion, while preserving existing 3-axis/legacy fixtures and AB default
emit behavior at the configuration/`EmitParams` level.

## What was done

### Core/default behavior

- `Kinematics::default()` remains **AB** in `crates/core/src/emit/kinematics.rs`.
  - `Kinematics::Ab { pivot_offset: [0,0,0], rotary_offset: [0,0] }`
  - This preserves existing conformance fixtures that assume default emit parity with 3-axis motion output.

- Added explicit fallback in `Profile::emit_params()`:
  - `crates/core/src/profile/mod.rs`
  - `emit_params()` now defaults `kinematics` to `REFERENCE_FIVE_AXIS_MACHINE` (BC) unless
    `machine.five_axis` is present.
  - This gives profile-driven 5-axis emits a stable BC reference target.

### CLI behavior

- `crates/cli/src/main.rs` now imports `REFERENCE_FIVE_AXIS_MACHINE` and uses it as the default for
  `dry emit --five-axis` when neither `--rotary-axes` nor `profile.machine.five_axis` is supplied.
  - This avoids changing `EmitParams::default()` semantics while keeping CLI behavior deterministic.

### Tests added/updated

- `crates/core/tests/kinematics.rs`
  - `default_kinematics_is_ab` now asserts AB defaults consistently.
  - `default_emit_byte_identical_to_explicit_ab` remains explicit coverage of AB default equivalence.
- `crates/core/tests/five_axis.rs`
  - corrected expectations to validate AB-default rotary outputs (`A`/`B` mapping) rather than BC.
- `crates/cli/tests/cli.rs`
  - added/confirmed `emit_five_axis_defaults_to_reference_bc_when_no_kinematics_provided`
    regression test:
    - `dry emit --five-axis` equals `dry emit --five-axis --rotary-axes bc`
    - asserts `B90`, `C0` in default CLI path.

## Validation run

- `cargo test -p dry-core -- --nocapture`
  - passes (all core tests)
- `cargo test -p dry-cli emit_five_axis_defaults_to_reference_bc_when_no_kinematics_provided -- --nocapture`
  - passes (target CLI regression)

## Current behavioral contract (important for next handoff)

1. `EmitParams::default().kinematics` is AB.
2. `Profile::emit_params()` provides BC fallback only in profile-derived emit params.
3. CLI `dry emit --five-axis` without `--rotary-axes` or `--profile.machine.five_axis`
   resolves to BC.

## Open notes

- Remaining explicit BC assumptions are expected to be CLI/path-specific and tested via regression.
- No pending test failures after the latest fixes.

## Suggested next step

- Continue to the next hardening item: choose `Task P5.3` (CNC RS-274 + optional STEP-NC intent export),
  or keep working on `Task P5.5` as planned in the user priority list.

