# Kinematics Cluster (Machine-model v2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire machine kinematics end-to-end: a verifier rule that enforces acceleration/junction limits, a `dry import-printer-cfg` that fills a profile's kinematics from a real Klipper `printer.cfg`, and wasm/PyO3/TS surface to run + verify with kinematics.

**Architecture:** Three sequenced phases. **Phase 1** adds the verifier checks (`dry-core`, pure). **Phase 2** adds the Klipper importer (`dry-core` + CLI). **Phase 3** exposes kinematics on the wasm/PyO3/TS bindings. Each phase is independently shippable; later phases build on Phase 1's `Contracts.kinematics` surface.

**Tech Stack:** Rust workspace (`crates/core`, `crates/cli`, `crates/wasm`, `py/` PyO3, `sdk/ts/` TypeScript). `serde`/`serde_json`, `clap`, `wasm-bindgen`, `pyo3`. No new runtime dependencies.

## Global Constraints

- **`dry-core` stays pure** — no HTTP/async/new deps. The Klipper INI parser is hand-rolled.
- **Backward compatible** — `Contracts.kinematics: None` (or all-`None` fields) disables the new rule; all new SDK entrypoints are additive.
- **Severity split (locked):** arc centripetal acceleration vs `max_acceleration_mm_s2` = **Error**; junction velocity-change vs `max_junction_velocity_mm_s` = **Warning**. Implemented as **two `RuleId`s** (`PeakAcceleration`=Error, `JunctionVelocity`=Warning) because the codebase maps one severity per `RuleId` (`default_severity()`/`catalog()`).
- **SDK kinematics surface (locked):** passed as a JSON string `kinematics_json` deserializing to `MachineKinematics` (empty string → none). Strings marshal fine across wasm-bindgen (unlike nested vecs).
- **Determinism:** every phase is deterministic and golden-gated; PA/input-shaper is **out of scope** (deferred).
- **Exact field names** (from the code): `MachineKinematics { max_acceleration_mm_s2: Option<f64>, max_junction_velocity_mm_s: Option<f64> }`; `Segment { kind: SegmentKind, center, start: [Option<Length>;3], end: [Option<Length>;3], speed: Feedrate (mm/min via .value()), travel: bool, volume, length }`; `SegmentKind::Arc`. `Feedrate.value()` is mm/min → divide by 60 for mm/s.
- **Commit cadence:** one commit per task.

---

## Phase 1 — `peak-acceleration` / `junction-velocity` verifier rules (`dry-core`)

### Task 1: Kinematic contract types, two RuleIds, profile mapping

**Files:**
- Modify: `crates/core/src/verify.rs` (add `KinematicContracts`, `Contracts.kinematics`, two `RuleId` variants + their `ALL`/`default_severity`/`summary` entries)
- Modify: `crates/core/src/profile.rs` (`Profile::contracts()` mapping)
- Modify: `crates/core/src/lib.rs` (re-export `KinematicContracts`)
- Test: inline tests in `verify.rs`

**Interfaces:**
- Produces: `pub struct KinematicContracts { pub max_acceleration_mm_s2: Option<f64>, pub max_junction_velocity_mm_s: Option<f64> }` (derive `Debug, Clone, Default, Deserialize`); `Contracts.kinematics: Option<KinematicContracts>`; `RuleId::PeakAcceleration` (`"peak-acceleration"`, Error), `RuleId::JunctionVelocity` (`"junction-velocity"`, Warning).

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/core/src/verify.rs`:

```rust
#[test]
fn catalog_includes_the_two_kinematic_rules() {
    let cat = catalog();
    let pa = cat.iter().find(|r| r.id == RuleId::PeakAcceleration).expect("peak-acceleration in catalog");
    assert_eq!(pa.severity, Severity::Error);
    assert_eq!(RuleId::PeakAcceleration.as_str(), "peak-acceleration");
    let jv = cat.iter().find(|r| r.id == RuleId::JunctionVelocity).expect("junction-velocity in catalog");
    assert_eq!(jv.severity, Severity::Warning);
    assert_eq!(RuleId::JunctionVelocity.as_str(), "junction-velocity");
}

