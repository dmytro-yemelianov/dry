# Machine-model v2 — brainstorm / design sketch

**Date:** 2026-06-30
**Status:** Exploration / draft — no code changed
**Scope:** Three related sub-features from the roadmap

---

## 1. Problem / motivation

### Current state (grounded in code)

`MachineKinematics` (`crates/core/src/profile.rs`, lines 109–123) carries exactly two fields:

```
max_acceleration_mm_s2:     Option<f64>
max_junction_velocity_mm_s: Option<f64>
```

The docstring already draws the boundary clearly:

> "Pressure-advance / input-shaper models are explicitly out of scope for v1."

`balanced_pipeline` (`crates/core/src/optimize/mod.rs`, line 70) consumes these via
`adaptive_speed_with_kinematics`, which:

1. Caps arc feedrate to `v = sqrt(a_limit · r)`.
2. Scales corner feedrates by a cosine-of-half-angle factor.
3. Applies an absolute per-junction ceiling of `scv · factor · 60` mm/min when
   `max_junction_velocity_mm_s` is set.

That is the extent of machine-aware motion today. Three gaps follow.

**Gap 1 — no verifier rule enforces kinematic limits.**
The optimizer *shapes* speed, but the verifier (`crates/core/src/verify.rs`) has no rule that checks
the output against `MachineKinematics`. A toolpath submitted without going through `balanced_pipeline`,
or one where the gated rewrite was rejected, can silently violate kinematic limits. The 15 existing
`RuleId` variants cover flow, speed range, bounds, Z monotonicity, temperature, retraction — but not
peak acceleration or junction velocity.

**Gap 2 — no firmware-specific dynamics model.**
PA (pressure advance) and input-shaper calibration data have nowhere to live in the profile schema.
Today `FirmwareProfile` carries only `flavor: Option<String>`. The `Profile::contracts()` method maps
the machine profile to verifier contracts but cannot include firmware dynamics because those fields do
not exist.

**Gap 3 — `MachineKinematics` is invisible to SDK users.**
Both the wasm binding (`crates/wasm/src/lib.rs`) and the PyO3 binding (`py/src/lib.rs`) expose
`resolve_gcode`, `resolve_metrics`, `resolve_verify`, and `resolve_optimized_ir`. None of them
accept `MachineKinematics`. `resolve_optimized_ir` calls `optimize_pipeline` (the geometry-only safe
pass), not `balanced_pipeline`. There is no wasm or py entrypoint that runs the kinematics-aware
pipeline at all. Note: `Kinematics` *is* exposed in both bindings, but that is the 5-axis rotary
kinematics type (`emit/kinematics.rs`) — a completely different concept.

---

## 2. Sub-feature A: firmware-specific PA / input-shaper modeling

### What these features are and why they matter

**Pressure Advance (PA):** a firmware-side extruder control algorithm (Klipper's primary name;
Marlin calls it Linear Advance). It pre-pressurises the nozzle ahead of speed increases and bleeds
pressure after speed drops, compensating for the elastic delay in filament delivery. The effect is
characterised by a single scalar coefficient `K` (mm, or dimensionless depending on firmware
version). Without PA modeling, dry cannot predict corner under/over-extrusion at high speeds, and
cannot advise whether a given junction velocity is safe for material quality.

**Input Shaper (IS):** resonance compensation that filters toolhead motion commands to avoid exciting
structural resonance frequencies. The key calibration data is a resonant frequency `f_hz` (typically
per-axis: X and Y), a damping ratio `zeta`, and a shaper algorithm name (MZV, EI, 2HUMP_EI, etc.).
The practical effect for dry is that IS *changes the effective maximum acceleration* the toolhead can
safely achieve for a given print quality target. For a simple input-shaper model, the effective
acceleration ceiling is approximately `a_eff = (2π·f)² · clip_factor`, where `clip_factor` depends
on the shaper type. This is deterministic given the calibration parameters.

### Approaches

#### A1 — Profile annotation only (no motion transform)

Store PA `K` and IS `{frequency_hz, zeta, shaper}` in the profile schema. Do not alter the motion
IR or extruder schedule. Instead:

- For IS: derive `effective_max_acceleration_mm_s2` from the IS data and substitute it for (or
  clamp) `MachineKinematics.max_acceleration_mm_s2` in `adaptive_speed_with_kinematics`. This is a
  deterministic, one-line computation given the calibration numbers.
- For PA: record `K` as a profile annotation. Produce a verifier *advisory* (Warning) if a junction
  velocity change exceeds what PA can compensate given K (simplified: `ΔV > threshold(K, speed)`).
  Do not rewrite extruder timing.

