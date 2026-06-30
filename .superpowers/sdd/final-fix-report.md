# Final Fix Report — feat/explain-llm-compare

**Date:** 2026-06-30  
**Branch:** `feat/explain-llm-compare`

---

## Fix 1 — Restore the dependency-light default build

**File changed:** `Cargo.toml`

**Change:** Added `default-members = ["crates/core", "crates/cli"]` to the `[workspace]` section so that a bare `cargo build` compiles only the two core crates and does not pull in `crates/llm` (which depends on `ureq`).

**Verification:**

| Command | Result |
|---|---|
| `cargo build 2>&1 \| tail -5` | `Finished dev profile` — only `dry-core` and `dry-cli` compiled |
| `cargo build -v 2>&1 \| grep -c 'Compiling ureq'` | `0` — ureq not compiled |
| `cargo build --features llm 2>&1 \| tail -5` | `Finished dev profile` — succeeds with llm feature |
| `cargo test -p dry-llm 2>&1 \| tail -10` | `ok. 6 passed; 0 failed` |

---

## Fix 2 — Correct the documented `--llm --json` envelope

**File changed:** `docs/11-profiles-and-reports.md` (§3.6)

**Change:** Replaced the incorrect §3.6 JSON example with one that matches the actual serialized output. Key corrections:

- `recommendations` array: correct field names (`title`, `rationale`, `expected_effect`, `priority`, `action_kind`, `mode`, `field`, `value`) — removed `action` (wrong), added `rationale`, `expected_effect`, `action_kind` (the real serde name, lowercase enum variant)
- `results` array: changed from the wrong shape `{ "action", "verdict", "note" }` to the new correct shape `{ "title", "result" }` (see Fix 3)
- `result` object shows the real `ExecutionResult` fields: `action`, `before`, `after`, `verdict`, `note`
- `before`/`after` are `MetricSnapshot` objects: `{ total_time_s, max_flow_mm3_s, findings, error_count }`
- `verdict` values are snake_case (`improved`, `clean_no_gain`, `regressed`, `informational`)
- Retained the "NOT drift-gated / non-deterministic" note

---

## Fix 3 — Link results to their recommendation in the JSON envelope

**File changed:** `crates/cli/src/main.rs`

**Change:** In `run_explain_llm`, the `results` mapping was:
```rust
results.iter().map(|(_, r)| r).collect::<Vec<_>>()
```
Changed to:
```rust
results.iter().map(|(title, r)| serde_json::json!({ "title": title, "result": r })).collect::<Vec<_>>()
```

This makes each result entry carry the originating recommendation title alongside the `ExecutionResult`, matching the updated §3.6 doc.

**Verification:** `cargo clippy --all-targets --features llm -- -D warnings` — no warnings. `cargo build --features llm` — succeeds.

---

## Fix 4 — Add missing classify happy-path tests

**File changed:** `crates/core/src/recommend.rs`

**Added tests:**
- `rewrite_safe_is_executable` — `rewrite "safe"` → `Executable(Rewrite { mode: OptimizeMode::Safe })`
- `rewrite_max_is_executable` — `rewrite "max"` → `Executable(Rewrite { mode: OptimizeMode::Max })`
- `contract_min_temp_is_executable` — `contract min_temp "210"` → `Executable(Contract { field: MinTemp, override_: Scalar(210.0) })`
- `contract_monotonic_z_true_is_executable` — `contract monotonic_z "true"` → `Executable(Contract { field: MonotonicZ, override_: Flag(true) })`

**Verification:**
```
cargo test -p dry-core 2>&1 | grep "recommend::tests"
```
Output (all passing):
```
test recommend::tests::advisory_kind_is_advisory ... ok
test recommend::tests::contract_max_flow_is_executable ... ok
test recommend::tests::contract_min_temp_is_executable ... ok
test recommend::tests::contract_monotonic_z_true_is_executable ... ok
test recommend::tests::contract_speed_range_parses_pair ... ok
test recommend::tests::contract_override_is_informational_and_same_toolpath ... ok
test recommend::tests::rewrite_balanced_is_executable ... ok
test recommend::tests::rewrite_max_is_executable ... ok
test recommend::tests::rewrite_safe_is_executable ... ok
test recommend::tests::rewrite_without_mode_is_advisory ... ok
test recommend::tests::rewrite_safe_produces_measured_result ... ok
test recommend::tests::unknown_field_is_advisory ... ok
test recommend::tests::unparsable_value_is_advisory ... ok
```

---

## Fix 5 — Lint the llm-gated code in CI

**File changed:** `.github/workflows/ci.yml`

**Change:** Added `cargo clippy --all-targets --features llm -- -D warnings` to the "build + test with llm feature" step, between the build and the test:

```yaml
- name: build + test with llm feature
  run: |
    cargo build --features llm
    cargo clippy --all-targets --features llm -- -D warnings
    cargo test -p dry-llm
```

---

## Final Verification Suite — All Passed

| Command | Result |
|---|---|
| `cargo fmt --all` | No output — code already formatted |
| `cargo clippy --all-targets -- -D warnings \| tail -20` | `Finished dev profile` — no warnings |
| `cargo clippy --all-targets --features llm -- -D warnings \| tail -20` | `Finished dev profile` — no warnings |
| `cargo test \| tail -30` | All tests pass (including wasm_native_math) |
| `cargo build --features llm \| tail -5` | `Finished dev profile` |
| `cargo test -p dry-llm \| tail -20` | `ok. 6 passed; 0 failed` |

---

## Files Changed

| File | Fix |
|---|---|
| `Cargo.toml` | Fix 1: `default-members` |
| `crates/cli/src/main.rs` | Fix 3: `results` title linkage |
| `crates/core/src/recommend.rs` | Fix 4: classify happy-path tests |
| `docs/11-profiles-and-reports.md` | Fix 2 + Fix 3: §3.6 JSON example |
| `.github/workflows/ci.yml` | Fix 5: clippy with llm feature |

---

## Issues Encountered

None. All checks passed on first attempt. The test filter `cargo test -p dry-core recommend` did not match inline module tests (Rust test filter matches the test function name, not the module path) — confirmed tests by running `cargo test -p dry-core` and grepping the output for `recommend::tests::`.
