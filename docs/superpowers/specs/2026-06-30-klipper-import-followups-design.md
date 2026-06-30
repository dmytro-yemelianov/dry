# `dry import-printer-cfg` — follow-ups: delta, multi-extruder, macros

**Date:** 2026-06-30
**Status:** Approved design, ready for implementation
**Branch:** `feat/klipper-import-followups`
**Exploration input:** `docs/superpowers/specs/2026-06-30-klipper-import-brainstorm.md` (§6 "v1 defers")

## Problem

`dry import-printer-cfg` (v1, in `crates/core/src/profile/klipper.rs`) imports a Klipper
`printer.cfg` into a dry `Profile`. Three capabilities were explicitly deferred in the v1 brainstorm
(§6 "v1 defers") and are now in scope:

1. **Delta build_volume** — `klipper.rs:205` skips `build_volume` entirely for
   `kinematics: delta`/`rotary_delta` (deltas have no per-axis `[stepper_x/y/z] position_max`), so
   delta users get no envelope at all.
2. **Multi-extruder** — `klipper.rs:239` imports only the first `[extruder]` and warns
   "only the first extruder imported"; IDEX/toolchanger limits from `[extruder1]`, `[extruder2]`, …
   are dropped.
3. **`start_gcode`/`end_gcode` from macros** — Klipper `[gcode_macro …]` bodies are never extracted,
   so an imported profile has no start/end procedure even when the printer.cfg defines one.

All three are additive: the v1 happy path (Cartesian/CoreXY, single extruder, no macros) must emit a
**byte-identical** profile to today, so the existing `klipper_voron.expected.json` golden is unchanged.

## Decisions (resolved with the user, 2026-06-30)

1. **Delta → approximate from `print_radius`.** When kinematics is delta/rotary_delta and a printable
   radius is available, emit a bounding box `[[-R, R], [-R, R], [z_lo, z_hi]]`, loudly flagged as a
   bounding-box approximation of a cylindrical volume. Fall back to skip+warn when no radius source.
2. **Multi-extruder → conservative merge.** Fold every extruder section into the single
   `MaterialProfile`/`ProcessProfile` using the most-restrictive value (max `min_extrude_temp`, min
   `nozzle_diameter`→`line_width`), warning which extruders contributed and on any
   `filament_diameter` mismatch.
3. **Macros → opt-in `--include-macros`** matching common names (`START_PRINT`/`PRINT_START` →
   `start_gcode`, `END_PRINT`/`PRINT_END` → `end_gcode`), with a loud "raw Jinja2 — review" warning.
   Off by default; v1 behaviour (no extraction) is the default.

## Public API change

`import_klipper` gains an options argument (the only signature change; one CLI caller at
`crates/cli/src/main.rs:620`, two re-export lines at `profile/mod.rs:13` and `lib.rs:66`):

```rust
/// Options controlling optional, potentially-lossy Klipper import behaviour.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KlipperImportOptions {
    /// Extract `[gcode_macro START_PRINT|PRINT_START|END_PRINT|PRINT_END]` bodies into
    /// `start_gcode` / `end_gcode`. Off by default — the bodies are raw Jinja2, not valid g-code.
    pub include_macros: bool,
}

pub fn import_klipper(
    text: &str,
    opts: &KlipperImportOptions,
) -> Result<(Profile, Vec<KlipperImportWarning>), KlipperImportError>;
```

`KlipperImportOptions` is re-exported alongside `import_klipper` from `profile/mod.rs` and `lib.rs`.
`KlipperImportOptions::default()` (all-false) reproduces v1 behaviour exactly.

## Follow-up 1 — Delta `build_volume` from `print_radius`

In the `is_delta` branch of `import_klipper` (currently `klipper.rs:205-211`), replace the
unconditional skip with a derivation:

- **Radius `R`** (first source wins, warn which was used):
  1. `[printer] print_radius` — the printable-area radius. **Preferred.**
  2. `[delta_calibrate] radius` — the calibration probe radius (≈ printable radius).
  3. `[printer] delta_radius` — the *geometric* delta radius; **overestimates** the printable area;
     use only as a last resort with a stronger warning.
