# Post-Audit Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the cross-target capability divergence that lets the Python and TypeScript SDKs pass programs the engine refuses, make the gates that missed it actually enforce, and stop the assurance layer reporting four different proof strengths as one word.

**Architecture:** The engine (`crates/core`) is already correct in every case below — no engine rule changes. The work is (a) deleting two local reimplementations in the bindings and routing them through `dry_core::check_compatibility` behind an unchanged public signature, (b) strengthening four gates that reported green on a defect they were supposed to catch, and (c) making the Lean claim ledger distinguish kernel proof from compiled evaluation. Every task is independently testable and independently revertible.

**Tech Stack:** Rust (`crates/core`, `crates/cloud`, PyO3 in `py/`), Python (`py/python/dry`), TypeScript (`sdk/ts`), Lean 4 + Lake (`formal/`), GitHub Actions, TOML/JSON-Schema manifests.

**Spec:** [`docs/superpowers/specs/2026-09-05-post-audit-remediation.md`](../specs/2026-09-05-post-audit-remediation.md) — every finding ID (`C1`…`C14`) below refers to that document, which carries the evidence.

## Global Constraints

- **Baseline:** `main` @ `cf3216e`. All 20 tracked manifests read `0.10.0`.
- **Do not touch any version manifest, and do not add a dated `## [X.Y.Z]` heading to `CHANGELOG.md`.** All entries go under `## [Unreleased]`. The version bump and the release heading are one separate release-prep PR at cut time; see the spec's versioning section. Rationale: `## [0.10.0]` already declares "the compiler, IR, and report schemas are unchanged", which this work contradicts, and tag `v0.10.0` does not yet exist.
- **Target release: `v0.11.0` (MINOR).** Two public API additions plus a behavioural change in which previously-`compatible: true` programs become `compatible: false`.
- **One PR per group, each with the full aggregated check set green before merge.** Poll `gh pr checks <N>` — never a single workflow run; `ci.yml` and `codecov.yml` are separate runs whose ids differ by one digit.
- **Merge with `--merge` (merge commit), never squash.** Tags in this repo point at pre-merge commits and the release pipeline's `git merge-base --is-ancestor` guard depends on them staying reachable.
- **Never regenerate a golden, vector, proof fixture snapshot or mutation manifest to obtain a pass.** If one legitimately must change, that is a separate PR with its own review and a dated rationale note. Task 8 is the only task that touches a Lean fixture's emitted document, and it is designed to leave the bytes identical.
- **Local gate before every commit**, run serially — never two `cargo` commands against the same target dir at once:
  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  cargo test -p dry-core -p dry-cli
  ```
- **Excluded roots run their own gates from their own directory** (`crates/wasm`, `crates/cloud`, `py`, `containers/verify-runner`). Root workspace commands do not validate them.
- A material patch is complete only after an independent `reviewer` accepts it. The implementer cannot self-certify.

## File Structure

| File | Responsibility | Tasks |
|---|---|---|
| `crates/core/tests/retrospective_audit.rs` | Pin the arc-envelope rule distinctly from the plain bounds rule | 1 |
| `py/src/lib.rs` | New `check_compatibility_json` PyO3 entry point mirroring the wasm one | 2 |
| `py/python/dry/__init__.py` | `Design.check_compatibility` becomes a wire adapter over the engine; local rule loop deleted | 3 |
| `py/tests/test_capability.py` | Arc reproducer + delegation assertions | 2, 3 |
| `sdk/ts/src/engine.ts` | `DryWasm.check_machine_compatibility` declaration + `checkMachineCompatibility` wrapper | 4 |
| `sdk/ts/src/design.ts` | `Design.checkCompatibility` becomes a wire adapter; local rule loop deleted | 4 |
| `sdk/ts/test/capability.test.ts` | Arc reproducer + delegation assertions | 4 |
| `conformance/capability-parity.toml` | New `machine-compatibility` capability row | 5 |
| `tools/check_capability_parity.py` | Manifest-completeness assertion | 5 |
| `crates/cloud/src/lib.rs` | Fail closed on a malformed contracts header | 6 |
| `.github/workflows/ci.yml` | Run the two suites that exist and never execute | 7 |
| `formal/Dry/Tests/{ResolveChannels,SimulateMetrics}Fixtures.lean` | Emit the real check result instead of a constant | 8 |
| `proofs/claims.schema.json`, `proofs/claims.toml`, `tools/validate_proof_claims.py`, `tools/generate_assurance_report.py` | Record and surface proof strength | 9 |
| `formal/Dry/Numeric/SCurve.lean`, `formal/Dry/Geometry/Brep.lean`, `formal/Dry/Geometry/Clothoid.lean` | Titles and cross-references that match their content | 10 |
| `sdk/ts/src/ops.ts`, `sdk/ts/src/machine.ts` | Wire spellings that match the IR | 11 |
| `docs/02-roadmap.md`, `docs/27-system-capabilities-and-architecture-graph.md`, `crates/core/README.md`, `CHANGELOG.md` | Version horizon and dialect labels that match the code | 12 |

---

## PR 1 — Cross-target capability parity (C1, C2)

Fixes the blocker and the gate that could not see it. Tasks 1–5 ship together: splitting them would merge a state where the engine rule is pinned but the SDKs still diverge.

### Task 1: Pin the arc-envelope rule distinctly

**Files:**
- Modify: `crates/core/tests/retrospective_audit.rs:90-94`

**Interfaces:**
- Consumes: `dry_core::check_compatibility`, `MachineCapabilities`, `AxisRange` (already imported by this file).
- Produces: nothing consumed by later tasks; this is the regression pin the binding fixes are measured against.

The defect: the existing assertion is a disjunction satisfied by the plain bounds rule, so `ARC_OUT_OF_BOUNDS_X` is pinned by no test in the repository.

- [ ] **Step 1: Write the failing assertion**

Replace the disjunction at `crates/core/tests/retrospective_audit.rs:90-94`:

```rust
    let report = check_compatibility(&toolpath, &caps);
    assert!(!report.compatible);
    let codes: Vec<&str> = report.findings.iter().map(|f| f.code.as_str()).collect();
    assert!(
        codes.contains(&"ARC_OUT_OF_BOUNDS_X"),
        "the arc-envelope rule must fire on its own, not be masked by the endpoint rule: {codes:?}"
    );