#[test]
fn contracts_default_has_no_kinematics() {
    assert!(Contracts::default().kinematics.is_none());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dry-core catalog_includes_the_two_kinematic_rules`
Expected: FAIL — `RuleId::PeakAcceleration` / `Contracts.kinematics` not defined.

- [ ] **Step 3: Implement the types + catalog + mapping**

In `crates/core/src/verify.rs`:
- Add to the `Contracts` struct (after `first_layer_speed_range`):
```rust
    /// Kinematic limits for the peak-acceleration / junction-velocity rules. `None` disables them.
    #[serde(default)]
    pub kinematics: Option<KinematicContracts>,
```
- Add the type (near `Contracts`):
```rust
/// Kinematic limits checked by the `peak-acceleration` (arc centripetal) and `junction-velocity`
/// (per-junction Δv) rules. An unset field disables its check.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct KinematicContracts {
    pub max_acceleration_mm_s2: Option<f64>,
    pub max_junction_velocity_mm_s: Option<f64>,
}
```
- Add `PeakAcceleration` and `JunctionVelocity` to the `RuleId` enum; extend `ALL` to `[RuleId; 17]` with both appended after `FirstLayerSpeed`; add their `as_str()` arms (`"peak-acceleration"`, `"junction-velocity"`); add `RuleId::JunctionVelocity` to the `Warning` arm of `default_severity()` (leaving `PeakAcceleration` in the `_ => Severity::Error` default); add `summary()` arms:
  - `PeakAcceleration => "an arc's centripetal acceleration exceeds the machine's max acceleration"`
  - `JunctionVelocity => "a junction's velocity change exceeds the machine's square-corner velocity"`

In `crates/core/src/profile.rs` `Profile::contracts()`, add:
```rust
            kinematics: self.machine.kinematics.as_ref().map(|k| crate::verify::KinematicContracts {
                max_acceleration_mm_s2: k.max_acceleration_mm_s2,
                max_junction_velocity_mm_s: k.max_junction_velocity_mm_s,
            }),
```

In `crates/core/src/lib.rs`, add `KinematicContracts` to the `pub use verify::{...}` list.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p dry-core verify` and `cargo test -p dry-core` (no regressions). `cargo clippy -p dry-core --all-targets -- -D warnings`.
Expected: PASS. (Existing rule-catalog goldens that assert the catalog length/contents may need regenerating — if a golden test fails because the catalog grew, regenerate it per its documented mechanism and confirm the two new rules appear.)

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/verify.rs crates/core/src/profile.rs crates/core/src/lib.rs
git commit -m "feat(core): kinematic contracts + peak-acceleration/junction-velocity rule ids"
```

### Task 2: The two kinematic checks in `verify_stream`

**Files:** Modify `crates/core/src/verify.rs` (checks inside `verify_stream`); Test: inline.

**Interfaces:** Consumes Task 1's `Contracts.kinematics`, `RuleId::{PeakAcceleration, JunctionVelocity}`. Uses `Segment` fields `kind`, `center`, `start`, `speed` (mm/min).

- [ ] **Step 1: Write the failing tests**

Add to `tests` in `verify.rs` (use the crate's existing test helpers for building segments — mirror how nearby tests construct `Segment`/`Toolpath`; the assertions are what matter):

```rust
#[test]
fn arc_over_centripetal_limit_is_a_peak_acceleration_error() {
    // An arc of radius r taken at speed v has a = v^2/r. Pick v, r so a exceeds the limit.
    let tp = arc_toolpath(/* radius_mm = */ 5.0, /* speed_mm_min = */ 6000.0); // 100 mm/s → a = 2000 mm/s^2
    let c = Contracts { kinematics: Some(KinematicContracts {
        max_acceleration_mm_s2: Some(1000.0), max_junction_velocity_mm_s: None }), ..Contracts::default() };
    let report = verify(&tp, &c);
    assert!(report.findings.iter().any(|f| f.rule == "peak-acceleration" && f.severity == Severity::Error));
}