- **Z high `z_hi`** (first source wins): `[stepper_a] position_max`, else `[stepper_a] position_endstop`.
- **Z low `z_lo`**: `[printer] minimum_z_position` if present (often a small negative), else `0.0`.
- **Emit:**
  - If `R` and `z_hi` both found → `build_volume = Some([[-R, R], [-R, R], [z_lo, z_hi]])` and warn:
    `machine.build_volume` — `"delta build_volume is a bounding-box approximation of a cylindrical
    volume (radius R from <source>) — review"`.
  - If `R` found but no `z_hi` source → omit `build_volume`, warn: `"delta print_radius found but no
    Z height ([stepper_a] position_max/position_endstop) — build_volume omitted"`.
  - If no `R` source → omit, warn: `"machine.build_volume skipped for delta/rotary_delta — no
    print_radius/delta_radius found"` (improved text over today's blanket skip).

Numeric parse failures use the existing `parse_f64_warn` helper. The resulting `build_volume` must
satisfy `Profile::validate()` (`-R < R`, `z_lo ≤ z_hi`); `print_radius` is positive in practice.

## Follow-up 2 — Multi-extruder conservative merge

Replace the first-extruder-only block (`klipper.rs:236-289`) with a merge over all material-extruder
sections.

- **Section selection:** a section name is a material extruder iff it is exactly `"extruder"` or
  `"extruder"` followed by ASCII digits (`extruder1`, `extruder2`, …). **Exclude** `extruder_stepper`
  (a synchronized stepper, no material fields) — note the existing `extruder_count` at
  `klipper.rs:239` uses `starts_with("extruder")` and so miscounts `extruder_stepper`; the new filter
  fixes that.
- **Iterate** the selected sections in sorted order (`extruder` sorts before `extruder1` under the
  existing `BTreeMap` key order — `extruder` < `extruder1` lexically; verify and rely on it).
- **Merge rule (most-restrictive):**
  - `material.filament_diameter` ← the first extruder's value; if a later extruder defines a
    **different** value, warn `"filament_diameter differs across extruders (<a> vs <b>) — used <a>"`
    (keep the first; physical filament size should be uniform).
  - `material.min_nozzle_temperature_c` ← **max** of all extruders' `min_extrude_temp` (highest floor
    is the most conservative).
  - `process.line_width` ← **min** of all extruders' `nozzle_diameter` (tightest line). Keep the
    existing per-field `"process.line_width derived from nozzle_diameter — review"` warning.
  - `pressure_advance`: if **any** extruder defines it, emit the existing single deferred warning.
- **Merge-summary warning** (only when ≥2 material-extruder sections present):
  `extruder` — `"merged N extruders (most-restrictive): min_nozzle_temperature_c=max(...),
  line_width=min(...)"`. With exactly one extruder, emit **no** merge warning (byte-identical to v1).
- A single-extruder config must produce exactly the v1 result (same fields, same warnings, same
  order) — the merge of one element is the identity.

## Follow-up 3 — `--include-macros`

The `scan_ini` scanner collapses every `[gcode_macro NAME]` to one `"gcode_macro"` key and drops the
indented `gcode:` body, so macros need a **separate, dedicated** line walker (do not change
`scan_ini`):

```rust
/// Extract the indented `gcode:` body of the first `[gcode_macro <NAME>]` whose name (case-insensitive)
/// is in `names`. Returns the body as one trimmed line per Vec entry, or None if not found.
fn extract_macro_body(text: &str, names: &[&str]) -> Option<Vec<String>>
```