```

- [ ] **Step 2: Run it and read the result**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p dry-core --test retrospective_audit
```

If it PASSES, the arc rule already fires on this fixture and the disjunction was merely sloppy — record that in the commit message and continue. If it FAILS, the fixture never reached the arc rule and the test was vacuous; add an arc whose endpoints are inside the envelope and whose circle is not, using the same `caps` the test already builds:

```rust
    design.arc_centre(210.0, 150.0, 60.0, 150.0);
```

Adjust the coordinates so both endpoints satisfy `caps.x_range` while `centre_x + hypot(dx, dy)` exceeds `caps.x_range.max`, then re-run.

- [ ] **Step 3: Confirm the rule is pinned in isolation**

```bash
cargo test -p dry-core --test retrospective_audit
```
Expected: PASS, with the `ARC_OUT_OF_BOUNDS_X` assertion reached.

- [ ] **Step 4: Commit**

```bash
git add crates/core/tests/retrospective_audit.rs
git commit -m "test(capability): pin the arc-envelope rule instead of accepting either code"
```

### Task 2: Expose the compatibility check to Python

**Files:**
- Modify: `py/src/lib.rs` (new `#[pyfunction]`, and register it in the `#[pymodule]` block at `:643`)
- Test: `py/tests/test_capability.py`

**Interfaces:**
- Consumes: `dry_core::{check_compatibility, MachineCapabilities}`, and the existing `parse`-equivalent helpers this file already uses to turn `ops_json` + `params_json` into a resolved `Toolpath`.
- Produces: `check_compatibility_json(ops_json: &str, params_json: &str, capabilities_json: &str) -> PyResult<String>`, returning the serialized `CompatibilityReport`. Task 3 calls exactly this name.

This mirrors `crates/wasm/src/lib.rs:437-449` one-for-one so the two bindings cannot drift again.

- [ ] **Step 1: Write the failing Python test**

Append to `py/tests/test_capability.py`:

```python
def test_an_arc_leaving_the_envelope_is_refused():
    """The engine bounds an arc by its full circle; the SDK must not report it compatible."""
    design = dry.Design().move(150, 150, 0.2).arc_centre(210, 150, 60, 150)
    caps = {
        "name": "test",
        "x_range": [0, 250],
        "y_range": [0, 250],
        "z_range": [0, 250],
    }
    report = design.check_compatibility(caps)
    codes = [f["code"] for f in report["findings"]]
    assert "ARC_OUT_OF_BOUNDS_X" in codes, codes
    assert report["compatible"] is False
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd py && python -m pytest tests/test_capability.py::test_an_arc_leaving_the_envelope_is_refused -v
```
Expected: FAIL — `ARC_OUT_OF_BOUNDS_X` is not in `codes`, because the local Python loop never emits it.

- [ ] **Step 3: Add the PyO3 entry point**

In `py/src/lib.rs`, next to the other resolve-and-analyse functions:

```rust
/// Pre-flight a design against machine capabilities in the engine.
///
/// Mirrors `check_machine_compatibility` in `crates/wasm/src/lib.rs`: the two bindings must expose
/// the same engine call, or the SDKs drift apart on which programs they refuse.
#[pyfunction]
fn check_compatibility_json(
    ops_json: &str,
    params_json: &str,
    capabilities_json: &str,
) -> PyResult<String> {
    let ops: Vec<dry_core::Op> = serde_json::from_str(ops_json)
        .map_err(|e| PyValueError::new_err(format!("invalid ops: {e}")))?;
    let params: dry_core::ResolveParams = serde_json::from_str(params_json)
        .map_err(|e| PyValueError::new_err(format!("invalid params: {e}")))?;
    let caps: dry_core::MachineCapabilities = serde_json::from_str(capabilities_json)
        .map_err(|e| PyValueError::new_err(format!("invalid capabilities: {e}")))?;
    let design = dry_core::Design { ops };
    let toolpath = dry_core::resolve_checked(&design, &params)
        .map_err(|e| PyValueError::new_err(e.to_string()))?;
    let report = dry_core::check_compatibility(&toolpath, &caps);
    serde_json::to_string(&report).map_err(|e| PyValueError::new_err(e.to_string()))
}
```

Match the surrounding file's actual construction of `Design`/`ResolveParams` — read the neighbouring `resolve_verify` function and copy its exact idiom rather than assuming the field names above.

- [ ] **Step 4: Register it in the module**

In the `#[pymodule]` block (`py/src/lib.rs:643` onwards), alongside the other registrations:

```rust
    m.add_function(wrap_pyfunction!(check_compatibility_json, m)?)?;
```

- [ ] **Step 5: Build the extension and confirm the symbol exists**

```bash
cd py && maturin develop && python -c "from dry import _native; print(_native.check_compatibility_json)"
```
Expected: a built function object, not `AttributeError`.

- [ ] **Step 6: Commit**

```bash
git add py/src/lib.rs py/tests/test_capability.py
git commit -m "feat(py): expose the engine compatibility check to the Python binding"
```

### Task 3: Route the Python SDK through the engine

**Files:**
- Modify: `py/python/dry/__init__.py:422-485` (the whole `check_compatibility` body)
- Test: `py/tests/test_capability.py`, `py/tests/test_machine_catalog.py` (must keep passing unchanged)

**Interfaces:**
- Consumes: `_native.check_compatibility_json` from Task 2.
- Produces: `Design.check_compatibility(capabilities, printer="generic")` with an **unchanged signature and unchanged return shape** — only the findings change, by gaining the two arc codes.

The public Python capability document uses `x_range: [min, max]` lists and `max_feedrate`; the engine uses `{"min": …, "max": …}` and `max_feedrate_mm_min`. Adapt at the boundary; do not change the public shape, and preserve the existing `[0, 300]` defaults for absent ranges.

