# Klipper `printer.cfg` import → dry profile — brainstorm

**Date:** 2026-06-30
**Command in scope:** `dry import-printer-cfg <printer.cfg>`
**Output:** a dry `Profile` JSON (version 1)

---

## 1. Problem / motivation

Dry profiles are currently hand-written JSON. Users who want to use `dry review-gcode`, `dry
rewrite-gcode --mode balanced`, or `dry explain --profile` with machine-accurate limits must author
the profile themselves, which requires knowing the dry schema and manually translating printer
settings.

Klipper users already have their printer's authoritative specification in `printer.cfg`: axis limits,
extruder geometry, firmware retraction settings, and kinematics-tuning parameters are all present
(often verbatim from a calibration session). The logical first-party move is a `dry
import-printer-cfg` command that reads that file and emits a ready-to-use dry profile JSON — seeding
`machine.kinematics`, `material`, and process defaults from real hardware data rather than guesses.

This matters especially for the `balanced` optimisation pipeline, which consumes
`machine.kinematics.max_acceleration_mm_s2` and `machine.kinematics.max_junction_velocity_mm_s` to
set arc centripetal limits and per-junction feedrate caps (see `Profile::emit_params`,
`crates/core/src/profile.rs` lines 109-123, and `docs/11-profiles-and-reports.md` §1). Without a
real profile these fields are absent and the pipeline falls back to built-in defaults (500 mm/s²,
no junction cap). Klipper's `printer.cfg` is the cheapest reliable source of these numbers.

Current state: `Profile::from_json` in `crates/core/src/profile.rs` is the only intake path. There
is no importer from any printer configuration format. All three conformance profiles under
`conformance/profiles/` are Cura-derived.

---

## 2. What maps cleanly vs not

Klipper's `.cfg` is an INI-ish format: `[section]` headers, `key: value` pairs (`:` or `=`
separator), `#` comments, multi-line values via leading whitespace. The relevant sections are
`[printer]`, `[stepper_x/y/z]`, `[extruder]`, `[firmware_retraction]`, and optionally
`[input_shaper]` and `[gcode_macro ...]`.

The target types from `crates/core/src/profile.rs`:

- `Profile` → `{ version, name, firmware: FirmwareProfile, machine: MachineProfile,
  material: MaterialProfile, process: ProcessProfile, start_gcode, end_gcode }`
- `MachineProfile` → `{ build_volume: Option<[[f64;2];3]>, feedrate_range: Option<[f64;2]>,
  kinematics: Option<MachineKinematics> }`
- `MachineKinematics` → `{ max_acceleration_mm_s2: Option<f64>,
  max_junction_velocity_mm_s: Option<f64> }`
- `MaterialProfile` → `{ filament_diameter, max_volumetric_flow_mm3_s,
  min_nozzle_temperature_c }`
- `ProcessProfile` → `{ line_width, layer_height, monotonic_z, max_retraction_distance,
  max_retraction_speed, max_travel_without_retraction, first_layer_height_range,
  first_layer_speed_range }`

### Mapping table

