# Kinematics, real end-to-end — Machine-model v2 (cluster)

**Date:** 2026-06-30
**Status:** Approved design, ready for implementation
**Branch:** `feat/kinematics-cluster`
**Exploration inputs:** `docs/superpowers/specs/2026-06-30-machine-model-v2-brainstorm.md` (sub-features B & C)
and `docs/superpowers/specs/2026-06-30-klipper-import-brainstorm.md`.

## Problem

`dry` already *shapes* speed against machine kinematics (`balanced_pipeline` →
`adaptive_speed_with_kinematics` consumes `MachineKinematics { max_acceleration_mm_s2,
max_junction_velocity_mm_s }`), but three gaps leave kinematics half-wired:

1. **No verifier rule enforces kinematic limits.** A toolpath that bypasses `balanced` (or whose gated
   rewrite was rejected) can silently violate the machine's acceleration / junction limits. The verifier
   has rules for flow, speed, bounds, Z, temperature, retraction — but none for peak acceleration.
2. **No easy way to get real kinematics into a profile.** Profiles are hand-written JSON; the most
   reliable source of a printer's real limits — a Klipper `printer.cfg` — has no importer.
3. **`machine.kinematics` is invisible to SDK users.** Neither the wasm nor the PyO3 binding can run the
   kinematics-aware pipeline or verify against kinematic limits; only the geometry-only `safe` pass is
   reachable.

These three reinforce each other: the import (2) *populates* the kinematics a profile carries, the rule
(1) *enforces* them, and the SDK surface (3) makes both reachable from TS/Python. This cluster wires
kinematics end-to-end and **defers** firmware-dynamics modeling (pressure-advance / input-shaper) to a
later product decision.

## Decisions (resolved during brainstorming)

1. **One spec, three sequenced phases:** (1) peak-acceleration verifier rule → (2) Klipper import → (3)
   wasm/TS exposure. Each enables the next: the rule defines the kinematics contract surface, the import
   fills it from real hardware, the SDK surface exposes it.
2. **Peak-acceleration severity is split:** arc centripetal acceleration vs `max_acceleration_mm_s2` is an
   **Error** (real mechanical / stepper-skip risk); junction velocity-change (`|Δv|`) vs
   `max_junction_velocity_mm_s` is a **Warning** (cornering-smoothness / quality limit). Matches dry's
   precedent: machine-safety = Error, quality/process = Warning.
3. **SDK accepts kinematics as a JSON string** (`kinematics_json`) that deserializes to
   `MachineKinematics`, not flat float params — future-proof against adding fields later, engine stays the
   single source of truth.
4. **PA / input-shaper deferred.** No `firmware.pressure_advance` / `input_shaper` schema work in this
   cluster; the Klipper importer emits "deferred" warnings when it sees those sections.
5. **`dry-core` stays pure** — the INI parser is hand-rolled (no new dependency); all phases are
   deterministic and golden-gated; everything is backward compatible (`kinematics: None` disables the rule;
   new SDK entrypoints are additive).

---

## Phase 1 — `peak-acceleration` verifier rule (`dry-core`)

**Files:** `crates/core/src/verify.rs` (types + rule), `crates/core/src/profile.rs`
(`Profile::contracts()` mapping), `crates/core/src/report.rs` if the catalog summary lives there.

**Contracts surface** — added to `verify.rs` (kept here, not coupled to `profile::MachineKinematics`, so
`verify` stays free of a `profile` dependency):

```rust
/// Kinematic limits for the peak-acceleration rule. `None` (or all-None fields) disables the rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KinematicContracts {
    pub max_acceleration_mm_s2: Option<f64>,
    pub max_junction_velocity_mm_s: Option<f64>,
}
// on Contracts:
pub kinematics: Option<KinematicContracts>,
```

**Rule** — new `RuleId::PeakAcceleration` (wire string `"peak-acceleration"`), added to `RuleId::ALL` and
the catalog with summary *"a junction velocity-change or arc centripetal acceleration exceeds the machine
kinematic limit"*. In `verify` / `verify_stream`:

- **Arc segments** (kind = arc with radius `r`): `a_centripetal = (v_mm_s)² / r`; if
  `max_acceleration_mm_s2` is set and `a_centripetal > limit` → finding at that segment, **Severity::Error**.
- **Contiguous printing junctions** `(i, i+1)` where `segments[i]` end ≈ `segments[i+1]` start (within the
  0.1 mm contiguity tolerance used in `adaptive_speed`): `Δv = |v_{i+1} − v_i|` (mm/s); if
  `max_junction_velocity_mm_s` is set and `Δv > limit` → finding at segment `i`, **Severity::Warning**.