#[test]
fn junction_over_scv_is_a_junction_velocity_warning() {
    // Two contiguous printing segments with a large speed change.
    let tp = two_segment_junction(/* v0_mm_min = */ 600.0, /* v1_mm_min = */ 6000.0); // Δv = 90 mm/s
    let c = Contracts { kinematics: Some(KinematicContracts {
        max_acceleration_mm_s2: None, max_junction_velocity_mm_s: Some(5.0) }), ..Contracts::default() };
    let report = verify(&tp, &c);
    assert!(report.findings.iter().any(|f| f.rule == "junction-velocity" && f.severity == Severity::Warning));
}

#[test]
fn no_kinematics_means_no_kinematic_findings() {
    let tp = arc_toolpath(5.0, 6000.0);
    let report = verify(&tp, &Contracts::default());
    assert!(!report.findings.iter().any(|f| f.rule == "peak-acceleration" || f.rule == "junction-velocity"));
}
```

(Define the `arc_toolpath` / `two_segment_junction` helpers in the test module using the same `Segment`/arc construction the existing `arc_radius` tests use — match `s.kind = SegmentKind::Arc`, set `center` and `start`/`end`, and `speed = Feedrate::mm_per_min(...)`.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dry-core junction_over_scv_is_a_junction_velocity_warning`
Expected: FAIL — checks not implemented (no such finding).

- [ ] **Step 3: Implement the checks**

