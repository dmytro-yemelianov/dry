//! Versioned machine/material profile data.
//!
//! Profiles are intentionally small at this stage: they carry the factual limits that can be enforced
//! by the existing verifier and the import defaults needed to recover geometry from slicer G-code.

use crate::emit::EmitParams;
use crate::gcode::GcodeImportParams;
use crate::resolve::ResolveParams;
use crate::verify::Contracts;
use serde::{Deserialize, Serialize};

fn default_profile_version() -> u32 {
    1
}

/// Versioned profile JSON for a printer/material/process combination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Profile schema version. Version 1 is the initial Dry profile schema.
    #[serde(default = "default_profile_version")]
    pub version: u32,
    /// Human-readable profile name, for reports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Firmware-flavor metadata. Not all fields are enforced yet, but keeping it explicit prevents
    /// Klipper/Marlin/Duet assumptions from leaking into generic validation.
    #[serde(default)]
    pub firmware: FirmwareProfile,
    /// Machine envelope and motion limits.
    #[serde(default)]
    pub machine: MachineProfile,
    /// Material and hotend limits.
    #[serde(default)]
    pub material: MaterialProfile,
    /// Process defaults used when lifting slicer G-code to Dry IR.
    #[serde(default)]
    pub process: ProcessProfile,
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            version: default_profile_version(),
            name: None,
            firmware: FirmwareProfile::default(),
            machine: MachineProfile::default(),
            material: MaterialProfile::default(),
            process: ProcessProfile::default(),
        }
    }
}

/// Firmware metadata for dialect-aware review.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FirmwareProfile {
    /// Firmware/dialect flavor such as `klipper`, `marlin`, or `duet`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
}

/// Machine envelope and motion limits.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineProfile {
    /// Build volume as `[[x_lo, x_hi], [y_lo, y_hi], [z_lo, z_hi]]` in millimetres.
    #[serde(default, alias = "bounds", skip_serializing_if = "Option::is_none")]
    pub build_volume: Option<[[f64; 2]; 3]>,
    /// Allowed feedrate range `[min, max]` in mm/min for extruding moves.
    #[serde(
        default,
        alias = "speed_range",
        skip_serializing_if = "Option::is_none"
    )]
    pub feedrate_range: Option<[f64; 2]>,
}

/// Material and hotend limits.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MaterialProfile {
    /// Filament diameter in millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filament_diameter: Option<f64>,
    /// Maximum volumetric flow in mm³/s.
    #[serde(
        default,
        alias = "max_flow",
        alias = "max_volumetric_flow",
        skip_serializing_if = "Option::is_none"
    )]
    pub max_volumetric_flow_mm3_s: Option<f64>,
    /// Minimum nozzle temperature in °C required for extrusion.
    #[serde(
        default,
        alias = "min_temp",
        alias = "min_nozzle_temp",
        skip_serializing_if = "Option::is_none"
    )]
    pub min_nozzle_temperature_c: Option<f64>,
}

/// Process defaults used for G-code import/review.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessProfile {
    /// Assumed extrusion line width in millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_width: Option<f64>,
    /// Assumed layer height in millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_height: Option<f64>,
    /// Require Z never to decrease.
    #[serde(default)]
    pub monotonic_z: bool,
}

/// A profile parse or validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileError {
    message: String,
}

impl ProfileError {
    fn new(message: impl Into<String>) -> Self {
        ProfileError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProfileError {}

fn validate_finite(name: &str, value: f64) -> Result<(), ProfileError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProfileError::new(format!("{name} must be finite")))
    }
}

fn validate_positive(name: &str, value: Option<f64>) -> Result<(), ProfileError> {
    if let Some(value) = value {
        validate_finite(name, value)?;
        if value <= 0.0 {
            return Err(ProfileError::new(format!("{name} must be positive")));
        }
    }
    Ok(())
}

fn validate_range(name: &str, range: Option<[f64; 2]>) -> Result<(), ProfileError> {
    if let Some([lo, hi]) = range {
        validate_finite(&format!("{name} lower bound"), lo)?;
        validate_finite(&format!("{name} upper bound"), hi)?;
        if lo > hi {
            return Err(ProfileError::new(format!(
                "{name} lower bound must be <= upper bound"
            )));
        }
    }
    Ok(())
}

fn validate_positive_range(name: &str, range: Option<[f64; 2]>) -> Result<(), ProfileError> {
    validate_range(name, range)?;
    if let Some([lo, hi]) = range {
        if lo < 0.0 || hi <= 0.0 {
            return Err(ProfileError::new(format!(
                "{name} must have a non-negative lower bound and positive upper bound"
            )));
        }
    }
    Ok(())
}