| Klipper field | dry Profile field | Quality | Notes |
|---|---|---|---|
| *(source is a `.cfg` file)* | `firmware.flavor` = `"klipper"` | **exact** | Always set; trivially derived. |
| `[printer] max_accel` (mm/s²) | `machine.kinematics.max_acceleration_mm_s2` | **exact** | Same units. Direct transcription. |
| `[printer] square_corner_velocity` (mm/s) | `machine.kinematics.max_junction_velocity_mm_s` | **exact** | SCV is Klipper's per-junction velocity limit — the identical concept as dry's field. |
| `[printer] max_velocity` (mm/s) × 60 | `machine.feedrate_range[1]` (mm/min) | **approximate** | Unit conversion required. The lower bound has no Klipper source; emit as `[1.0, max_velocity*60]` or omit `feedrate_range` and let the user fill it in. |
| `[stepper_x] position_min` / `position_max` | `machine.build_volume[0]` | **approximate** | Works for Cartesian; for CoreXY the X stepper maps to X but position may be offset; delta printers have no per-axis steppers in this form. Must handle absent `position_min` (Klipper default = 0). |
| `[stepper_y] position_min` / `position_max` | `machine.build_volume[1]` | **approximate** | Same caveats as X. |
| `[stepper_z] position_min` / `position_max` | `machine.build_volume[2]` | **approximate** | `position_min` is often `0` or a small negative value from a `z_offset`; `position_max` is the useful limit. |
| `[extruder] filament_diameter` (mm) | `material.filament_diameter` | **exact** | Same name, same units, same meaning. |
| `[extruder] min_extrude_temp` (°C) | `material.min_nozzle_temperature_c` | **exact** | Same concept; Klipper default is 170°C if absent (omit the field rather than assume). |
| `[extruder] nozzle_diameter` (mm) | `process.line_width` (approximate) | **lossy** | Line width is a slicer-side heuristic (typically nozzle × 1.0–1.2). Using `nozzle_diameter` directly is a lower bound; flag as an approximation in the emitted profile comment or `name`. |
| `[firmware_retraction] retract_length` (mm) | `process.max_retraction_distance` | **approximate** | Klipper's value is the *configured default*, not a safety ceiling. The dry field is a contract limit. Semantically the same at single-extruder setups. |
| `[firmware_retraction] retract_speed` (mm/s) × 60 | `process.max_retraction_speed` (mm/min) | **approximate** | Unit conversion × 60. Same semantic caveat as `retract_length`. |
| `[printer] kinematics` (string: cartesian/corexy/delta/…) | *(no dry field)* | **no mapping** | The kinematic type is not modelled in `MachineKinematics` v1. Record in a warning; could populate `name` or a comment field. |
| `[printer] max_z_velocity` / `max_z_accel` | *(no dry field)* | **no mapping** | Z-axis limits are not separately modelled. Deferred. |
| `[input_shaper] shaper_type_x/y`, `shaper_freq_x/y` | *(no dry field)* | **no mapping** | Explicitly deferred in `docs/11` §1: "Pressure-advance and input-shaper models are deliberately out of scope for v1." |
| `[extruder] pressure_advance` | *(no dry field)* | **no mapping** | Explicitly deferred (same cite). |
| `[extruder] max_extrude_cross_section` (mm²) | *(close to `max_volumetric_flow_mm3_s`)* | **no mapping** | Different dimension (mm² area vs mm³/s flow); not a flow ceiling. Omit v1. |
| `[gcode_macro START_PRINT]` / `[gcode_macro END_PRINT]` body | `start_gcode` / `end_gcode` | **lossy** | Klipper macro bodies are Jinja2-templated (`{%if ...%}`, `{printer.extruder.temperature}`). Raw extraction may not be valid G-code. Extract as-is with a warning; user must review. |
| `process.layer_height` | *(not in printer.cfg)* | **cannot derive** | Slicer concern. Leave absent. |
| `material.max_volumetric_flow_mm3_s` | *(not in printer.cfg)* | **cannot derive** | Hotend/material property. Klipper has no standard field. Leave absent. |
| `process.monotonic_z`, `process.first_layer_*` | *(not in printer.cfg)* | **cannot derive** | Process settings, not machine settings. Leave absent. |
| `process.max_travel_without_retraction` | *(not in printer.cfg)* | **cannot derive** | Process/slicer setting. Leave absent. |

**Summary:** Seven fields map cleanly (exact or unit-conversion-only). Five fields map approximately
with semantic caveats (Klipper's configured defaults ≠ dry's contract ceilings, or kinematics-type
assumptions). Nine dry profile fields cannot be derived from `printer.cfg` and must stay absent (the
profile is partial by design). The biggest semantic gap is that Klipper's `firmware_retraction`
values are *defaults injected into G-code*, not enforced ceilings — importing them as dry contract
limits is conservative and correct for most users but may reject valid G-code that overrides them.

---

## 3. Approaches

### Approach A: CLI-only parser in `crates/cli`

Implement the INI parser and mapping logic directly in `crates/cli/src/main.rs` (or a new
`crates/cli/src/import_printer_cfg.rs` module). The `Cmd::ImportPrinterCfg` arm reads the file,
builds a `Profile`, and serializes it with `serde_json`.

**Pros:** No new dependencies, no changes to `dry-core`, fast to ship, isolated from the public API.

**Cons:** Logic is not unit-testable without invoking the CLI binary; not accessible to Python or
WASM bindings; diverges from the `Profile::from_json` / library-first pattern that `dry-core` uses
for all other profile operations.

### Approach B: `profile::import_klipper` in `dry-core` (recommended)

Add a new public function in `crates/core/src/profile.rs`:

```rust
pub fn import_klipper(text: &str) -> Result<(Profile, Vec<KlipperImportWarning>), KlipperImportError>
```

where `KlipperImportWarning` is a small `{ field: String, message: String }` struct carrying
per-field caveats (e.g. "line_width derived from nozzle_diameter: 0.4 mm — review before use").
The returned `Profile` always passes `Profile::validate()`. The CLI `Cmd::ImportPrinterCfg` arm
calls this function and renders warnings to stderr.