- [ ] **Step 1: Replace the body with an adapter**

Replace lines 422-485 of `py/python/dry/__init__.py`:

```python
    def check_compatibility(
        self,
        capabilities: Mapping[str, Any],
        printer: str = "generic",
    ) -> Mapping[str, Any]:
        """Pre-flight check toolpath against machine capabilities (D2.2).

        The rules live in the engine (`dry_core::check_compatibility`), not here. An earlier local
        copy of this loop implemented five of the engine's seven rules and silently passed arcs
        whose swept circle leaves the build envelope.
        """
        engine_caps: Dict[str, Any] = {
            "name": str(capabilities.get("name", "unnamed")),
            "x_range": _axis_range(capabilities.get("x_range", [0, 300])),
            "y_range": _axis_range(capabilities.get("y_range", [0, 300])),
            "z_range": _axis_range(capabilities.get("z_range", [0, 300])),
        }
        if capabilities.get("max_feedrate") is not None:
            engine_caps["max_feedrate_mm_min"] = float(capabilities["max_feedrate"])
        if capabilities.get("max_spindle_rpm") is not None:
            engine_caps["max_spindle_rpm"] = float(capabilities["max_spindle_rpm"])

        p = PRINTERS[printer]
        return json.loads(
            _native.check_compatibility_json(
                json.dumps(self.ops),
                json.dumps(p),
                json.dumps(engine_caps),
            )
        )
```

Read how the neighbouring `ir()` (`py/python/dry/__init__.py:342`) builds its params argument and use that exact idiom for `p` rather than the sketch above.

- [ ] **Step 2: Add the range adapter**

Next to the other module-level helpers in `py/python/dry/__init__.py`:

```python
def _axis_range(value: Any) -> Dict[str, float]:
    """Accept the SDK's `[min, max]` list or the engine's `{"min": …, "max": …}` object."""
    if isinstance(value, Mapping):
        return {"min": float(value["min"]), "max": float(value["max"])}
    lo, hi = value
    return {"min": float(lo), "max": float(hi)}
```

- [ ] **Step 3: Run the full Python suite**

```bash
cd py && python -m pytest tests/ -v
```
Expected: PASS, including `test_an_arc_leaving_the_envelope_is_refused` from Task 2 and the pre-existing `test_capability.py` / `test_machine_catalog.py` assertions, which check codes rather than message text and therefore survive the message-format change.

- [ ] **Step 4: Confirm the local rules are gone**

```bash
grep -n 'OUT_OF_BOUNDS_X' py/python/dry/__init__.py
```
Expected: no output.

- [ ] **Step 5: Commit**

```bash
git add py/python/dry/__init__.py py/tests/
git commit -m "fix(py): route machine compatibility through the engine, not a local copy of five of its seven rules"
```

### Task 4: Route the TypeScript SDK through the engine

**Files:**
- Modify: `sdk/ts/src/engine.ts` (add to the `DryWasm` interface, add the wrapper)
- Modify: `sdk/ts/src/design.ts:535-593` (the whole `checkCompatibility` body)
- Test: `sdk/ts/test/capability.test.ts`

**Interfaces:**
- Consumes: the wasm export `check_machine_compatibility(ops_json, params_json, capabilities_json)` that already exists at `crates/wasm/src/lib.rs:437`.
- Produces: `checkMachineCompatibility(ops: Op[], params: ResolveParams, capabilitiesJson: string): CompatibilityReport` in `engine.ts`. `Design.checkCompatibility` keeps its signature and its camelCase result shape.

- [ ] **Step 1: Write the failing test**

Append to `sdk/ts/test/capability.test.ts`:

```ts
test('an arc leaving the envelope is refused', () => {
  const design = new Design().move(150, 150, 0.2).arcCentre(210, 150, 60, 150);
  const caps: MachineCapabilities = {
    name: 'test',
    xRange: { min: 0, max: 250 },
    yRange: { min: 0, max: 250 },
    zRange: { min: 0, max: 250 },
  };
  const report = design.checkCompatibility(caps);
  const codes = report.findings.map((f) => f.code);
  assert.ok(codes.includes('ARC_OUT_OF_BOUNDS_X'), codes.join(','));
  assert.equal(report.compatible, false);
});
```

Use the arc-builder name this SDK actually exposes — read `sdk/ts/src/design.ts` for it rather than assuming `arcCentre`.

- [ ] **Step 2: Run it to verify it fails**

```bash
cd sdk/ts && npm test -- capability
```
Expected: FAIL — `ARC_OUT_OF_BOUNDS_X` absent.

- [ ] **Step 3: Declare the binding and wrap it**

In `sdk/ts/src/engine.ts`, add to the `DryWasm` interface next to `resolve_verify`:

```ts
  check_machine_compatibility(
    opsJson: string,
    paramsJson: string,
    capabilitiesJson: string
  ): string;
```

and export the wrapper next to `resolveMetrics`:

```ts
/** Pre-flight a design against machine capabilities in the Rust engine. */
export function checkMachineCompatibility(
  ops: Op[],
  params: ResolveParams,
  capabilitiesJson: string
): unknown {
  return JSON.parse(
    bind().check_machine_compatibility(JSON.stringify(ops), JSON.stringify(params), capabilitiesJson)
  );
}
```

- [ ] **Step 4: Replace the local loop with an adapter**

Replace `sdk/ts/src/design.ts:535-593`:

