//! Multi-head / IDEX machine synchronization (D2.5, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Provides configuration models and G-code emission for multi-head machines:
//! - Independent Dual Extruder (IDEX) 3D printers.
//! - Dual-spindle CNC machining centers.
//! - Hybrid additive-subtractive machines.

use serde::{Deserialize, Serialize};

/// Operating mode for multi-carriage machines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadMode {
    /// Carriages move independently under standard tool selection (Auto-Park).
    #[default]
    Independent,
    /// Second carriage duplicates the primary carriage at a fixed X offset.
    Duplication,
    /// Second carriage mirrors the primary carriage motion across the center.
    Mirrored,
    /// Carriages run sequentially with full parking retracts.
    Sequential,
}

/// Configuration definition for one carriage / toolhead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadConfig {
    /// Head slot index (0 for primary T0, 1 for secondary T1, etc.).
    pub head_index: u32,
    /// Name / label for the head (e.g. "Primary E3D", "Right Spindle").
    pub name: String,
    /// Toolhead coordinate offset `[dx, dy, dz]` relative to origin (mm).
    #[serde(default)]
    pub offset_xyz: [f64; 3],
    /// Safe parking X position (mm).
    pub park_x: f64,
    /// Maximum travel feedrate for this head (mm/min).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_feedrate: Option<f64>,
}

impl HeadConfig {
    pub fn new(head_index: u32, name: impl Into<String>, park_x: f64) -> Self {
        Self {
            head_index,
            name: name.into(),
            offset_xyz: [0.0, 0.0, 0.0],
            park_x,
            max_feedrate: None,
        }
    }
}

/// Emit IDEX carriage mode G-code (Marlin / RepRap / Klipper `M605` standard).
pub fn emit_idex_mode(mode: HeadMode, duplication_x_offset: Option<f64>) -> Vec<String> {
    match mode {
        HeadMode::Independent => vec!["M605 S1 ; Set IDEX Mode: Auto-Park (Independent)".into()],
        HeadMode::Duplication => {
            let offset = duplication_x_offset.unwrap_or(100.0);
            vec![
                format!("M605 S2 X{offset:.3} ; Set IDEX Mode: Duplication"),
                "M605 W ; Activate Duplication".into(),
            ]
        }
        HeadMode::Mirrored => vec!["M605 S3 ; Set IDEX Mode: Mirrored".into()],
        HeadMode::Sequential => vec!["M605 S0 ; Set IDEX Mode: Manual / Sequential".into()],
    }
}

/// Emit toolhead selection G-code (e.g. `T0`, `T1`).
pub fn emit_select_head(head_index: u32) -> String {
    format!("T{head_index} ; Select Toolhead {head_index}")
}