**INI parsing:** hand-roll a 40-line line scanner. Klipper's format is simple enough: skip `#`
comments, match `[section header]`, split `key: value` on the first `:` or `=`, strip whitespace.
No multi-level nesting. This avoids any new crate dependency (important given `dry-core`'s lean
dependency policy visible in the workspace structure).

**Pros:** Consistent with `Profile::from_json` pattern; unit-testable with `#[test]` on raw strings;
accessible to Python/WASM bindings in the future; warnings carry structured metadata.

**Cons:** Slightly more surface in `dry-core`; the INI parser is hand-rolled (acceptable for this
scope).

### Approach C: Separate `crates/klipper-import` crate

A new workspace member with its own `Cargo.toml`.

**Pros:** Cleanest isolation; can carry a richer INI crate dependency without touching `dry-core`.

**Cons:** Overkill for v1 — the logic is <200 lines; adds workspace/CI overhead; delays shipping.

**Recommendation: Approach B.** The function fits naturally beside `Profile::from_json`, the
hand-rolled parser is trivial, and the warning struct will be immediately useful for the CLI output
and eventually for WASM.

---

## 4. CLI surface

```
dry import-printer-cfg <file> [--out <path>] [--name <profile-name>]
```

- `<file>`: path to a Klipper `printer.cfg` (or any `.cfg` include fragment).
- `--name <profile-name>`: sets `profile.name` in the output; defaults to the file's stem (e.g.
  `printer` for `printer.cfg`) or a synthesized string like `"klipper-import"`.
- `--out <path>`: write the profile JSON to this file instead of stdout.

**Output (stdout or `--out`):** a `Profile` JSON serialized with `serde_json::to_string_pretty`,
with a trailing newline — identical pattern to `dry import-gcode --out`.

**Stderr:** one line per `KlipperImportWarning`, prefixed with `warning:`. Example:
```
warning: process.line_width derived from nozzle_diameter (0.4 mm) — review; actual line width depends on slicer settings
warning: [printer] kinematics = corexy — no dry field; verify build_volume axis mapping manually
warning: [firmware_retraction] retract_length = 0.5 mm imported as process.max_retraction_distance contract ceiling — review if G-code overrides this
```

**Error handling:**
- File not found / unreadable → `error: cannot read <file>: <io-error>`, exit 2.
- No `[printer]` section found → `error: <file> does not look like a Klipper printer.cfg (no [printer] section)`, exit 2. This gates against accidentally feeding a non-Klipper file.
- Malformed numeric values → field silently skipped with a `warning:` line; the profile is emitted with that field absent.

**Post-import workflow (suggested in `--help` long description):**

```
dry import-printer-cfg printer.cfg --name voron24 --out voron24.json
dry review-gcode part.gcode --profile voron24.json
dry rewrite-gcode part.gcode --profile voron24.json --mode balanced -o part.balanced.gcode
```

---

## 5. Components and data flow

All type names are grounded in `crates/core/src/profile.rs` and `crates/cli/src/main.rs`.

```
printer.cfg (text)
  │
  ▼
profile::import_klipper(text: &str)         [crates/core/src/profile.rs]
  │  hand-rolled INI scanner
  │  section dispatch: [printer], [stepper_*], [extruder], [firmware_retraction], [gcode_macro]
  │  field-by-field mapping → Profile builder
  │
  ├─► Profile { firmware: FirmwareProfile { flavor: Some("klipper") },
  │             machine: MachineProfile {
  │               build_volume: Option<[[f64;2];3]>,     ← from stepper_x/y/z
  │               feedrate_range: Option<[f64;2]>,       ← from max_velocity × 60
  │               kinematics: Some(MachineKinematics {
  │                 max_acceleration_mm_s2,              ← from max_accel
  │                 max_junction_velocity_mm_s,          ← from square_corner_velocity
  │               }),
  │             },
  │             material: MaterialProfile {
  │               filament_diameter,                     ← from [extruder]
  │               min_nozzle_temperature_c,              ← from min_extrude_temp
  │               max_volumetric_flow_mm3_s: None,       ← cannot derive
  │             },
  │             process: ProcessProfile {
  │               line_width,                            ← from nozzle_diameter (lossy)
  │               max_retraction_distance,               ← from retract_length
  │               max_retraction_speed,                  ← from retract_speed × 60
  │               ..Default::default()                   ← rest stays None/false
  │             },
  │             start_gcode, end_gcode,                  ← from [gcode_macro] bodies (lossy)
  │           }
  │
  └─► Vec<KlipperImportWarning>   (emitted to stderr by CLI)
  │
  ▼
Profile::validate()                          [existing, crates/core/src/profile.rs]
  (always called before serialization; import_klipper must produce a valid Profile)
  │
  ▼
serde_json::to_string_pretty(&profile)
  │
  ▼
stdout / --out file
```