```ts
  /**
   * Pre-flight check toolpath against machine capabilities (D2.2).
   *
   * The rules live in the engine. An earlier local copy implemented five of the engine's seven
   * rules and silently passed arcs whose swept circle leaves the build envelope.
   */
  checkCompatibility(capabilities: MachineCapabilities, printer = 'generic'): CompatibilityReport {
    const engineCaps: Record<string, unknown> = {
      name: capabilities.name ?? 'unnamed',
      x_range: { min: capabilities.xRange.min, max: capabilities.xRange.max },
      y_range: { min: capabilities.yRange.min, max: capabilities.yRange.max },
      z_range: { min: capabilities.zRange.min, max: capabilities.zRange.max },
    };
    if (capabilities.maxFeedrate !== undefined) engineCaps.max_feedrate_mm_min = capabilities.maxFeedrate;
    if (capabilities.maxSpindleRpm !== undefined) engineCaps.max_spindle_rpm = capabilities.maxSpindleRpm;

    const raw = checkMachineCompatibility(this.ops, params(printer), JSON.stringify(engineCaps)) as {
      compatible: boolean;
      findings: Array<{ severity: 'Warning' | 'Error'; code: string; message: string; segment_index?: number }>;
    };
    return {
      compatible: raw.compatible,
      findings: raw.findings.map((f) => ({
        severity: f.severity,
        code: f.code,
        message: f.message,
        segmentIndex: f.segment_index,
      })),
    };
  }
```

Add `checkMachineCompatibility` to the existing `from './engine'` import list at `sdk/ts/src/design.ts:5-13`.

- [ ] **Step 5: Rebuild the wasm bundle the SDK loads, then run the suite**

```bash
bash web/build.sh
cd sdk/ts && npm run build && npm test
```
Expected: PASS, including the pre-existing `capability.test.ts` and `machine.test.ts` assertions.

- [ ] **Step 6: Confirm the local rules are gone**

```bash
grep -n 'OUT_OF_BOUNDS_X' sdk/ts/src/design.ts
```
Expected: no output.

- [ ] **Step 7: Commit**

```bash
git add sdk/ts/src/engine.ts sdk/ts/src/design.ts sdk/ts/test/capability.test.ts
git commit -m "fix(sdk/ts): route machine compatibility through the engine, not a local copy of five of its seven rules"
```

### Task 5: Make the parity manifest cover this, and assert its own completeness

**Files:**
- Modify: `conformance/capability-parity.toml` (new capability row)
- Modify: `tools/check_capability_parity.py` (completeness assertion)

**Interfaces:**
- Consumes: the symbols Tasks 2 and 4 introduced (`check_compatibility_json`, `export function checkMachineCompatibility`).
- Produces: a failing gate for any future core capability that reaches no manifest row.

The manifest never had a `machine-compatibility` row, and its `python`/`ts` cells point at `py/src/lib.rs` and `sdk/ts/src/engine.ts` — neither of which is where the local copies lived. Recording the row is necessary but not sufficient; without a completeness assertion the next unrecorded capability is invisible too.

- [ ] **Step 1: Add the capability row**

Append to `conformance/capability-parity.toml`:

```toml
[[capability]]
id = "machine-compatibility"
title = "Machine capability pre-flight (envelope, arc envelope, feedrate, spindle)"
core = { path = "crates/core/src/profile/capability.rs", symbol = "pub fn check_compatibility", status = "reachable" }
cli = { path = "crates/cli/src/main.rs", symbol = "check_compatibility", status = "absent", note = "No CLI surface yet; the pre-flight is an SDK/binding affordance. Recorded as a reviewed gap, not an oversight." }
python = { path = "py/src/lib.rs", symbol = "check_compatibility_json", status = "reachable" }
wasm = { path = "crates/wasm/src/lib.rs", symbol = "check_machine_compatibility", status = "reachable" }
ts = { path = "sdk/ts/src/engine.ts", symbol = "export function checkMachineCompatibility", status = "reachable" }
```

- [ ] **Step 2: Verify the gate accepts it**

```bash
python3 tools/check_capability_parity.py
```
Expected: exit 0. If the `cli` cell fails because `check_compatibility` does appear in `crates/cli/src/main.rs`, change that cell to `reachable` with the real symbol — an `absent` cell that is actually present is precisely the failure this manifest exists to raise.

- [ ] **Step 3: Write the failing completeness check**

Add to `tools/check_capability_parity.py`, after the per-cell loop:

```python
EXEMPT_CORE_SYMBOLS = {
    # Engine internals and value types with no binding surface of their own. Each entry is a
    # reviewed decision: a capability belongs in the manifest, a type does not.
}


def check_manifest_completeness(manifest, errors):
    """Every capability re-exported from the engine root must have a manifest row or an exemption.

    The manifest went twelve capabilities deep while the engine exposed far more, and nothing said
    so. A capability with no row is invisible to this gate in both directions.
    """
    core_root = ROOT / "crates/core/src/lib.rs"
    exported = set(re.findall(r"pub use [^;]*?::\{?([^;{}]*)\}?;", core_root.read_text(encoding="utf-8")))
    names = {n.strip() for chunk in exported for n in chunk.split(",") if n.strip()}
    recorded = {c["core"]["symbol"].split()[-1] for c in manifest["capability"]}
    missing = sorted(n for n in names if n.startswith("check_") or n.startswith("try_"))
    for name in missing:
        if name not in recorded and name not in EXEMPT_CORE_SYMBOLS:
            fail(errors, f"engine exports '{name}' but no capability row or exemption covers it")
```

Call it from `main()` and seed `EXEMPT_CORE_SYMBOLS` from the run's own output — each entry added with a one-line reason, never as a bulk silencing.

- [ ] **Step 4: Run the gate and settle the exemptions**

```bash
python3 tools/check_capability_parity.py
```
Expected: initially FAIL, listing engine exports with no row. For each, decide: add a capability row (if it is a capability) or add an `EXEMPT_CORE_SYMBOLS` entry with its reason (if it is not). Re-run until exit 0.

- [ ] **Step 5: Prove the gate bites**

```bash
python3 - <<'PY'
import pathlib
p = pathlib.Path("conformance/capability-parity.toml")
orig = p.read_text()
p.write_text(orig.replace('id = "machine-compatibility"', 'id = "machine-compatibility-x"'))
PY
python3 tools/check_capability_parity.py; echo "EXIT=$?"
git checkout conformance/capability-parity.toml
```
Expected: non-zero exit while the row is renamed, exit 0 after the checkout.