Trade-offs: conservatively correct; doesn't reproduce exact firmware output; PA advisory is a
heuristic, not a physical simulation. Easy to add; does not touch IR at all.

#### A2 — Extruder-schedule rewrite for PA

Model what Klipper/Marlin would actually emit with PA active: ahead of each speed-increase corner,
add a brief extruder over-drive; after a speed-decrease corner, hold back. The rewrite is
deterministic given `K`, the corner angle, and the entry/exit speeds.

Trade-offs: higher fidelity; makes the emitted G-code explicitly reflect PA's extruder pulses (useful
for non-Klipper targets that need them baked in). Adds complexity: the rewrite depends on the
firmware's PA implementation details (Klipper's `extruder_smoother`, Marlin's `LIN_ADVANCE`), which
differ. Risks diverging from firmware behaviour across firmware versions. Hard to test without a
physical printer.

#### A3 — IS-derived effective-acceleration override (subset of A1, cleanest deterministic path)

Derive `a_eff` from IS data and propagate it into the existing acceleration pipeline:

```
a_eff = (2π · f_hz)²   // for a simple single-hump shaper  (speculative — real factor depends on shaper type)
MachineKinematics.max_acceleration_mm_s2_effective = min(user_supplied, a_eff)
```

Then pass this derived value into `adaptive_speed_with_kinematics` unchanged. No IR transform; no PA.

### Recommendation

**Start with A1 + A3 merged** as "Phase A-zero":

1. Add two optional blocks to the profile schema:
   - `machine.kinematics.input_shaper: Option<InputShaperProfile>` with fields
     `frequency_hz_x`, `frequency_hz_y`, `damping_ratio`, `shaper_name`.
   - `firmware.pressure_advance: Option<f64>` (the K coefficient).
2. In `MachineKinematics::effective_acceleration()` (a new pure method), derive the IS-limited
   acceleration ceiling from the shaper data and return `min(max_acceleration_mm_s2, a_eff)`.
3. Thread this through `balanced_pipeline` transparently.
4. Add a verifier *Warning* for segments where the junction ΔV implies PA compensation beyond a
   simple threshold — flag it advisory, not error, until the PA model is validated.

Do **not** implement approach A2 yet — extruder-schedule rewriting for PA is speculative and
firmware-version-sensitive.

**What is genuinely hard / speculative:**
- The IS `clip_factor` for real shaper types (MZV, EI, 2HUMP_EI) is non-trivial; the formula above
  is illustrative. A clean implementation needs the per-shaper formulas from Klipper's reference, or
  a lookup table derived from first principles.
- PA `K` thresholding: without empirical validation, the verifier advisory threshold will be a
  guess. It should be clearly labelled as heuristic in the code and docs.
- IS is fundamentally axis-asymmetric (X and Y can differ); `max_acceleration_mm_s2` is a scalar.
  Using `min(f_x, f_y)` is conservative and correct for a first pass.

---

## 3. Sub-feature B: `peak-acceleration` verifier rule

This is the smallest, highest-confidence piece. The engine already has all the data needed; the
rule just needs to be wired in.

### Design

**New `RuleId` variant:** `RuleId::PeakAcceleration` (wire string `"peak-acceleration"`)

**Default severity:** `Error`. Exceeding the machine's kinematic limits causes stepper skips,
resonance, or (for junction velocity) immediate toolhead stress. This is a machine-safety concern,
not a quality advisory.

**What it checks:**

For each pair of consecutive printing segments `(i, i+1)` where the endpoint of `i` matches the
start of `i+1` (within 0.1 mm, consistent with the `is_contiguous` check in `adaptive_speed.rs`):

```
v_i    = segments[i].speed.value() / 60.0     // mm/s
v_next = segments[i+1].speed.value() / 60.0   // mm/s
delta_v = (v_next - v_i).abs()                // mm/s

if delta_v > contracts.kinematics.max_junction_velocity_mm_s:
    push(RuleId::PeakAcceleration, Some(i), ...)
```

For arc segments, additionally:

```
v = s.speed.value() / 60.0   // mm/s
a_centripetal = v² / r        // mm/s²

if a_centripetal > contracts.kinematics.max_acceleration_mm_s2:
    push(RuleId::PeakAcceleration, Some(i), ...)
```

**How it slots into `Contracts`:**

Add a field to `Contracts`:

```rust
/// Kinematic limits for peak-acceleration checking. None disables the rule.
pub kinematics: Option<KinematicContracts>,
```

where `KinematicContracts` is a thin struct (or just re-use `MachineKinematics` directly since it
already derives `Deserialize`):

