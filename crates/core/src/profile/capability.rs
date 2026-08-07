//! Machine capability validator and failure-closed lowering gate.
//!
//! Validates an $L_2$ [`Toolpath`] against a [`MachineCapability`] schema before emission,
//! enforcing failure-closed rejection when a toolpath requires unsupported machine capabilities.

use crate::ir::Toolpath;
use serde::{Deserialize, Serialize};

/// Normative Machine Capability declaration (matches `spec/machine-capability.schema.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineCapability {
    /// Schema version (must be 1).
    #[serde(default = "default_version")]
    pub version: u32,
    /// Primary process family (e.g., "additive-fff", "subtractive-cnc", "directed-energy-laser", "robotic-arm").
    pub process_family: String,
    /// Axis degrees-of-freedom (3, 4, 5, or 6).
    pub axes: u8,
    /// Optional rotary axes configuration ("ab", "ac", "bc", or "none").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotary_axes: Option<String>,
    /// Bounding box `[[x_min, x_max], [y_min, y_max], [z_min, z_max]]` in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_volume: Option<[[f64; 2]; 3]>,
    /// Max volumetric flow rate in mm^3/s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_volumetric_flow_mm3_s: Option<f64>,
    /// Max linear feedrate in mm/min.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_feedrate_mm_min: Option<f64>,
    /// Max spindle/laser RPM or count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spindle_rpm: Option<f64>,
    /// Whether spindle/laser power words (`S`) are supported.
    #[serde(default)]
    pub supports_power_channel: bool,
    /// Whether 5-axis orientation vectors `[i, j, k]` are supported.
    #[serde(default)]
    pub supports_toolframe_orientation: bool,
}

fn default_version() -> u32 {
    1
}

impl Default for MachineCapability {
    fn default() -> Self {
        MachineCapability {
            version: 1,
            process_family: "additive-fff".into(),
            axes: 3,
            rotary_axes: None,
            build_volume: Some([[0.0, 250.0], [0.0, 250.0], [0.0, 250.0]]),
            max_volumetric_flow_mm3_s: Some(30.0),
            max_feedrate_mm_min: Some(18000.0),
            max_spindle_rpm: None,
            supports_power_channel: false,
            supports_toolframe_orientation: false,
        }
    }
}