- [ ] **Step 6: Commit**

```bash
git add conformance/capability-parity.toml tools/check_capability_parity.py
git commit -m "fix(conformance): record machine compatibility and assert the parity manifest is complete"
```

### Task 6: Land PR 1

- [ ] **Step 1: Run every affected gate**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p dry-core -p dry-cli
python3 tools/check_capability_parity.py
(cd py && python -m pytest tests/ -q)
(cd sdk/ts && npm test)
```

- [ ] **Step 2: Add the CHANGELOG entry under `## [Unreleased]`**

```markdown
### Fixed
- **Machine compatibility is checked by the engine on every surface.** The Python and TypeScript
  SDKs each carried a local copy of the pre-flight loop implementing five of the engine's seven
  rules; both omitted `ARC_OUT_OF_BOUNDS_X` and `ARC_OUT_OF_BOUNDS_Y`, so an arc whose swept circle
  leaves the build envelope returned `compatible: true` from those SDKs and `compatible: false`
  from Rust and wasm. Both now call `dry_core::check_compatibility` through a boundary adapter;
  the public signatures and result shapes are unchanged and only the findings differ. Finding
  messages now come from the engine and their wording changed accordingly.

### Added
- `check_compatibility_json` in the Python binding and `checkMachineCompatibility` in `sdk/ts`,
  mirroring the existing wasm `check_machine_compatibility`.
- `conformance/capability-parity.toml` gains a `machine-compatibility` row, and
  `tools/check_capability_parity.py` now fails when an engine capability has no row and no
  recorded exemption — the gap that let this divergence exist unnoticed.
```

- [ ] **Step 3: Open the PR, then dispatch an independent reviewer**

```bash
git push -u origin fix/capability-parity-across-bindings
gh pr create --title "fix(bindings): machine compatibility is checked by the engine on every surface" --body-file -
```

Dispatch a `reviewer` agent in review-only mode over the diff before merging. The implementer cannot self-certify.

- [ ] **Step 4: Watch the full aggregated check set and merge**

```bash
gh pr checks <N>          # all workflows, not one run
gh pr merge <N> --merge --delete-branch
```

---

## PR 2 — Fail closed, and run the tests that exist (C3, C4)

### Task 7: `crates/cloud` rejects a malformed contracts header

**Files:**
- Modify: `crates/cloud/src/lib.rs:51-53`

**Interfaces:** no new symbols; the route's error contract gains a 400.

- [ ] **Step 1: Replace the silent degradation**

```rust
    let contracts = match req.headers().get("X-Dry-Contracts")? {
        Some(contracts_str) => match serde_json::from_str(&contracts_str) {
            Ok(contracts) => contracts,
            // `Contracts::default()` disables every contract-driven check. Degrading to it on a
            // malformed header returns a clean-looking report for a program nobody verified.
            Err(e) => return Response::error(format!("invalid X-Dry-Contracts header: {e}"), 400),
        },
        None => Contracts::default(),
    };
```

- [ ] **Step 2: Build the crate from its own root**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd crates/cloud && cargo build --target wasm32-unknown-unknown --locked && cargo clippy --all-targets -- -D warnings
```
Expected: both exit 0.

- [ ] **Step 3: Commit**

```bash
git add crates/cloud/src/lib.rs
git commit -m "fix(cloud): reject a malformed contracts header instead of disabling every check"
```

### Task 8: Run `crates/wasm`'s unit tests in CI

**Files:**
- Modify: `.github/workflows/ci.yml` (the `wasm` job at `:232-296`)
- Modify: `crates/wasm/src/lib.rs:786` (the unreachable wasm32-gated test)

- [ ] **Step 1: Confirm the tests compile for the host target**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd crates/wasm && cargo test --locked
```
Read the result. If the three native-runnable tests pass, a plain `cargo test` step is the right fix. If they fail to compile for the host, they need `wasm-bindgen-test` and a headless runner instead — in that case stop and record the finding rather than deleting the tests.

- [ ] **Step 2: Add the step to the `wasm` job**

In `.github/workflows/ci.yml`, inside the `wasm` job, before the build step:

```yaml
      - name: unit tests
        id: wasm-test
        working-directory: crates/wasm
        run: cargo test --locked
```

Register `wasm-test` in that job's gate-summary `OUTCOMES` list (`ci.yml:275-281`) alongside the existing step ids, or the step's result is not aggregated.

- [ ] **Step 3: Resolve the unreachable test**

`crates/wasm/src/lib.rs:786` is `#[cfg(target_arch = "wasm32")] #[test]` with no wasm test runner configured anywhere in the repository, so it can never execute. Either convert it to `#[wasm_bindgen_test]` and add the runner, or delete it and fold its assertion into a host-runnable test. Do not leave a test that cannot run.

- [ ] **Step 4: Verify the job definition parses**

```bash
actionlint .github/workflows/ci.yml
```
Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml crates/wasm/src/lib.rs
git commit -m "fix(ci): run the wasm crate's unit tests, which no job executed"
```

### Task 9: Compile the release-only emission branch

**Files:**
- Modify: `.github/workflows/ci.yml:119`

`crates/core/tests/emit_rejects_unrepresentable.rs:63-69` states it must run under `--release`; the only release step is scoped to a different file.

- [ ] **Step 1: Extend the release step**

```yaml
        run: cargo test --release -p dry-core --test emit_refuses_non_finite --test emit_rejects_unrepresentable --locked
```

- [ ] **Step 2: Run it locally exactly as CI will**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --release -p dry-core --test emit_refuses_non_finite --test emit_rejects_unrepresentable --locked
```
Expected: PASS. If the release-only branch **fails**, that is a real emission defect that debug builds were hiding — stop, do not weaken the test, and open it as its own finding with the failing output.

- [ ] **Step 3: Commit and land PR 2**

```bash
git add .github/workflows/ci.yml
git commit -m "fix(ci): compile the release-mode emission branch the test file says must be covered"
```