```rust
pub struct KinematicContracts {
    pub max_acceleration_mm_s2: Option<f64>,
    pub max_junction_velocity_mm_s: Option<f64>,
}
```

Update `Profile::contracts()` to populate this from `machine.kinematics`.

**Catalog update:**

Add to `RuleId::ALL` (at position 15, after `FirstLayerSpeed`):

```rust
RuleId::PeakAcceleration
```

Summary: `"a junction or arc centripetal acceleration exceeds the machine kinematic limit"`.

**Gate / interaction with optimizer:**

This rule fires *after* any optimization pass that reduces speed. The gated optimizer already checks
for new error rules; once this rule exists, a `balanced_pipeline` run that fails to reduce a
corner's speed sufficiently will cause `apply_gated` to see the new rule in post-errors and reject
the rewrite — or, if the raw toolpath already violates it, the rule fires on the unoptimized path.
No changes to `apply_gated` itself are needed.

---

## 4. Sub-feature C: expose `machine.kinematics` on wasm/TS

### Current state

Neither the wasm nor the py binding exposes any kinematics-aware optimization. `resolve_optimized_ir`
(both bindings) calls `optimize_pipeline(&tp)` which is `arc_fit(merge_collinear(tp))` — the
geometry-only safe pass. `balanced_pipeline`, which accepts `Option<&MachineKinematics>`, is never
reachable from the wasm or py surface.

`Kinematics` *is* already exposed (as a string `"ab"/"ac"/"bc"`), but that is the 5-axis rotary
type in `emit/kinematics.rs` — unrelated to `MachineKinematics` in `profile.rs`. The name
collision is a documentation risk.

### What's needed

**Option C1 — flat scalar params (consistent with existing verify API):**

Add new wasm exports:

```rust
#[wasm_bindgen]
pub fn resolve_balanced_ir(
    ops_json: &str,
    params_json: &str,
    max_acceleration: f64,      // 0 → unset
    max_junction_velocity: f64, // 0 → unset
) -> Result<String, JsError>
```

and equivalently for py:

```rust
#[pyfunction]
#[pyo3(signature=(ops_json, params_json, max_acceleration=None, max_junction_velocity=None))]
fn resolve_balanced_ir(...) -> PyResult<String>
```

Both call `balanced_pipeline(&tp, Some(&MachineKinematics { ... }))` when at least one value is
set, else `safe_pipeline`.

The pattern matches `resolve_verify`'s flat-float convention: `max_flow_opt: f64` (0 → unset).
It is the lowest-friction change.

**Option C2 — kinematics as JSON string:**

Accept an optional `kinematics_json: &str` that deserializes to `MachineKinematics`. More
future-proof (the struct can grow new fields without changing the function signature). Slightly more
friction for simple callers. Suitable if PA/IS fields are added to `MachineKinematics`.

**Option C3 — expose the whole profile as JSON:**

`resolve_with_profile(ops_json, profile_json)` that parses a full `Profile`, derives
`resolve_params`, `contracts`, and `kinematics`, runs `balanced_pipeline`, and returns the optimized
IR. Maximally clean for callers who already hold a profile object; requires the TS SDK to construct
a full profile JSON blob.

### Recommendation

**C2 for the balanced IR functions** (because `MachineKinematics` will grow IS/PA fields and C1
won't scale), **with C1 as a thin fallback wrapper** for callers that only need the two scalar
fields. Both bindings should expose the same function name and semantics — the engine is the single
source of truth.

For `resolve_verify`: extend with the same `kinematics_json` optional param so the peak-acceleration
rule (Sub-feature B) is reachable from both SDKs.

**Cross-SDK identity:**

`MachineKinematics` derives `Serialize + Deserialize`, so the JSON round-trip is already clean.
The TS SDK types should be generated from the Rust struct's serde shape (or a JSON Schema derived
from it), not hand-written, to prevent drift.

---

## 5. Components and data flow

```
Profile (profile.rs)
  machine.kinematics: Option<MachineKinematics>
      max_acceleration_mm_s2
      max_junction_velocity_mm_s
      [NEW] input_shaper: Option<InputShaperProfile>    ← Sub-feature A
  firmware
      [NEW] pressure_advance: Option<f64>               ← Sub-feature A

Profile::contracts() → Contracts
  [NEW] kinematics: Option<KinematicContracts>          ← Sub-feature B wiring

verify_stream(segments, &contracts)
  [NEW] RuleId::PeakAcceleration check at each junction ← Sub-feature B

balanced_pipeline(&tp, Option<&MachineKinematics>)
  → adaptive_speed_with_kinematics(...)
      [NEW] uses MachineKinematics::effective_acceleration()
            which applies IS-derived limit               ← Sub-feature A

crates/wasm/src/lib.rs
  [NEW] resolve_balanced_ir(ops, params, kinematics_json) ← Sub-feature C
  [NEW] resolve_verify extended with kinematics_json      ← Sub-feature B+C

py/src/lib.rs  (same exports, mirrored)                  ← Sub-feature C
```