Each check is gated on its own limit being `Some`; both `None` → rule contributes nothing.

**Profile mapping** — `Profile::contracts()` populates `Contracts.kinematics` from `machine.kinematics`
(`MachineKinematics` → `KinematicContracts`, field-for-field). Profiles without a `kinematics` block yield
`kinematics: None`.

**Optimizer interaction** — none required. Because the centripetal check is an Error, `apply_gated`
already treats a post-rewrite peak-acceleration Error as a "new error rule" and rejects that span's
rewrite; a raw toolpath that violates the limit surfaces the Error directly. The Warning never blocks the
gate.

**Testing:**
- Unit: an arc whose `v²/r` exceeds `max_acceleration_mm_s2` produces a `peak-acceleration` Error; a
  junction whose `Δv` exceeds `max_junction_velocity_mm_s` produces a Warning; both `None` → no finding;
  a compliant path → none.
- The existing rule-catalog / report goldens (which exercise **every** `RuleId`) gain the new rule; the
  independent `tools/validate_reports.py` schema check must still pass.

---

## Phase 2 — `dry import-printer-cfg` (`dry-core` + CLI)

**Files:** `crates/core/src/profile/klipper.rs` (new submodule), `crates/core/src/profile.rs`
(re-export), `crates/core/src/lib.rs` (re-export), `crates/cli/src/main.rs` (`Cmd::ImportPrinterCfg`),
`conformance/profiles/` or `conformance/reports/` (fixture + golden).

**Core API:**

```rust
pub struct KlipperImportWarning { pub field: String, pub message: String }
pub enum KlipperImportError { /* NotKlipper, Io-agnostic parse failures */ }
pub fn import_klipper(text: &str) -> Result<(Profile, Vec<KlipperImportWarning>), KlipperImportError>;
```

The returned `Profile` always passes `Profile::validate()`. INI parsing is a hand-rolled line scanner
(skip `#` comments; match `[section]`; split `key: value` on the first `:`/`=`; trim) — **no new crate
dependency**.

**Field mapping (v1):**

| Klipper | dry field | Quality |
|---|---|---|
| *(file is a `.cfg`)* | `firmware.flavor = "klipper"` | exact |
| `[printer] max_accel` | `machine.kinematics.max_acceleration_mm_s2` | exact |
| `[printer] square_corner_velocity` | `machine.kinematics.max_junction_velocity_mm_s` | exact |
| `[extruder] filament_diameter` | `material.filament_diameter` | exact |
| `[extruder] min_extrude_temp` | `material.min_nozzle_temperature_c` | exact |
| `[stepper_x/y/z] position_min/max` | `machine.build_volume` | approximate (warn) |
| `[extruder] nozzle_diameter` | `process.line_width` | lossy (warn) |
| `[firmware_retraction] retract_length` | `process.max_retraction_distance` | approximate (warn) |
| `[firmware_retraction] retract_speed` ×60 | `process.max_retraction_speed` (mm/min) | approximate (warn) |

**Absent / deferred (each emits a warning):** `feedrate_range` — **omitted, not fabricated** (no Klipper
source for the lower bound); `material.max_volumetric_flow_mm3_s` — absent, with a **prominent** "add from
your hotend calibration" note (it is the single most useful safety contract for `review-gcode`);
`[input_shaper]` and `[extruder] pressure_advance` — no-op "deferred to a future machine-model release"
warnings; `[printer] kinematics` (cartesian/corexy/delta) — recorded in a warning only.

**v1 behavior defaults:** first `[extruder]` only (warn if more); delta kinematics → skip `build_volume`
+ warn (accel/SCV still import); no `--include-macros` (Jinja2 macro bodies are not valid g-code —
deferred); `[include]` directives not followed (warn + ignore).

**CLI:** `dry import-printer-cfg <file> [--out <path>] [--name <profile-name>]` — emits the `Profile` as
pretty JSON to stdout or `--out`; warnings to stderr (`warning: <field> — <message>`); `--name` sets
`profile.name` (default: file stem). **Errors:** unreadable file → "cannot read … : …", exit 2; no
`[printer]` section → "… does not look like a Klipper printer.cfg (no [printer] section)", exit 2;
malformed numeric value → field skipped + a warning (profile still emitted).

**Testing:**
- Unit (raw strings): the clean fields parse to the exact target values (units checked); a delta config
  skips `build_volume` + warns; a non-Klipper file → `KlipperImportError`; multi-extruder warns.