Add the `## [Unreleased]` CHANGELOG entries, push, open the PR, dispatch an independent `reviewer`, watch `gh pr checks`, merge with `--merge`.

---

## PR 3 — Assurance honesty (C5, C6, C7 partial)

### Task 10: Fixtures emit their real check result

**Files:**
- Modify: `formal/Dry/Tests/ResolveChannelsFixtures.lean:146`
- Modify: `formal/Dry/Tests/SimulateMetricsFixtures.lean:130`

Nine of the eleven fixtures emit `Json.bool modelChecks`. These two emit the constant `Json.bool true`, so the Rust assertions at `crates/core/tests/resolve_channels_refinement.rs:129` and `crates/core/tests/simulate_metrics_refinement.rs:101` are unfalsifiable.

- [ ] **Step 1: Emit the computed value**

In `formal/Dry/Tests/SimulateMetricsFixtures.lean:130`:

```lean
    ("model_checks", Json.bool modelChecks),
```

In `formal/Dry/Tests/ResolveChannelsFixtures.lean:146`, the corresponding definition is `resolveChannelsFixtureChecks`:

```lean
    ("model_checks", Json.bool resolveChannelsFixtureChecks),
```

- [ ] **Step 2: Confirm the emitted bytes are unchanged**

```bash
python3 tools/check_proof_fixtures.py
```
Expected: exit 0 with **no** snapshot diff. Both predicates currently evaluate to `true`, so the document is byte-identical and no golden may be regenerated. If the snapshot differs, stop — that means a predicate is false and the fixture was concealing a real failure.

- [ ] **Step 3: Prove the assertion now bites**

Temporarily change `cases.length = 6` to `cases.length = 7` in `ResolveChannelsFixtures.lean`, re-run `tools/check_proof_fixtures.py`, confirm it fails, then revert.

- [ ] **Step 4: Commit**

```bash
git add formal/Dry/Tests/ResolveChannelsFixtures.lean formal/Dry/Tests/SimulateMetricsFixtures.lean
git commit -m "fix(formal): two fixtures emitted a hardcoded model_checks, making their Rust assertions unfalsifiable"
```

**Note for the reviewer:** this task makes the *assertion* real. It does **not** make the *check* meaningful — both predicates are still cardinality counts over a corpus whose `expected` block is derived from the model itself (spec C5). That is FM1.5's remaining work and belongs in its own slice, not here.

### Task 11: Record proof strength in the claim ledger

**Files:**
- Modify: `proofs/claims.schema.json` (new `proof_method` property)
- Modify: `proofs/claims.toml` (one `proof_method` per claim)
- Modify: `tools/validate_proof_claims.py` (require it; cross-check it against the Lean source)
- Modify: `tools/generate_assurance_report.py` and `docs/assurance/01-assurance-sitemap.md` (surface it)

Six claims are discharged by `native_decide` — compiled evaluation outside the kernel — and a repo-wide grep for `native_decide` across `*.md`, `*.toml` and `*.yml` returns zero hits.

- [ ] **Step 1: Add the enum to the schema**

In `proofs/claims.schema.json`, on the claim object:

```json
"proof_method": {
  "type": "string",
  "enum": ["kernel", "native_decide", "test_refinement"],
  "description": "How the claim is discharged. 'kernel' is a Lean kernel proof. 'native_decide' closes the goal by compiled evaluation, trusting the compiler and the Lean.ofReduceBool axiom rather than the kernel. 'test_refinement' means the Lean statement is abstract and the correspondence to Rust rests on a refinement corpus."
}
```

Add `"proof_method"` to the object's `required` list.

- [ ] **Step 2: Make the validator require and verify it**

In `tools/validate_proof_claims.py`, after the existing theorem-existence check:

```python
    lean_text = (ROOT / claim["lean_source"]).read_text(encoding="utf-8")
    uses_native = "native_decide" in lean_text
    if uses_native and claim["proof_method"] != "native_decide":
        fail(errors, f"{cid}: {claim['lean_source']} uses native_decide but proof_method is "
                     f"'{claim['proof_method']}' — kernel-external trust must be declared")
    if claim["proof_method"] == "native_decide" and not uses_native:
        fail(errors, f"{cid}: declared native_decide but {claim['lean_source']} does not use it")
```

- [ ] **Step 3: Run it and let it tell you the true classification**

```bash
python3 tools/validate_proof_claims.py
```
Expected: FAIL, naming each claim whose declared method disagrees with its source. Set each `proof_method` from that output — do not guess. The six `native_decide` claims are at `OrientationContractFixtures.lean:102`, `ResolveOrientationFixtures.lean:119`, `NestedApplicationFixtures.lean:163`, `SimulateMetricsFixtures.lean:79`, `NativeNumericFixtures.lean:202`, `DepositionFixtures.lean:61` (plus `CompositionShapeFixtures.lean:121`, registered as refinement evidence). Re-run until exit 0.

- [ ] **Step 4: Surface it in the sitemap**

Add a **Method** column to the generated table in `tools/generate_assurance_report.py`, then:

```bash
python3 tools/generate_assurance_report.py        # regenerate
python3 tools/generate_assurance_report.py --check # confirm the committed doc matches
```
Expected: `--check` exits 0 and `docs/assurance/01-assurance-sitemap.md` no longer prints `proved` for a `native_decide` claim.

- [ ] **Step 5: Commit**

```bash
git add proofs/claims.schema.json proofs/claims.toml tools/validate_proof_claims.py tools/generate_assurance_report.py docs/assurance/01-assurance-sitemap.md
git commit -m "fix(proofs): the ledger records how each claim is discharged, not just that it is"
```

### Task 12: Titles that match their theorems

**Files:**
- Modify: `proofs/claims.toml` (`FM1.NUMERIC.SCURVE.BOUNDS`, `FM1.GEOMETRY.BREP.NORMAL` titles)
- Modify: `formal/Dry/Geometry/Clothoid.lean:12` (cross-reference path)

- [ ] **Step 1: Retitle the two overclaimed entries**

