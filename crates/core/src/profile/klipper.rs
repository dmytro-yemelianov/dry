//! Klipper `printer.cfg` → dry [`Profile`] import.
//!
//! A hand-rolled INI scanner (no new dependency) maps Klipper configuration
//! fields to a dry [`Profile`], returning non-fatal [`KlipperImportWarning`]s
//! for every field that was omitted, approximated, or needs manual review.

use std::collections::BTreeMap;

use super::{
    FirmwareProfile, MachineKinematics, MachineProfile, MaterialProfile, ProcessProfile, Profile,
};

// ── public types ──────────────────────────────────────────────────────────────

/// A non-fatal issue encountered while importing a Klipper `printer.cfg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KlipperImportWarning {
    /// The dry profile field that was affected.
    pub field: String,
    /// Human-readable description of the issue.
    pub message: String,
}

/// A fatal error that prevents importing a Klipper `printer.cfg`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KlipperImportError {
    /// The input does not contain a `[printer]` section (not a Klipper config).
    NotKlipper,
    /// The input could not be parsed.
    Parse(String),
}

impl std::fmt::Display for KlipperImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KlipperImportError::NotKlipper => {
                write!(f, "no [printer] section found — not a Klipper printer.cfg")
            }
            KlipperImportError::Parse(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for KlipperImportError {}

// ── INI scanner ───────────────────────────────────────────────────────────────

type IniMap = BTreeMap<String, BTreeMap<String, String>>;

/// Scan a Klipper-style INI text into a nested map of `section → key → value`.
///
/// - Blank lines and lines starting with `#` or `;` are ignored.
/// - Section headers `[section name]` take only the first whitespace-delimited token
///   (e.g. `[gcode_macro PARK]` → `"gcode_macro"`), lowercased.
/// - Key–value pairs are split on the first `:` or `=` (whichever comes first), both trimmed.
fn scan_ini(text: &str) -> IniMap {
    let mut map: IniMap = BTreeMap::new();
    let mut current_section = String::new();

    for line in text.lines() {
        let line = line.trim();

        // skip blanks and comments
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // section header
        if line.starts_with('[') {
            if let Some(close) = line.find(']') {
                let inner = &line[1..close];
                // first token only so "[gcode_macro PARK]" → "gcode_macro"
                let section = inner
                    .split_whitespace()
                    .next()
                    .unwrap_or(inner)
                    .to_lowercase();
                current_section = section;
                map.entry(current_section.clone()).or_default();
            }
            continue;
        }

        if current_section.is_empty() {
            continue;
        }

        // key: value  OR  key = value  — split on whichever separator comes first
        let sep = match (line.find(':'), line.find('=')) {
            (Some(c), Some(e)) => c.min(e),
            (Some(c), None) => c,
            (None, Some(e)) => e,
            (None, None) => continue,
        };

        let key = line[..sep].trim().to_lowercase();
        let value = line[sep + 1..].trim().to_string();
        if !key.is_empty() {
            map.entry(current_section.clone())
                .or_default()
                .insert(key, value);
        }
    }
    map
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_f64_warn(
    raw: &str,
    field: &str,
    label: &str,
    warnings: &mut Vec<KlipperImportWarning>,
) -> Option<f64> {
    match raw.parse::<f64>() {
        Ok(v) => Some(v),
        Err(_) => {
            warnings.push(KlipperImportWarning {
                field: field.to_string(),
                message: format!("could not parse {label} '{raw}' as a number — skipped"),
            });
            None
        }
    }
}

// ── import function ───────────────────────────────────────────────────────────

/// Import a Klipper `printer.cfg` into a dry [`Profile`].
///
/// Returns the profile (with fields populated as far as the config allows) and a
/// list of non-fatal [`KlipperImportWarning`]s for every field that was omitted,
/// approximated, or needs manual review.
///
/// # Errors
///
/// Returns [`KlipperImportError::NotKlipper`] if there is no `[printer]` section.
pub fn import_klipper(
    text: &str,
) -> Result<(Profile, Vec<KlipperImportWarning>), KlipperImportError> {
    let ini = scan_ini(text);

    // guard: must have [printer]
    if !ini.contains_key("printer") {
        return Err(KlipperImportError::NotKlipper);
    }

    let mut warnings: Vec<KlipperImportWarning> = Vec::new();

    let mut profile = Profile {
        firmware: FirmwareProfile {
            flavor: Some("klipper".to_string()),
        },
        machine: MachineProfile::default(),
        material: MaterialProfile::default(),
        process: ProcessProfile::default(),
        ..Profile::default()
    };

    // ── [printer] ─────────────────────────────────────────────────────────────
    let printer = &ini["printer"];

    // detect kinematics type (delta families skip build_volume)
    let kin_type = printer.get("kinematics").map(|s| s.to_lowercase());
    let is_delta = matches!(kin_type.as_deref(), Some("delta") | Some("rotary_delta"));

    let mut kin = MachineKinematics::default();
    let mut kin_set = false;

    if let Some(raw) = printer.get("max_accel") {
        if let Some(v) = parse_f64_warn(
            raw,
            "machine.kinematics.max_acceleration_mm_s2",
            "max_accel",
            &mut warnings,
        ) {
            kin.max_acceleration_mm_s2 = Some(v);
            kin_set = true;
        }
    }

    if let Some(raw) = printer.get("square_corner_velocity") {
        if let Some(v) = parse_f64_warn(
            raw,
            "machine.kinematics.max_junction_velocity_mm_s",
            "square_corner_velocity",
            &mut warnings,
        ) {
            kin.max_junction_velocity_mm_s = Some(v);
            kin_set = true;
        }
    }

    if kin_set {
        profile.machine.kinematics = Some(kin);
    }

    // feedrate_range: deliberately omitted — no Klipper lower-bound source
    warnings.push(KlipperImportWarning {
        field: "machine.feedrate_range".to_string(),
        message: "machine.feedrate_range omitted (no Klipper lower-bound source) — add manually"
            .to_string(),
    });

    // ── build_volume from stepper position limits ──────────────────────────────
    let get_stepper = |section: &str, key: &str| -> Option<f64> {
        ini.get(section)?.get(key)?.parse::<f64>().ok()
    };

    if is_delta {
        warnings.push(KlipperImportWarning {
            field: "machine.build_volume".to_string(),
            message:
                "machine.build_volume skipped for delta/rotary_delta kinematics — add manually"
                    .to_string(),
        });
    } else {
        let x_min = get_stepper("stepper_x", "position_min").unwrap_or(0.0);
        let x_max = get_stepper("stepper_x", "position_max");
        let y_min = get_stepper("stepper_y", "position_min").unwrap_or(0.0);
        let y_max = get_stepper("stepper_y", "position_max");
        let z_min = get_stepper("stepper_z", "position_min").unwrap_or(0.0);
        let z_max = get_stepper("stepper_z", "position_max").unwrap_or(0.0);

        if let (Some(xmax), Some(ymax)) = (x_max, y_max) {
            profile.machine.build_volume = Some([[x_min, xmax], [y_min, ymax], [z_min, z_max]]);
            warnings.push(KlipperImportWarning {
                field: "machine.build_volume".to_string(),
                message: "machine.build_volume approximated from stepper position limits"
                    .to_string(),
            });
        }
    }

    // ── [extruder] ────────────────────────────────────────────────────────────

    // warn if more than one extruder section is present
    let extruder_count = ini.keys().filter(|k| k.starts_with("extruder")).count();
    if extruder_count > 1 {
        warnings.push(KlipperImportWarning {
            field: "extruder".to_string(),
            message: "only the first extruder imported".to_string(),
        });
    }

    if let Some(extruder) = ini.get("extruder") {
        if let Some(raw) = extruder.get("filament_diameter") {
            if let Some(v) = parse_f64_warn(
                raw,
                "material.filament_diameter",
                "filament_diameter",
                &mut warnings,
            ) {
                profile.material.filament_diameter = Some(v);
            }
        }

        if let Some(raw) = extruder.get("min_extrude_temp") {
            if let Some(v) = parse_f64_warn(
                raw,
                "material.min_nozzle_temperature_c",
                "min_extrude_temp",
                &mut warnings,
            ) {
                profile.material.min_nozzle_temperature_c = Some(v);
            }
        }

        if let Some(raw) = extruder.get("nozzle_diameter") {
            if let Some(v) =
                parse_f64_warn(raw, "process.line_width", "nozzle_diameter", &mut warnings)
            {
                profile.process.line_width = Some(v);
                warnings.push(KlipperImportWarning {
                    field: "process.line_width".to_string(),
                    message: "process.line_width derived from nozzle_diameter — review".to_string(),
                });
            }
        }

        if extruder.contains_key("pressure_advance") {
            warnings.push(KlipperImportWarning {
                field: "extruder.pressure_advance".to_string(),
                message: "[extruder] pressure_advance ignored — deferred to a future machine-model release"
                    .to_string(),
            });
        }
    }

    // ── [firmware_retraction] ─────────────────────────────────────────────────
    if let Some(fw_ret) = ini.get("firmware_retraction") {
        if let Some(raw) = fw_ret.get("retract_length") {
            if let Some(v) = parse_f64_warn(
                raw,
                "process.max_retraction_distance",
                "retract_length",
                &mut warnings,
            ) {
                profile.process.max_retraction_distance = Some(v);
                warnings.push(KlipperImportWarning {
                    field: "process.max_retraction_distance".to_string(),
                    message:
                        "process.max_retraction_distance set from firmware_retraction.retract_length — review"
                            .to_string(),
                });
            }
        }

        if let Some(raw) = fw_ret.get("retract_speed") {
            if let Some(v) = parse_f64_warn(
                raw,
                "process.max_retraction_speed",
                "retract_speed",
                &mut warnings,
            ) {
                // Klipper retract_speed is in mm/s; dry uses mm/min
                profile.process.max_retraction_speed = Some(v * 60.0);
                warnings.push(KlipperImportWarning {
                    field: "process.max_retraction_speed".to_string(),
                    message:
                        "process.max_retraction_speed set from firmware_retraction.retract_speed (×60 mm/s→mm/min) — review"
                            .to_string(),
                });
            }
        }
    }

    // ── [input_shaper] ────────────────────────────────────────────────────────
    if ini.contains_key("input_shaper") {
        warnings.push(KlipperImportWarning {
            field: "input_shaper".to_string(),
            message: "[input_shaper] ignored — deferred to a future machine-model release"
                .to_string(),
        });
    }

    // ── [include ...] ─────────────────────────────────────────────────────────
    if ini.contains_key("include") {
        warnings.push(KlipperImportWarning {
            field: "include".to_string(),
            message: "[include] not followed".to_string(),
        });
    }

    // ── absent: max_volumetric_flow_mm3_s ─────────────────────────────────────
    // Not present in printer.cfg — always emit a prominent warning.
    warnings.push(KlipperImportWarning {
        field: "material.max_volumetric_flow_mm3_s".to_string(),
        message:
            "material.max_volumetric_flow_mm3_s not in printer.cfg — add from your hotend calibration (most useful review-gcode safety contract)"
                .to_string(),
    });

    Ok((profile, warnings))
}

// ── tests ─────────────────────────────────────────────────────────────────────

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
        let k = p.machine.kinematics.clone().unwrap();
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
        assert!(matches!(
            import_klipper("hello world\n"),
            Err(KlipperImportError::NotKlipper)
        ));
    }

    #[test]
    fn feedrate_range_is_omitted_with_a_warning() {
        let (p, w) = import_klipper(CFG).unwrap();
        assert!(p.machine.feedrate_range.is_none());
        assert!(w.iter().any(|x| x.field == "machine.feedrate_range"));
    }
}