impl Profile {
    /// Parse and validate a profile JSON document.
    pub fn from_json(text: &str) -> Result<Self, ProfileError> {
        let profile: Profile = serde_json::from_str(text)
            .map_err(|e| ProfileError::new(format!("invalid profile JSON: {e}")))?;
        profile.validate()?;
        Ok(profile)
    }

    /// Validate schema version and numeric limits.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.version != 1 {
            return Err(ProfileError::new(format!(
                "unsupported profile version {} (expected 1)",
                self.version
            )));
        }
        if let Some(bounds) = self.machine.build_volume {
            for (axis, [lo, hi]) in ["X", "Y", "Z"].into_iter().zip(bounds) {
                validate_finite(&format!("{axis} build-volume lower bound"), lo)?;
                validate_finite(&format!("{axis} build-volume upper bound"), hi)?;
                if lo > hi {
                    return Err(ProfileError::new(format!(
                        "{axis} build-volume lower bound must be <= upper bound"
                    )));
                }
            }
        }
        validate_positive_range("feedrate_range", self.machine.feedrate_range)?;
        validate_positive(
            "material.filament_diameter",
            self.material.filament_diameter,
        )?;
        validate_positive(
            "material.max_volumetric_flow_mm3_s",
            self.material.max_volumetric_flow_mm3_s,
        )?;
        validate_positive(
            "material.min_nozzle_temperature_c",
            self.material.min_nozzle_temperature_c,
        )?;
        validate_positive("process.line_width", self.process.line_width)?;
        validate_positive("process.layer_height", self.process.layer_height)?;
        Ok(())
    }

    /// Convert profile limits to verifier contracts.
    pub fn contracts(&self) -> Contracts {
        Contracts {
            bounds: self.machine.build_volume,
            max_flow: self.material.max_volumetric_flow_mm3_s,
            speed_range: self.machine.feedrate_range,
            monotonic_z: self.process.monotonic_z,
            min_temp: self.material.min_nozzle_temperature_c,
        }
    }

    /// Convert profile material defaults to L1 resolve parameters.
    ///
    /// The current profile schema does not yet carry authored-design print/travel speeds, so those remain
    /// the engine defaults. Filament diameter is centralized here so adapters do not grow separate fallback
    /// rules.
    pub fn resolve_params(&self) -> ResolveParams {
        ResolveParams {
            dia: self.material.filament_diameter.unwrap_or(1.75),
            ..ResolveParams::default()
        }
    }

    /// Convert profile material/process defaults to G-code import parameters.
    pub fn gcode_import_params(&self) -> GcodeImportParams {
        GcodeImportParams {
            version: 0,
            filament_diameter: self.material.filament_diameter.unwrap_or(1.75),
            line_width: self.process.line_width,
            layer_height: self.process.layer_height,
        }
    }

    /// Convert firmware/profile settings to emitter parameters.
    ///
    /// No firmware-specific emitter fields exist yet, so this deliberately centralizes the current default.
    pub fn emit_params(&self) -> EmitParams {
        EmitParams::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_maps_to_contracts() {
        let profile = Profile::from_json(
            r#"{
              "version": 1,
              "name": "voron24-abs",
              "firmware": {"flavor": "klipper"},
              "machine": {
                "build_volume": [[0, 350], [0, 350], [0, 250]],
                "feedrate_range": [300, 18000]
              },
              "material": {
                "filament_diameter": 1.75,
                "max_volumetric_flow_mm3_s": 24,
                "min_nozzle_temperature_c": 230
              },
              "process": {
                "line_width": 0.45,
                "layer_height": 0.2,
                "monotonic_z": true
              }
            }"#,
        )
        .unwrap();

        let contracts = profile.contracts();
        assert_eq!(contracts.bounds.unwrap()[0], [0.0, 350.0]);
        assert_eq!(contracts.max_flow, Some(24.0));
        assert_eq!(contracts.speed_range, Some([300.0, 18000.0]));
        assert!(contracts.monotonic_z);
        assert_eq!(contracts.min_temp, Some(230.0));

        let import = profile.gcode_import_params();
        assert_eq!(import.filament_diameter, 1.75);
        assert_eq!(import.line_width, Some(0.45));
        assert_eq!(import.layer_height, Some(0.2));

        let resolve = profile.resolve_params();
        assert_eq!(resolve.dia, 1.75);
    }

    #[test]
    fn rejects_bad_ranges() {
        let err = Profile::from_json(
            r#"{
              "version": 1,
              "machine": {"build_volume": [[0, 10], [0, 10], [20, 0]]}
            }"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("Z build-volume"));
    }
}