`FM1.NUMERIC.SCURVE.BOUNDS` → *"S-curve profile parameters are sign-valid: acceleration, jerk and velocity bounds are non-negative and the validity predicate is sound"*. `formal/Dry/Numeric/SCurve.lean:31` proves exactly this; it models no phases, no `v(t)`, no `a(t)` and no duration.

`FM1.GEOMETRY.BREP.NORMAL` → *"The three cardinal B-Rep face normals are unit vectors"*. `formal/Dry/Geometry/Brep.lean:25` proves exactly this; there is no quadric and no normalization function.

Do not weaken the `exclusions` prose — it is already accurate; the titles are what outran it.

- [ ] **Step 2: Fix the dangling cross-reference**

`formal/Dry/Geometry/Clothoid.lean:12` cites `crates/core/src/optimize/clothoid.rs`, which does not exist. The file is `crates/core/src/clothoid.rs`.

- [ ] **Step 3: Verify**

```bash
ls crates/core/src/clothoid.rs
python3 tools/validate_proof_claims.py && python3 tools/validate_spec_claim_links.py && python3 tools/generate_assurance_report.py --check
```
Expected: all exit 0.

- [ ] **Step 4: Commit and land PR 3**

```bash
git add proofs/claims.toml formal/Dry/Geometry/Clothoid.lean
git commit -m "docs(proofs): two claim titles named work their theorems do not do"
```

Add the `## [Unreleased]` entries, push, PR, independent `reviewer`, `gh pr checks`, `--merge`.

---

## PR 4 — TypeScript wire contract (C8)

### Task 13: `SegmentKind` matches the wire, and the duplicate flavor union goes

**Files:**
- Modify: `sdk/ts/src/ops.ts:92`
- Modify: `sdk/ts/src/machine.ts:14`
- Test: `sdk/ts/test/` — a decode assertion

`crates/core/src/ir.rs` carries `#[serde(rename_all = "lowercase")]` on `SegmentKind`, so the JSON value is `manualgcode`. `spec/dry-ir-v0.schema.json:52` documents the split; `ops.ts:92` declares `'manual_gcode'`, which never matches a decoded document. The `Op` union at `ops.ts:42` keeps `'manual_gcode'` — that one is correct, because `resolve.rs:109` carries an explicit `#[serde(rename = "manual_gcode")]`.

- [ ] **Step 1: Write the failing test**

```ts
test('a manual-gcode segment decodes to the wire spelling', () => {
  const ir = new Design().manualGcode('M117 hi').ir();
  const kinds = ir.segments.map((s) => s.kind);
  assert.ok(kinds.includes('manualgcode'), kinds.join(','));
});
```

Use the builder name this SDK exposes — read `sdk/ts/src/design.ts` for it.

- [ ] **Step 2: Run it to verify it fails, then fix the union**

```bash
cd sdk/ts && npm test
```

In `sdk/ts/src/ops.ts:92`, change `| 'manual_gcode';` to `| 'manualgcode';`. Leave `ops.ts:42` and `ops.ts:116` alone — those are the `Op` tag and the field name, both correctly underscored.

- [ ] **Step 3: Delete the duplicate flavor union**

`sdk/ts/src/machine.ts:14` declares a firmware-flavor union containing `'reprap'` (which `FirmwareFlavor::named()` rejects) and missing `duet`, `siemens`, `heidenhain`, `haas`, `rapid`. The correct 15-name union already exists at `sdk/ts/src/engine.ts:159-174` and is the one `index.ts` re-exports. Delete the `machine.ts` copy and import the `engine.ts` type.

- [ ] **Step 4: Typecheck and test**

```bash
cd sdk/ts && npm run build && npm test
```
Expected: both exit 0.

- [ ] **Step 5: Commit and land PR 4**

```bash
git add sdk/ts/src/ops.ts sdk/ts/src/machine.ts sdk/ts/test/
git commit -m "fix(sdk/ts): SegmentKind used the binary spelling, so it never matched a decoded document"
```

---

## PR 5 — ABB RAPID gets an oracle (C10)

### Task 14: Pin the RAPID emitter

**Files:**
- Create: `conformance/reports/robot/reference-rapid.mod`
- Modify: `crates/core/tests/cnc_industrial_flavors.rs`
- Modify: `crates/core/src/emit/rapid.rs:32` (the ignored `_params`)

RAPID is the only emit flavor with neither a golden nor an external oracle. Unreached branches: the antipodal case at `crates/core/src/emit/rapid.rs:16-18` (a 180° X rotation — a robot flip if wrong), the general case at `:19-26`, `Dwell`/`WaitTime` at `:54-59`, `Arc`/`MoveC` at `:91-99`.

- [ ] **Step 1: Write unit tests for the three quaternion branches**

Cover `dz > 0.999999`, `dz < -0.999999` (antipodal) and the general case, asserting the resulting quaternion components directly rather than asserting the emitted text contains a substring.

- [ ] **Step 2: Write tests reaching `Dwell` and `Arc`**

Assert the emitted `WaitTime` and `MoveC` records.

- [ ] **Step 3: Freeze a golden**

Emit a small reference program to `conformance/reports/robot/reference-rapid.mod` and add a drift test comparing the emitter's output against it, following the `krl_program_structure.rs` pattern including its `UPDATE_GOLDEN` env gate.

- [ ] **Step 4: Resolve the ignored `_params`**

`crates/core/src/emit/rapid.rs:32` takes `_params: &EmitParams` and ignores it, so `cnc_frame` work offsets are silently dropped. Decide with `kernel-engineer`: either honour the frame as `emit/krl.rs` does, or refuse a `CncFrame` the emitter cannot express. **Silently dropping it is not one of the options.** Whichever is chosen, pin it with a test.