`Kinematics` (emit/kinematics.rs, the 5-axis rotary type) is **unrelated** and unchanged. The name
collision should be called out in docs/naming comments to prevent future confusion.

---

## 6. Recommended sequencing

### Step 1: Sub-feature B — peak-acceleration rule (do this first)

**Why first:** smallest delta, highest confidence, zero speculation. The only changes are:
- Add `kinematics: Option<KinematicContracts>` to `Contracts` (backward-compatible: `None` = no change)
- Add `RuleId::PeakAcceleration` to the catalog
- Add 20–30 lines of logic to `verify_stream`
- Update `Profile::contracts()`

This delivers immediate value: any toolpath reviewed against a profile with kinematic limits now
gets machine-safety checking, not just speed-range and flow checks. It also hardens the optimizer:
`apply_gated` will automatically treat peak-accel violations as new error rules to suppress.

### Step 2: Sub-feature C — expose kinematics on wasm/TS

**Why second:** once the verifier rule exists, SDK users need to be able to both *run* the
kinematics-aware optimizer and *verify* the result — both from wasm/py. Extend `resolve_verify` to
accept the kinematic contracts; add `resolve_balanced_ir`. The TS SDK wraps these two new exports.

### Step 3: Sub-feature A — PA / input-shaper (deferred until product decision)

**Why last:** requires a product decision on how faithful the model must be (see §7). The safe
Phase A-zero (schema extension + IS-derived acceleration) can be prototyped after B and C are done.
Full PA extruder-schedule rewriting should wait for empirical validation.

---

## 7. Scope / YAGNI

Explicitly defer:

- **Extruder-schedule rewriting for PA** (Sub-feature A2): firmware-version-sensitive, speculative,
  not testable without hardware. Defer until a specific firmware target is chosen and validated.
- **Per-axis acceleration limits**: `max_acceleration_mm_s2` is a scalar; real machines have
  different X/Y/Z limits. The scalar is a safe conservative proxy. Per-axis limits add significant
  IR complexity (direction-decomposed velocity vectors). Defer.
- **`profile_json` as a first-class wasm boundary type**: the flat-param + JSON-kinematics approach
  covers all current use cases. A full profile object at the wasm boundary is a bigger API redesign.
- **Segment-level extruder-dynamics time series** in the trace output: useful for PA validation but
  a separate feature.
- **TS type generation from Rust serde shapes**: ideal long-term; for now, manually author the TS
  interface for `MachineKinematics` and add a drift test.

---

## 8. Open questions for the user

1. **Calibration data source for PA/IS**: Should `machine.kinematics.input_shaper` and
   `firmware.pressure_advance` live in the user-editable profile JSON (checked into source, stable),
   or in a separate "calibration result" file that the profile references by path? The distinction
   matters for reproducibility: a profile pinned to specific calibration data produces reproducible
   reports; a floating reference can drift silently.

2. **PA model fidelity**: Is the goal (a) a verifier advisory that warns when a junction is likely
   to have visible under-extrusion given K, or (b) a motion transform that changes the emitted
   extruder schedule to bake in the PA pulse? These are very different scopes. (b) only makes sense
   for non-Klipper targets where the firmware does not apply PA itself.

3. **IS shaper type coverage**: The MZV, EI, and 2HUMP_EI shapers have different effective
   acceleration formulas. Should dry implement all three, or only MZV (the Klipper default and most
   common) for the first pass? The formula is not complex but the number of variants matters for
   test coverage.

4. **Peak-acceleration severity**: Should `RuleId::PeakAcceleration` always be `Error`, or should
   the junction-velocity check be a `Warning` (advisory, can print but will have quality impact)
   while only the centripetal-acceleration check (real mechanical stress) is `Error`? The existing
   precedent is: machine-safety = Error, quality/process advisory = Warning.

5. **wasm API shape**: prefer flat floats (simpler, matches `resolve_verify`) or JSON string for
   `MachineKinematics` (more future-proof as IS/PA fields are added)? This is a commitment that is
   hard to change without breaking SDK consumers.

6. **TS SDK type generation**: is there already toolgen infrastructure from Rust/serde to TS types,
   or will the TS interface for `MachineKinematics` be hand-authored and drift-tested?