The `Cmd::ImportPrinterCfg` arm in `crates/cli/src/main.rs` follows the same skeleton as
`Cmd::ImportGcode`: open file, call the core function, serialize, write stdout or `--out`, print
warnings to stderr.

---

## 6. Scope / YAGNI — what v1 covers vs defers

### v1 covers

- `[printer]` section: `max_accel`, `square_corner_velocity`, `max_velocity`, `kinematics` (type
  extracted for warning, not mapped).
- `[stepper_x]`, `[stepper_y]`, `[stepper_z]`: `position_min`, `position_max` → `build_volume`.
  Only Cartesian/CoreXY are treated as reliable (delta warned).
- `[extruder]` (first extruder only): `filament_diameter`, `min_extrude_temp`, `nozzle_diameter`.
- `[firmware_retraction]`: `retract_length`, `retract_speed`.
- `firmware.flavor` = `"klipper"` always.
- `--name` override for `profile.name`.
- Structured warnings for every lossy/approximate mapping.

### v1 defers

- **`[input_shaper]`**: explicitly out of scope per `docs/11` §1; emit a no-op warning.
- **`[extruder] pressure_advance`**: explicitly out of scope; emit a no-op warning.
- **Multi-extruder configs** (`[extruder1]`, `[extruder stepper_*]`): import first extruder only;
  warn if additional extruder sections are found.
- **Delta kinematics** (`[printer] kinematics: delta`): `build_volume` from stepper sections
  doesn't apply; warn and skip build_volume; `max_accel` and `square_corner_velocity` still import
  cleanly.
- **`[gcode_macro]` extraction**: optionally emit raw macro bodies as `start_gcode`/`end_gcode`
  behind a `--include-macros` flag (off by default; Jinja2 templates are not valid G-code and will
  confuse `dry import-gcode`).
- **`max_volumetric_flow_mm3_s`**: not in `printer.cfg`; leave absent; document that the user must
  supply it from hotend calibration.
- **`process.layer_height`**, **`process.monotonic_z`**, **`process.first_layer_*`**: slicer
  concerns, not derivable from printer.cfg.
- **Includes** (`[include *.cfg]`): v1 does not follow `[include]` directives; warn and ignore.
- **`minimum_cruise_ratio`** (newer Klipper): no dry field; ignore.

---

## 7. Open questions for the user

1. **Which Klipper fields matter most?** The `max_accel` + `square_corner_velocity` mapping enables
   `--mode balanced` — is that the primary use case, or is `build_volume` + `feedrate_range`
   (for verifier contracts in `dry review-gcode`) equally important? This determines which fields get
   the most careful handling and testing.

2. **`feedrate_range` lower bound**: the command currently emits `feedrate_range = [1.0,
   max_velocity * 60]` as a placeholder lower bound, or omits the field entirely. What is the user
   preference — a `[1.0, N]` range that passes validation, or no `feedrate_range` at all and a
   warning asking the user to add it manually?

3. **Delta and IDEX kinematics**: should v1 refuse to import `build_volume` for delta printers
   (warn + leave absent) or attempt a rough approximation from `[stepper_a/b/c] position_max`?
   Delta has an arm_length-derived radius; the approximation is lossy enough it may do more harm
   than good.

4. **Multi-extruder**: should v1 silently import only the first `[extruder]` section, or error out
   with an explicit "multi-extruder not yet supported" message? (The latter is more honest but may
   frustrate IDEX Voron Trident users.)

5. **`start_gcode` / `end_gcode` from macros**: should `--include-macros` be included in v1 scope,
   or always deferred? The raw Jinja2 bodies are dangerous to auto-extract since they break
   `dry import-gcode`. An explicit opt-in flag (`--include-macros`) is the least-harm option.

6. **Material and hotend fields**: `max_volumetric_flow_mm3_s` is the single most useful safety
   contract for `dry review-gcode`. Since it cannot be derived from `printer.cfg`, should the
   command emit a prominent stderr note directing the user to add it manually (e.g.  "add
   `material.max_volumetric_flow_mm3_s` from your hotend calibration")? Or is a partial profile
   with that field absent an acceptable v1 deliverable?

7. **Parser placement**: the recommendation is Approach B (`profile::import_klipper` in
   `dry-core`). Does this align with the dependency policy? The hand-rolled INI scanner adds ~80
   lines to `profile.rs`. Alternatively, a new `profile/klipper.rs` submodule keeps things tidy
   without a new crate.