- [ ] **Step 5: Run, commit, land PR 5**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p dry-core --test cnc_industrial_flavors
```

---

## PR 6 — Gates that describe themselves accurately (C9, C11)

### Task 15: Four CLI subcommands get a smoke test

**Files:** Modify `crates/cli/tests/cli.rs`

`unpack`, `explain`, `schema` and `fleet` appear zero times in `crates/cli/tests/*.rs`. `pack` is tested, so the `pack` → `unpack` round trip is open.

- [ ] **Step 1: Write four smoke tests**

Each asserts the exit code and one output invariant. `unpack` asserts byte-equality with the `pack` input, closing the round trip. Follow the file's existing idiom (`env!("CARGO_BIN_EXE_dry")`, `cli.rs:34`).

- [ ] **Step 2: Run and commit**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p dry-cli
```

### Task 16: Codecov says what it is

**Files:** Modify `.github/workflows/codecov.yml`

The workflow computes `lcov.info` and uploads it; the upload action is commented out and there is no threshold, so a coverage regression cannot fail CI. Either assert a floor on `lcov.info`, or state in the workflow header that the job is informational and gates nothing. Do not leave a job whose name reads like a gate and whose behaviour is an artifact upload.

---

## PR 7 — Docs and roadmap alignment (C13, plus the roadmap edits)

### Task 17: One layer label, and a `v0.11.0` horizon

**Files:**
- Modify: `docs/27-system-capabilities-and-architecture-graph.md:23`
- Modify: `crates/core/README.md:13,36`
- Modify: `docs/02-roadmap.md` (Version Horizons table)

- [ ] **Step 1: Relabel `generate/` as L1**

Two normative docs place TPMS, pocket, drape, lathe, thread-mill and B-Rep at L0. The code settles it: `crates/core/src/generate/mod.rs:1,7` says "pure **L1** sugar", `crates/core/src/generate/tpms.rs:19` imports `crate::resolve::{Design, Op}` and produces L1 `Op`s, and ADR 0002 `:35` agrees. True L0 is `crates/core/src/features.rs`. Relabel the docs to L1 and add an explicit "L0 is `features.rs`" note.

Verify: `grep -n 'L0' docs/27-system-capabilities-and-architecture-graph.md crates/core/README.md` no longer names TPMS/pocket/drape/lathe/B-Rep.

- [ ] **Step 2: Add the `v0.11.0` row**

```markdown
| **`v0.11.0`** | **Post-Audit Remediation** | Cross-target capability parity enforced in the engine rather than reimplemented per binding; a parity manifest that asserts its own completeness; fail-closed contract ingress on the edge worker; CI executing the wasm and release-mode suites that existed but never ran; and a claim ledger that distinguishes kernel proof from compiled evaluation. | **Target** |
```

Annotate the `v0.9.0` row to record that its capability-parity gate was found non-enforcing and repaired in v0.11.0. Do not silently rewrite the v0.9.0 status: the roadmap already sets the precedent (its 2026-08-30 note) that "merged" and "released" are different claims — "gated" and "enforced" are too.

- [ ] **Step 3: Verify the docs build**

```bash
bash docs/site/build.sh public
```
Expected: exit 0, including link verification. Use workspace-relative markdown links only — never `file:///` (AGENTS.md Rule 4).

---

## Owner decisions — not implemented by this plan

Two findings are governance calls, not engineering ones. Both are recorded in the spec with their evidence; neither is actioned here.

- **C14 — `crates/cloud`'s `POST /verify` contradicts ADR 0003.** The ADR (Accepted) says the crate "stays archived"; the README says "Do not build on it"; the module docstring says "This crate is NOT product code" — and a product route exists, is promoted to "Tier 2" in `docs/27-verification-deployment-architecture.md`, and has a CI job named for it. Amend or supersede the ADR to describe the Tier 2 that exists, or revert the route. The fail-open defect (C3) is fixed in PR 2 either way, and Tier 2's use of `review_import_params()` + `Contracts::default()` against Tier 3's `profile.gcode_import_params()` + `profile.contracts()` means the two tiers do not produce the same report for the same input — that divergence needs documenting or closing whichever way the ADR lands.
- **C12 — `resolve` is both the L1→L2 pass and the spline geometry kernel**, with the same Catmull-Rom windowing loop written out at three layers and only a cardinality assertion (`crates/core/tests/spline.rs:41`) gating them against each other. The three copies currently agree, so there is no present defect. Extracting a shared sampler is a dialect-architecture decision and belongs to **D1.1**, which is still `[ ]` in `docs/04-tasks.md:131` — and D1.1's absence is also why C13's L0/L1 doc contradiction has no arbiter. Opening D1.1 is the higher-value move than a local refactor.

---

## Self-review

**Spec coverage.** C1 → Tasks 1–4. C2 → Task 5. C3 → Task 7. C4 → Tasks 8, 9. C5 → Task 10. C6 → Tasks 11, 12. C7 → not actioned; `formal/Dry/Geometry/Kinematics.lean` backs no registered claim, and modelling its non-singular branch plus aligning `1e-5` against the engine's `1e-9` is FM1.5 work with its own acceptance criteria — recorded in the spec, deliberately out of scope here. C8 → Task 13. C9 → Task 16. C10 → Task 14. C11 → Task 15. C12, C14 → owner decisions, above. C13 → Task 17.

**Type consistency.** `check_compatibility_json` (Task 2) is the name Task 3 calls. `checkMachineCompatibility` (Task 4, `engine.ts`) is the name Task 4's `design.ts` edit and Task 5's manifest row both use. The wasm export stays `check_machine_compatibility`, matching `crates/wasm/src/lib.rs:437` and Task 5's `wasm` cell. `proof_method` (Task 11) is spelled identically in the schema, the ledger, the validator and the report generator.

**Known soft spots, flagged rather than hidden.** Task 2's PyO3 body and Task 4's TS test both name builder methods (`Design { ops }`, `arcCentre`, `manualGcode`) taken from the surrounding code's shape rather than from a verified signature; each step says to read the neighbouring idiom first. Task 5's completeness regex is a starting point that will need its exemption set settled from its own first run — Step 4 says so explicitly. Task 8 Step 1 is a genuine fork: if `crates/wasm`'s tests do not compile for the host, the fix is a `wasm-bindgen-test` runner, not a `cargo test` step.