- Golden: a committed `conformance/.../voron.cfg`-style fixture → a drift-gated expected `Profile` JSON.
- CLI e2e (`crates/cli/tests/cli.rs`): `dry import-printer-cfg <fixture> --json`-equivalent run produces a
  Profile whose `machine.kinematics.max_acceleration_mm_s2` equals the fixture's `max_accel`.

---

## Phase 3 — expose `machine.kinematics` on wasm/TS (+ PyO3)

**Files:** `crates/wasm/src/lib.rs`, `py/src/lib.rs`, the TS SDK (interface + drift test), docs.

**New / extended entrypoints (both bindings, identical names + semantics):**

```rust
// wasm
pub fn resolve_balanced_ir(ops_json: &str, params_json: &str, kinematics_json: &str) -> Result<String, JsError>;
// resolve_verify gains a trailing kinematics_json param.
```

`kinematics_json` deserializes to `MachineKinematics` (empty string → `None`). `resolve_balanced_ir` runs
`balanced_pipeline(&tp, kinematics.as_ref())` when kinematics is present, else `safe_pipeline`. The
extended `resolve_verify` parses `kinematics_json` into `Contracts.kinematics` so the Phase-1 rule is
reachable from both SDKs. PyO3 mirrors the same signatures (`kinematics_json: Option<&str>`/`=None`).

**TS SDK:** hand-author a `MachineKinematics` interface matching the Rust serde shape (`maxAccelerationMmS2`
vs `max_acceleration_mm_s2` — match whatever the existing TS↔Rust convention is) and add a **drift test**
asserting the TS shape round-trips through the Rust engine (cross-SDK byte-identity, per the repo's
established TS-delegates-to-Rust pattern). Doc-note the `Kinematics` (5-axis rotary, `emit/kinematics.rs`)
vs `MachineKinematics` (profile) name collision to prevent confusion.

**Testing:** wasm + py unit tests (`kinematics_json` round-trips; present → balanced, empty → safe;
`resolve_verify` surfaces a `peak-acceleration` finding when a kinematic limit is exceeded); the TS drift
test. These run in the existing per-binding CI jobs (wasm/py are excluded from the core workspace).

---

## Data flow

```
printer.cfg ──import_klipper──▶ Profile { machine.kinematics: MachineKinematics }   (Phase 2, dry-core)
Profile::contracts() ─────────▶ Contracts { kinematics: Some(KinematicContracts) }  (Phase 1 wiring)
verify(IR, contracts) ────────▶ RuleId::PeakAcceleration  (arc → Error, junction → Warning)  (Phase 1)
MachineKinematics ────────────▶ balanced_pipeline(IR, kinematics)  (existing)
wasm/py/TS: kinematics_json ──▶ resolve_balanced_ir / resolve_verify  (Phase 3)
```

## Error handling

- Phase 1: `kinematics: None` (or all-`None` fields) silently disables the rule; arcs without a finite
  radius are skipped; non-contiguous junctions are skipped (consistent with `adaptive_speed`).
- Phase 2: see the CLI error table above; a partial profile is a valid, intended outcome (absent fields
  stay `None`, each surfaced as a warning).
- Phase 3: malformed `kinematics_json` → the binding's standard error (`JsError` / `PyErr`) with the parse
  message; empty string is the documented "no kinematics" sentinel, not an error.

## Testing & determinism

Everything is deterministic and golden-gated: Phase 1 via the rule-catalog/report goldens
(`crates/core/tests/…` + `tools/validate_reports.py`); Phase 2 via a committed `printer.cfg` fixture →
expected `Profile` JSON; Phase 3 via wasm/py unit tests + the TS drift test. No network, no new runtime
dependency. CI builds the wasm/py jobs as today.

## Scope / YAGNI (deferred)

- **PA / input-shaper modeling** and the `firmware.pressure_advance` / `machine.kinematics.input_shaper`
  schema work (and their calibration-data-source and model-fidelity questions) — a separate future spec.
- **Per-axis acceleration limits** (`max_acceleration_mm_s2` stays a scalar; conservative proxy).
- **Delta `build_volume` approximation**, **`--include-macros`**, **multi-extruder import**,
  **`[include]` following** — Klipper-import follow-ups.
- **A full `profile_json` wasm boundary type** — `kinematics_json` + the existing flat params cover the
  cluster.
- **Auto-generating TS types from the Rust serde shape** — hand-author + drift-test for now.