- Walk lines; on `[gcode_macro <NAME>]`, compare `<NAME>` case-insensitively against `names`.
- Inside a matched macro, find the `gcode:` key line, then capture every following line that begins
  with whitespace (Klipper's required body indentation) until a non-indented line, the next
  `[section]`, or EOF. Trim each captured line (`trim`) and skip blank lines; preserve order.
- Body lines are stored as `GcodeProcedure::Lines(Vec<String>)`.

In `import_klipper`, when `opts.include_macros`:
- `start_gcode` ← `extract_macro_body(text, &["start_print", "print_start"])`
- `end_gcode`   ← `extract_macro_body(text, &["end_print", "print_end"])`
- For each that resolves to `Some`, set the field and warn loudly, e.g. `start_gcode` —
  `"start_gcode imported from [gcode_macro …] — raw Jinja2 / macro calls, not valid g-code as
  imported; review before use"`.
- If `include_macros` is set but neither start nor end macro is found, warn `start_gcode` —
  `"--include-macros set but no START_PRINT/PRINT_START/END_PRINT/PRINT_END macro found"`.
- When `include_macros` is false (default), do nothing (no macro warnings) — preserves v1 output.

### CLI surface

Add to `Cmd::ImportPrinterCfg` (`crates/cli/src/main.rs:110`):

```rust
/// Extract [gcode_macro START_PRINT/END_PRINT] bodies into start_gcode/end_gcode
/// (raw Jinja2 — review before use).
#[arg(long)]
include_macros: bool,
```

Thread it into the handler (`main.rs:616`): build `KlipperImportOptions { include_macros }` and pass
`&opts` to `import_klipper`. All other handler behaviour (name override, stderr warnings,
pretty-JSON + trailing newline, `--out`) is unchanged.

## Data flow (unchanged skeleton, additive)

```
printer.cfg ──▶ import_klipper(text, &KlipperImportOptions{ include_macros })
                  scan_ini (unchanged)            → [printer]/[stepper_*]/[extruder*]/[firmware_retraction]
                  + delta build_volume            (Follow-up 1, in the is_delta branch)
                  + multi-extruder conservative   (Follow-up 2, replaces single-extruder block)
                  + extract_macro_body (opt-in)   (Follow-up 3, separate walker over raw text)
                ──▶ (Profile, Vec<KlipperImportWarning>)
                ──▶ Profile::validate() ──▶ to_string_pretty ──▶ stdout/--out ; warnings ──▶ stderr
```

## Error handling

Unchanged: missing `[printer]` → `KlipperImportError::NotKlipper` (exit 2 via CLI `die`); unreadable
file → `die` exit 2; numeric parse failures → per-field warning + field omitted. None of the
follow-ups introduce new fatal errors (delta with no radius, extruder mismatch, and missing macros
are all warnings).

## Testing & determinism

- **Unit tests in `klipper.rs`** (each asserts `Profile::validate()` passes):
  - Delta with `print_radius` + `stepper_a position_max` → `[[-R,R],[-R,R],[z_lo,z_hi]]` + approximation warning.
  - Delta with `delta_radius` only → box from delta_radius + "overestimates" warning.
  - Delta with no radius → `build_volume` omitted + warning.
  - Two extruders (0.4/170 and 0.6/190) → `line_width=0.4`, `min_nozzle_temperature_c=190` + merge warning.
  - `extruder_stepper` present → not counted as a material extruder (no merge warning).
  - Single extruder → identical to v1 (no merge warning).
  - `--include-macros` on, START_PRINT + END_PRINT present → `start_gcode`/`end_gcode` populated + Jinja2 warnings.
  - `--include-macros` off (default) with macros present → both `None`, no macro warning.
  - `--include-macros` on, no matching macro → warning, both `None`.
- **CLI conformance (drift-gated, mirrors `cli.rs:692`):** add fixtures under `conformance/profiles/`:
  - `klipper_delta.cfg` + `klipper_delta.expected.json` (delta envelope).
  - `klipper_idex.cfg` + `klipper_idex.expected.json` (two extruders, conservative merge).
  - `klipper_macros.cfg` + `klipper_macros.expected.json` produced with `--include-macros`
    (start/end gcode populated). Add an `import-printer-cfg` invocation per fixture in `cli.rs`,
    asserting stdout equals the committed `.expected.json` (regenerate goldens with the built binary).
  - The existing `klipper_voron.cfg` → `klipper_voron.expected.json` assertion stays and **must not
    change** (regression guard for the v1 happy path).
- **Determinism:** all logic is pure over the input text; `BTreeMap` iteration is deterministic;
  no clock/RNG. No new dependency.

## Scope / YAGNI (still deferred)

`[input_shaper]` / `pressure_advance` modelling (warn only — unchanged); `[include]` following;
per-extruder *distinct* materials (dry has one `MaterialProfile` — merge is the chosen model);
delta cylindrical (non-box) volume; macro Jinja2 evaluation / templating; arbitrary macro names
beyond the four START/END aliases; `minimum_cruise_ratio`, `max_z_velocity`/`max_z_accel`.
