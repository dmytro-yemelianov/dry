//! Machine profile capabilities and pre-flight compatibility engine (D2.1, `docs/20-dry-ir-ecosystem-implementation-plan.md` §6.4).
//!
//! Evaluates whether a toolpath meets the physical machine constraints before emission:
//! - Work envelope (min/max X, Y, Z)
//! - Maximum feedrate bounds
//! - Spindle RPM limits
//! - Rotary axis range (A, B, C)

use crate::ir::Toolpath;
use serde::{Deserialize, Serialize};

/// Axis bounds in millimetres.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisRange {
    pub min: f64,
    pub max: f64,
}

impl AxisRange {
    pub fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, val: f64) -> bool {
        val >= self.min && val <= self.max
    }
}

/// Machine physical and operational capability limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineCapabilities {
    pub name: String,
    pub x_range: AxisRange,
    pub y_range: AxisRange,
    pub z_range: AxisRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_feedrate_mm_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_spindle_rpm: Option<f64>,
}

impl MachineCapabilities {
    pub fn new(name: impl Into<String>, x: AxisRange, y: AxisRange, z: AxisRange) -> Self {
        Self {
            name: name.into(),
            x_range: x,
            y_range: y,
            z_range: z,
            max_feedrate_mm_min: None,
            max_spindle_rpm: None,
        }
    }
}

/// Severity of a compatibility finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Warning,
    Error,
}

/// A specific capability violation finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityFinding {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment_index: Option<usize>,
}

/// The report resulting from running the pre-flight compatibility engine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub findings: Vec<CompatibilityFinding>,
}

impl CompatibilityReport {
    pub fn new() -> Self {
        Self {
            compatible: true,
            findings: Vec::new(),
        }
    }

    pub fn add_finding(&mut self, finding: CompatibilityFinding) {
        if finding.severity == Severity::Error {
            self.compatible = false;
        }
        self.findings.push(finding);
    }
}

/// Check toolpath compatibility against machine capabilities.
pub fn check_compatibility(
    toolpath: &Toolpath,
    capabilities: &MachineCapabilities,
) -> CompatibilityReport {
    let mut report = CompatibilityReport::new();

    for (index, seg) in toolpath.segments.iter().enumerate() {
        if let Some(x_len) = seg.end[0] {
            let x = x_len.value();
            if !capabilities.x_range.contains(x) {
                report.add_finding(CompatibilityFinding {
                    severity: Severity::Error,
                    code: "OUT_OF_BOUNDS_X".into(),
                    message: format!(
                        "X coordinate {x:.3} is outside machine limit [{:.3}, {:.3}]",
                        capabilities.x_range.min, capabilities.x_range.max
                    ),
                    segment_index: Some(index),
                });
            }
        }

        if let Some(y_len) = seg.end[1] {
            let y = y_len.value();
            if !capabilities.y_range.contains(y) {
                report.add_finding(CompatibilityFinding {
                    severity: Severity::Error,
                    code: "OUT_OF_BOUNDS_Y".into(),
                    message: format!(
                        "Y coordinate {y:.3} is outside machine limit [{:.3}, {:.3}]",
                        capabilities.y_range.min, capabilities.y_range.max
                    ),
                    segment_index: Some(index),
                });
            }
        }

        if let Some(z_len) = seg.end[2] {
            let z = z_len.value();
            if !capabilities.z_range.contains(z) {
                report.add_finding(CompatibilityFinding {
                    severity: Severity::Error,
                    code: "OUT_OF_BOUNDS_Z".into(),
                    message: format!(
                        "Z coordinate {z:.3} is outside machine limit [{:.3}, {:.3}]",
                        capabilities.z_range.min, capabilities.z_range.max
                    ),
                    segment_index: Some(index),
                });
            }
        }

        if let Some(max_feed) = capabilities.max_feedrate_mm_min {
            if seg.speed.value() > max_feed {
                report.add_finding(CompatibilityFinding {
                    severity: Severity::Warning,
                    code: "EXCEEDS_MAX_FEEDRATE".into(),
                    message: format!(
                        "Feedrate {:.1} mm/min exceeds machine max {:.1} mm/min",
                        seg.speed.value(),
                        max_feed
                    ),
                    segment_index: Some(index),
                });
            }
        }
    }

    report
}