/// Structured violation record when an IR toolpath exceeds machine capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityViolation {
    /// Diagnostic rule code (e.g. "UNSUPPORTED_5_AXIS_ORIENTATION", "UNSUPPORTED_POWER_CHANNEL", "OUT_OF_BOUNDS_MOVE").
    pub rule_id: String,
    /// 0-indexed segment location in the IR toolpath.
    pub segment_index: usize,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl MachineCapability {
    /// Validate a toolpath against machine capabilities, returning `Ok(())` or a list of violations.
    pub fn validate(&self, toolpath: &Toolpath) -> Result<(), Vec<CapabilityViolation>> {
        let mut violations = Vec::new();

        for (idx, seg) in toolpath.segments.iter().enumerate() {
            // 1. Check 5-axis orientation vector support
            if let Some(orient) = seg.orientation {
                let is_identity = orient[0].abs() < 1e-6
                    && orient[1].abs() < 1e-6
                    && (orient[2] - 1.0).abs() < 1e-6;
                if !is_identity && (!self.supports_toolframe_orientation || self.axes < 5) {
                    violations.push(CapabilityViolation {
                        rule_id: "UNSUPPORTED_5_AXIS_ORIENTATION".into(),
                        segment_index: idx,
                        message: format!(
                            "Segment {} commands 5-axis orientation {:?}, but machine has {} axes (supports_orientation = {})",
                            idx, orient, self.axes, self.supports_toolframe_orientation
                        ),
                    });
                }
            }

            // 2. Check power channel (spindle / laser S-word) support
            if let Some(pwr) = seg.power {
                if pwr > 0.0 && !self.supports_power_channel {
                    violations.push(CapabilityViolation {
                        rule_id: "UNSUPPORTED_POWER_CHANNEL".into(),
                        segment_index: idx,
                        message: format!(
                            "Segment {idx} commands power channel value {pwr}, but target machine does not support power channels"
                        ),
                    });
                }
            }

            // 3. Check build volume bounds if defined
            if let Some(bounds) = &self.build_volume {
                for (axis_idx, axis_name) in [("X", 0), ("Y", 1), ("Z", 2)] {
                    if let Some(coord) = seg.end[axis_name].map(|l| l.value()) {
                        let lo = bounds[axis_name][0];
                        let hi = bounds[axis_name][1];
                        if coord < lo || coord > hi {
                            violations.push(CapabilityViolation {
                                rule_id: "OUT_OF_BOUNDS_MOVE".into(),
                                segment_index: idx,
                                message: format!(
                                    "Segment {idx} endpoint {axis_idx} coordinate {coord:.3} mm lies outside build volume [{lo:.1}, {hi:.1}] mm"
                                ),
                            });
                        }
                    }
                }
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Segment, SegmentKind, Toolpath};
    use crate::units::{Feedrate, Length, Volume};

    #[test]
    fn test_valid_toolpath_passes_capability_check() {
        let cap = MachineCapability::default();
        let toolpath = Toolpath {
            version: 0,
            segments: vec![Segment {
                start: [Some(Length(0.0)), Some(Length(0.0)), Some(Length(0.2))],
                end: [Some(Length(10.0)), Some(Length(10.0)), Some(Length(0.2))],
                travel: false,
                speed: Feedrate(1200.0),
                length: Length(14.142),
                volume: Volume(0.5),
                filament: Length(0.2),
                width: None,
                height: None,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: None,
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            }],
            meta: None,
        };

        assert!(cap.validate(&toolpath).is_ok());
    }

    #[test]
    fn test_unsupported_power_channel_fails_validation() {
        let cap = MachineCapability {
            supports_power_channel: false,
            ..Default::default()
        };

        let toolpath = Toolpath {
            version: 0,
            segments: vec![Segment {
                start: [Some(Length(0.0)), Some(Length(0.0)), Some(Length(0.2))],
                end: [Some(Length(10.0)), Some(Length(10.0)), Some(Length(0.2))],
                travel: false,
                speed: Feedrate(1200.0),
                length: Length(10.0),
                volume: Volume(0.0),
                filament: Length(0.0),
                width: None,
                height: None,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: None,
                power: Some(800.0), // Laser S-word requested
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            }],
            meta: None,
        };

        let res = cap.validate(&toolpath);
        assert!(res.is_err());
        let violations = res.unwrap_err();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "UNSUPPORTED_POWER_CHANNEL");
    }

    #[test]
    fn test_unsupported_5_axis_orientation_fails_validation() {
        let cap = MachineCapability::default(); // 3-axis default

        let toolpath = Toolpath {
            version: 0,
            segments: vec![Segment {
                start: [Some(Length(0.0)), Some(Length(0.0)), Some(Length(0.2))],
                end: [Some(Length(10.0)), Some(Length(10.0)), Some(Length(0.2))],
                travel: false,
                speed: Feedrate(1200.0),
                length: Length(10.0),
                volume: Volume(0.0),
                filament: Length(0.0),
                width: None,
                height: None,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: None,
                power: None,
                dwell_s: None,
                manual_gcode: None,
                // Non-planar vector requested (45° in XZ, exactly unit-length).
                orientation: Some([
                    std::f64::consts::FRAC_1_SQRT_2,
                    0.0,
                    std::f64::consts::FRAC_1_SQRT_2,
                ]),
                control_points: None,
            }],
            meta: None,
        };

        let res = cap.validate(&toolpath);
        assert!(res.is_err());
        let violations = res.unwrap_err();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "UNSUPPORTED_5_AXIS_ORIENTATION");
    }
}
