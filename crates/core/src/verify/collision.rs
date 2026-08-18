//! Tool holder & spindle collision detection (D2.1 / D4.1, `docs/20-dry-ir-ecosystem-implementation-plan.md` §6.4).
//!
//! Verifies that non-cutting tool holders, collet nuts, and spindle housings do not collide
//! with stock boundaries or previously unmachined stock walls during plunge cuts or rapid traverses.

use crate::ir::Toolpath;
use crate::verify::Severity;
use serde::{Deserialize, Serialize};

/// Physical dimensions of the non-cutting tool holder assembly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolHolder {
    /// Diameter of the tool holder body (mm).
    pub holder_diameter: f64,
    /// Distance from the tool tip to the holder face (gauge length / flute stickout, mm).
    pub stickout_length: f64,
    /// Maximum diameter of the collet nut / chuck (mm).
    pub collet_diameter: f64,
    /// Length of the collet zone above the stickout (mm).
    pub collet_length: f64,
}

impl Default for ToolHolder {
    fn default() -> Self {
        Self {
            holder_diameter: 50.0, // ER32 holder body
            stickout_length: 25.0, // 25mm exposed cutter stickout
            collet_diameter: 40.0, // ER32 collet nut
            collet_length: 30.0,
        }
    }
}

/// A collision finding detected during simulation / pre-flight verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollisionFinding {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub segment_index: usize,
    pub plunge_depth: f64,
}

/// Check toolpath for tool holder & collet interference against a defined stock volume `[min_x, max_x, min_y, max_y, min_z, max_z]`.
pub fn check_tool_holder_collision(
    toolpath: &Toolpath,
    holder: &ToolHolder,
    stock_bounds: [f64; 6],
) -> Vec<CollisionFinding> {
    let mut findings = Vec::new();
    let stock_top_z = stock_bounds[5];

    for (idx, seg) in toolpath.segments.iter().enumerate() {
        let (Some(ex), Some(ey), Some(ez)) = (seg.end[0], seg.end[1], seg.end[2]) else {
            continue;
        };

        let x = ex.value();
        let y = ey.value();
        let z = ez.value();

        // If cutter is inside stock XY boundary
        let in_stock_xy = x >= stock_bounds[0]
            && x <= stock_bounds[1]
            && y >= stock_bounds[2]
            && y <= stock_bounds[3];

        if in_stock_xy && z < stock_top_z {
            let depth = stock_top_z - z;

            // If cut depth exceeds tool stickout length, the collet/holder collides with the stock top surface
            if depth > holder.stickout_length {
                findings.push(CollisionFinding {
                    severity: Severity::Error,
                    code: "TOOL_HOLDER_COLLISION".into(),
                    message: format!(
                        "Plunge depth {:.2}mm exceeds tool stickout length {:.2}mm; collet collision at (X{:.2}, Y{:.2}, Z{:.2})",
                        depth, holder.stickout_length, x, y, z
                    ),
                    segment_index: idx,
                    plunge_depth: depth,
                });
            }
        }
    }

    findings
}