In `verify_stream`, add carried junction state alongside the existing `travel_run_length`/`retracted` state:
```rust
    let mut prev_print_end: Option<[Option<Length>; 3]> = None;
    let mut prev_speed_mm_s: Option<f64> = None;
```
Inside the per-segment loop, after the existing contract checks, add (gated on `c.kinematics`):
```rust
        if let Some(kin) = &c.kinematics {
            let is_print = !s.travel && s.length.value() > 0.0 && s.volume.value() > 0.0;
            // Arc centripetal acceleration → Error.
            if let Some(max_a) = kin.max_acceleration_mm_s2 {
                if s.kind == SegmentKind::Arc {
                    if let Some(r) = arc_radius_mm(&s) {           // reuse the radius computation
                        if r > 0.0 {
                            let v = s.speed.value() / 60.0;        // mm/s
                            let a = v * v / r;                     // mm/s^2
                            if a > max_a {
                                push(RuleId::PeakAcceleration, Some(i),
                                    format!("arc centripetal accel {a:.0} mm/s² exceeds max {max_a:.0}"));
                            }
                        }
                    }
                }
            }
            // Junction velocity change → Warning (contiguous printing junctions only).
            if let (Some(max_jv), Some(pv), true) = (kin.max_junction_velocity_mm_s, prev_speed_mm_s, is_print) {
                if junction_contiguous(&prev_print_end, &s.start) {   // end_{i-1} ≈ start_i within 0.1 mm
                    let dv = (s.speed.value() / 60.0 - pv).abs();
                    if dv > max_jv {
                        push(RuleId::JunctionVelocity, Some(i),
                            format!("junction Δv {dv:.1} mm/s exceeds square-corner velocity {max_jv:.1}"));
                    }
                }
            }
            if is_print {
                prev_print_end = Some(s.end);
                prev_speed_mm_s = Some(s.speed.value() / 60.0);
            }
        }
```
Add two small private helpers near `arc_radius_error`:
```rust
/// Arc radius in mm from the segment's start point and centre, or None if not a well-formed arc.
/// (Mirrors the radius computation in `arc_radius_error` / the arc sampler — reuse that exact access.)
fn arc_radius_mm(s: &Segment) -> Option<f64> { /* match how arc_radius_error reads center/start */ }

/// True when the previous printing segment's end ≈ this segment's start (within 0.1 mm in X/Y/Z).
fn junction_contiguous(prev_end: &Option<[Option<Length>; 3]>, start: &[Option<Length>; 3]) -> bool {
    let Some(pe) = prev_end else { return false };
    (0..3).all(|k| match (pe[k], start[k]) {
        (Some(a), Some(b)) => (a.value() - b.value()).abs() <= 0.1,
        (None, None) => true,
        _ => false,
    })
}
```
**Implementer note:** fill `arc_radius_mm` by reusing the exact `center`/`start` field access already in `arc_radius_error` (do not invent field names); it computes `radius = (sx - cx).hypot(sy - cy)` (see `verify.rs:351`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p dry-core verify` then `cargo test -p dry-core`. `cargo clippy -p dry-core --all-targets -- -D warnings`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/verify.rs
git commit -m "feat(core): enforce peak-acceleration (arc) + junction-velocity in verify"
```

---

## Phase 2 — `dry import-printer-cfg` (Klipper → profile)

### Task 3: `profile::import_klipper` (`dry-core`)

**Files:**
- Create: `crates/core/src/profile/klipper.rs`
- Modify: `crates/core/src/profile.rs` (make `profile` a module dir OR add `mod klipper; pub use klipper::*;` — follow whichever module style the file uses) and `crates/core/src/lib.rs` (re-export)
- Test: inline tests in `klipper.rs`

**Interfaces:**
- Produces: `pub struct KlipperImportWarning { pub field: String, pub message: String }`; `pub enum KlipperImportError { NotKlipper, Parse(String) }` (+ `Display`/`Error`); `pub fn import_klipper(text: &str) -> Result<(Profile, Vec<KlipperImportWarning>), KlipperImportError>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    const CFG: &str = "\
[printer]\nkinematics: corexy\nmax_velocity: 300\nmax_accel: 3000\nsquare_corner_velocity: 5.0\n\
[stepper_x]\nposition_min: 0\nposition_max: 250\n\
[stepper_y]\nposition_max: 210\n\
[stepper_z]\nposition_max: 210\n\
[extruder]\nnozzle_diameter: 0.4\nfilament_diameter: 1.75\nmin_extrude_temp: 170\n\
[firmware_retraction]\nretract_length: 0.5\nretract_speed: 35\n";

    #[test]
    fn maps_clean_kinematic_fields_exactly() {
        let (p, _w) = import_klipper(CFG).unwrap();
        assert_eq!(p.firmware.flavor.as_deref(), Some("klipper"));
        let k = p.machine.kinematics.unwrap();
        assert_eq!(k.max_acceleration_mm_s2, Some(3000.0));
        assert_eq!(k.max_junction_velocity_mm_s, Some(5.0));
        assert_eq!(p.material.filament_diameter, Some(1.75));
        assert_eq!(p.material.min_nozzle_temperature_c, Some(170.0));
        // retract_speed 35 mm/s → 2100 mm/min
        assert_eq!(p.process.max_retraction_speed, Some(2100.0));
        p.validate().expect("imported profile validates");
    }

    #[test]
    fn non_klipper_input_errors() {
        assert!(matches!(import_klipper("hello world\n"), Err(KlipperImportError::NotKlipper)));
    }

    #[test]
    fn feedrate_range_is_omitted_with_a_warning() {
        let (p, w) = import_klipper(CFG).unwrap();
        assert!(p.machine.feedrate_range.is_none());
        assert!(w.iter().any(|x| x.field == "machine.feedrate_range"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p dry-core klipper`
Expected: FAIL — `import_klipper` not defined.

- [ ] **Step 3: Implement the parser + mapping**

`crates/core/src/profile/klipper.rs` — a line scanner that builds a `BTreeMap<String, BTreeMap<String,String>>` (section → key → value): skip blank / `#`-comment lines; `[section]` opens a section (lowercased; take the first token so `[gcode_macro X]` → `gcode_macro`); `key: value` or `key = value` split on the first `:`/`=`, trimmed. Then map per the table (parse numbers with `.parse::<f64>()`, skip+warn on failure):

- `firmware.flavor = Some("klipper")` always.
- `[printer] max_accel` → `machine.kinematics.max_acceleration_mm_s2`; `square_corner_velocity` → `max_junction_velocity_mm_s` (build `MachineKinematics`, set `machine.kinematics` only if at least one present).
- `[extruder] filament_diameter` → `material.filament_diameter`; `min_extrude_temp` → `material.min_nozzle_temperature_c`; `nozzle_diameter` → `process.line_width` + warn `"process.line_width derived from nozzle_diameter — review"`.
- `[stepper_x/y/z] position_min`(default 0)/`position_max` → `machine.build_volume` (`[[xmin,xmax],[ymin,ymax],[zmin,zmax]]`), only if x & y maxes present; warn `"machine.build_volume approximated from stepper position limits"`. If `[printer] kinematics` is `delta`/`rotary_delta`, skip `build_volume` and warn.
- `[firmware_retraction] retract_length` → `process.max_retraction_distance` + warn; `retract_speed` (mm/s) ×60 → `process.max_retraction_speed` (mm/min) + warn.
- **Omit** `feedrate_range` → warn `"machine.feedrate_range omitted (no Klipper lower-bound source) — add manually"`.
- **Absent** `material.max_volumetric_flow_mm3_s` → prominent warn `"material.max_volumetric_flow_mm3_s not in printer.cfg — add from your hotend calibration (most useful review-gcode safety contract)"`.
- `[input_shaper]` present → warn `"[input_shaper] ignored — deferred to a future machine-model release"`; `[extruder] pressure_advance` present → same style warn.
- More than one `[extruder]`/`[extruder1]` → warn `"only the first extruder imported"`. `[include ...]` present → warn `"[include] not followed"`.
- **NotKlipper** error when there is no `[printer]` section.

Build the `Profile` with `..Default::default()` for everything unmapped; ensure `validate()` passes. Define `KlipperImportWarning`/`KlipperImportError` (+ `Display`/`Error`). Wire `mod klipper;` + `pub use` and re-export `import_klipper`, `KlipperImportWarning`, `KlipperImportError` from `lib.rs`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p dry-core klipper` then `cargo test -p dry-core`; `cargo clippy -p dry-core --all-targets -- -D warnings`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/profile/ crates/core/src/profile.rs crates/core/src/lib.rs
git commit -m "feat(core): import_klipper — Klipper printer.cfg → dry profile (hand-rolled INI)"
```

### Task 4: `dry import-printer-cfg` CLI command + golden

**Files:**
- Modify: `crates/cli/src/main.rs` (`Cmd::ImportPrinterCfg` variant + handler, mirroring `Cmd::ImportGcode` at `main.rs:110` / `:574`)
- Create: `conformance/profiles/klipper_voron.cfg`, `conformance/profiles/klipper_voron.expected.json`
- Test: `crates/cli/tests/cli.rs` e2e + a `dry-core` golden test (or fold the golden into the e2e)

**Interfaces:** Consumes `dry_core::{import_klipper, KlipperImportWarning, KlipperImportError, Profile}`.

- [ ] **Step 1: Add the CLI variant + handler**

In `enum Cmd`:
```rust
    /// Import a Klipper printer.cfg into a dry machine/material profile (kinematics, retraction, build volume).
    ImportPrinterCfg {
        file: String,
        #[arg(long)]
        out: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
```
Handler (mirror `Cmd::ImportGcode`'s file-open + serialize + `--out`/stdout pattern):
```rust
        Cmd::ImportPrinterCfg { file, out, name } => {
            let text = fs::read_to_string(&file).unwrap_or_else(|e| die(format!("cannot read {file}: {e}")));
            let (mut profile, warnings) = dry_core::import_klipper(&text)
                .unwrap_or_else(|e| die(format!("cannot import {file}: {e}")));
            if let Some(n) = name { profile.name = n; }            // else default handled below
            for w in &warnings { eprintln!("warning: {} — {}", w.field, w.message); }
            let json = serde_json::to_string_pretty(&profile).unwrap() + "\n";
            match out {
                Some(path) => fs::write(&path, json).unwrap_or_else(|e| die(format!("cannot write {path}: {e}"))),
                None => print!("{json}"),
            }
            ExitCode::SUCCESS
        }
```
(If `Profile.name` defaulting from the file stem is wanted, set it in `import_klipper` or here from `Path::new(&file).file_stem()`.)

- [ ] **Step 2: Create fixture + generate the golden**

Create `conformance/profiles/klipper_voron.cfg` (a realistic CoreXY config exercising the mapped sections — `[printer]` with `max_accel`/`square_corner_velocity`/`max_velocity`, `[stepper_x/y/z]`, `[extruder]`, `[firmware_retraction]`, plus an `[input_shaper]` to exercise the deferred-warning path).
Generate the expected profile: `cargo run -- import-printer-cfg conformance/profiles/klipper_voron.cfg --name voron > conformance/profiles/klipper_voron.expected.json` (strip/keep trailing newline consistently). Eyeball it: `machine.kinematics` populated from `max_accel`/`square_corner_velocity`, `feedrate_range` absent, `max_volumetric_flow` absent.

- [ ] **Step 3: Write the golden + e2e test**

Add to `crates/cli/tests/cli.rs`:
```rust
#[test]
fn import_printer_cfg_matches_golden() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/profiles");
    let out = Command::new(bin())
        .args(["import-printer-cfg", dir.join("klipper_voron.cfg").to_str().unwrap(), "--name", "voron"])
        .output().unwrap();
    assert!(out.status.success());
    let got: Value = serde_json::from_slice(&out.stdout).expect("valid profile JSON");
    let want: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("klipper_voron.expected.json")).unwrap()).unwrap();
    assert_eq!(got, want, "imported profile must match the golden");
    assert_eq!(got["machine"]["kinematics"]["max_acceleration_mm_s2"], 3000.0); // sanity vs the fixture's max_accel
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test` (full default suite incl. the e2e) → PASS; `cargo clippy --all-targets -- -D warnings`; `cargo fmt`.

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/main.rs conformance/profiles/klipper_voron.cfg conformance/profiles/klipper_voron.expected.json crates/cli/tests/cli.rs
git commit -m "feat(cli): dry import-printer-cfg + golden"
```

---

## Phase 3 — expose `machine.kinematics` on wasm / PyO3 / TS

### Task 5: wasm `resolve_balanced_ir` + `resolve_verify` kinematics

**Files:** Modify `crates/wasm/src/lib.rs`; Test: the wasm crate's existing test pattern (or a `dry-core`-level assertion of the underlying call).

**Interfaces:** Produces wasm exports `resolve_balanced_ir(ops_json, params_json, kinematics_json: &str) -> Result<String, JsError>` and an extended `resolve_verify(..., kinematics_json: &str)` (trailing param). Uses `dry_core::{balanced_pipeline, safe_pipeline, MachineKinematics, Contracts, KinematicContracts, verify}`.

- [ ] **Step 1: Add a kinematics parse helper + the balanced export**

Add near `build_range`:
```rust
/// Parse the optional `kinematics_json` boundary string into `MachineKinematics`. Empty → None.
/// A non-empty string that fails to parse is a clear JsError (never a panic).
fn parse_kinematics(kinematics_json: &str) -> Result<Option<dry_core::MachineKinematics>, JsError> {
    let s = kinematics_json.trim();
    if s.is_empty() { return Ok(None); }
    serde_json::from_str::<dry_core::MachineKinematics>(s)
        .map(Some)
        .map_err(|e| JsError::new(&format!("invalid kinematics_json: {e}")))
}
```
Add:
```rust
#[wasm_bindgen]
pub fn resolve_balanced_ir(ops_json: &str, params_json: &str, kinematics_json: &str) -> Result<String, JsError> {
    let (d, p) = parse(ops_json, params_json)?;
    let tp = resolve_checked(&d, &p).map_err(|e| JsError::new(&e.to_string()))?;
    let out = match parse_kinematics(kinematics_json)? {
        Some(k) => dry_core::balanced_pipeline(&tp, Some(&k)),
        None => dry_core::safe_pipeline(&tp),
    };
    Ok(out.to_json())
}
```
Extend `resolve_verify` with a trailing `kinematics_json: &str` param and set `kinematics: parse_kinematics(kinematics_json)?.map(|k| KinematicContracts { max_acceleration_mm_s2: k.max_acceleration_mm_s2, max_junction_velocity_mm_s: k.max_junction_velocity_mm_s })` in the `Contracts` it builds. Doc-comment the `Kinematics` (5-axis) vs `MachineKinematics` (profile) name distinction.

- [ ] **Step 2: Build + test**

Run: `cargo build -p dry-wasm --target wasm32-unknown-unknown` (or the crate's documented build) and the wasm crate's test command. Confirm it compiles and `resolve_balanced_ir`/extended `resolve_verify` are exported.
Expected: green build. (If the wasm crate has unit tests, add one asserting `parse_kinematics("")` → None and a valid JSON → Some.)

- [ ] **Step 3: Commit**

```bash
git add crates/wasm/src/lib.rs
git commit -m "feat(wasm): resolve_balanced_ir + kinematics_json on resolve_verify"
```

### Task 6: PyO3 mirror

**Files:** Modify `py/src/lib.rs`; Test: `py/`'s existing pytest suite.

**Interfaces:** Mirror Task 5: `resolve_balanced_ir(ops_json, params_json, kinematics_json=None)` and `resolve_verify(..., kinematics_json=None)` (PyO3 signature with `Option<&str>`/default None). Same `parse_kinematics` logic returning `PyErr` on bad JSON.

- [ ] **Step 1: Add the py functions** mirroring `py/src/lib.rs:112` (`resolve_optimized_ir`) and `:181` (`resolve_verify`), using `Option<&str>` for `kinematics_json` (None/empty → safe/none). Register them in the module (`#[pymodule]`).
- [ ] **Step 2: Test** — add a pytest in `py/`'s test dir: a kinematics JSON drives `resolve_balanced_ir` (differs from `safe`), and `resolve_verify` with a tight `max_acceleration` surfaces a `peak-acceleration` finding. Run the py test suite (`maturin develop` + `pytest`, per the repo's py CI).
- [ ] **Step 3: Commit** `git commit -m "feat(py): resolve_balanced_ir + kinematics_json on resolve_verify"`

### Task 7: TS SDK wrappers + drift test

**Files:** Modify `sdk/ts/src/engine.ts` (+ `index.ts` exports); Create a `MachineKinematics` interface; Test: `sdk/ts/test/kinematics.test.ts`.

**Interfaces:** Consumes the wasm exports from Task 5. Produces TS `resolveBalancedIr(ops, params, kinematics?)` + `resolveVerify(..., kinematics?)` and a `MachineKinematics` interface matching the Rust serde shape.

- [ ] **Step 1: Read `sdk/ts/src/engine.ts`** to match its existing wasm-call + serialization convention (how `resolveVerify`/`resolveOptimizedIr` are wrapped, snake_case vs camelCase fields). Add a `MachineKinematics` interface mirroring the Rust field names exactly as serialized (`max_acceleration_mm_s2`, `max_junction_velocity_mm_s` — confirm against the existing TS↔Rust field convention in the file).
- [ ] **Step 2: Add wrappers** `resolveBalancedIr(ops, params, kinematics?: MachineKinematics)` and extend `resolveVerify` with an optional `kinematics?` arg — serialize `kinematics` to JSON (or `""` when undefined) and pass as `kinematics_json` to the wasm fn. Export from `index.ts`.
- [ ] **Step 3: Drift test** `sdk/ts/test/kinematics.test.ts` (mirror `sdk/ts/test/verify-input.test.ts`): a `MachineKinematics` round-trips through `resolveBalancedIr` and produces the same IR as the Rust engine for the same input (delegation identity), and `resolveVerify` with a tight `max_acceleration` surfaces a `peak-acceleration` finding. Run `cd sdk/ts && npm test`.
- [ ] **Step 4: Commit** `git commit -m "feat(ts): MachineKinematics + resolveBalancedIr/resolveVerify kinematics, with drift test"`

---

## Phase 4 — docs

### Task 8: docs + CHANGELOG

**Files:** `docs/11-profiles-and-reports.md` (the two new rules in the catalog section + `machine.kinematics`), `docs/15-cli-cookbook.md` (`import-printer-cfg` recipe + the balanced/verify SDK note), `docs/05-product-directions.md` (machine-model v2 progress; PA/IS still deferred), `CHANGELOG.md` `[Unreleased]`.

- [ ] **Step 1** — document the `peak-acceleration` (Error) and `junction-velocity` (Warning) rules and the `machine.kinematics` profile block in `docs/11`; add an `import-printer-cfg` cookbook recipe (incl. the `→ review-gcode --profile` / `→ rewrite-gcode --mode balanced` workflow) and a note on the SDK `kinematics_json` surface in `docs/15`; note machine-model-v2 progress (peak-accel + Klipper import + SDK kinematics shipped; PA/input-shaper deferred) in `docs/05`.
- [ ] **Step 2** — CHANGELOG bullet:
```
- Machine-model v2 (kinematics, end-to-end): a `peak-acceleration` verifier rule (arc centripetal,
  Error) + `junction-velocity` rule (Δv, Warning) gated on a profile's `machine.kinematics`; a new
  `dry import-printer-cfg` that derives a profile from a Klipper printer.cfg; and `machine.kinematics`
  exposed on the wasm/PyO3/TS SDKs (`resolve_balanced_ir` + `kinematics_json` on `resolve_verify`).
  PA / input-shaper modeling remains deferred.
```
- [ ] **Step 3** — final check: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`. Commit `git commit -m "docs+changelog: kinematics cluster (machine-model v2)"`.

---

## Self-Review

**Spec coverage:** Phase 1 rule (arc=Error / junction=Warning) → Tasks 1–2; `Profile::contracts()` mapping → Task 1; Klipper import (mapping table, warnings, defaults, errors) → Task 3; CLI `import-printer-cfg` + golden → Task 4; wasm exposure → Task 5; PyO3 → Task 6; TS interface + drift test → Task 7; docs → Task 8. PA/input-shaper correctly absent (deferred). Determinism: Phase-1 catalog/report goldens (Task 1 note), Phase-2 profile golden (Task 4), Phase-3 TS drift test (Task 7).

**Placeholder scan:** the one deliberate "fill from the existing pattern" marker is `arc_radius_mm` in Task 2 — it is NOT left to invention; the implementer-note pins it to `arc_radius_error`'s exact `center`/`start` access and the `verify.rs:351` radius formula (the one field-access detail not in this plan's context). The golden `expected.json` is generated-then-committed (Task 4) — standard golden workflow. All other steps carry real code.

**Type consistency:** `KinematicContracts { max_acceleration_mm_s2, max_junction_velocity_mm_s }` defined in Task 1, consumed in Tasks 2/5/6; `RuleId::{PeakAcceleration, JunctionVelocity}` + wire strings `"peak-acceleration"`/`"junction-velocity"` consistent across Tasks 1, 2, 6, 7; `import_klipper` signature `(text) -> Result<(Profile, Vec<KlipperImportWarning>), KlipperImportError>` consistent across Tasks 3–4; `resolve_balanced_ir`/`kinematics_json` naming consistent across Tasks 5–7; `parse_kinematics` (wasm) mirrored in py (Task 6). The locked decisions (two RuleIds for the severity split; `kinematics_json` string) are reflected throughout and noted in Global Constraints.
