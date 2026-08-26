//! Tool holder & spindle collision detection (D2.1 / D4.1, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! One rule: the holder assembly must not descend below the top of the stock. The tool tip stands
//! `stickout_length` proud of the holder face, so a cut deeper than the stickout puts the holder
//! itself inside the stock.
//!
//! What this does not do, despite the name: it models no fixtures, no gantry, no previously
//! machined pockets, and no stock walls other than the top plane. A holder that clears the stock
//! top but fouls a clamp is not detected here.

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
    ///
    /// Declared for callers describing a real assembly, but no rule here reads it: it bounds the
    /// collet upwards, and the only obstruction this checker models is the stock top plane, which
    /// the holder face reaches first. Detecting what the collet's upper end fouls needs fixture
    /// geometry the checker is not given.
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

    // The body that can foul the stock is the widest of the assembly, not the cutter. Testing the
    // tip against the raw footprint misses the case this rule exists for: a cut just outside the
    // stock edge whose holder still overhangs it. Expand the footprint by the holder radius so the
    // test asks whether the assembly overlaps the stock, not whether the tip is over it.
    let holder_radius = f64::max(holder.holder_diameter, holder.collet_diameter).max(0.0) / 2.0;

    for (idx, seg) in toolpath.segments.iter().enumerate() {
        let (Some(ex), Some(ey), Some(ez)) = (seg.end[0], seg.end[1], seg.end[2]) else {
            continue;
        };

        let x = ex.value();
        let y = ey.value();
        let z = ez.value();

        let in_stock_xy = x >= stock_bounds[0] - holder_radius
            && x <= stock_bounds[1] + holder_radius
            && y >= stock_bounds[2] - holder_radius
            && y <= stock_bounds[3] + holder_radius;

        if in_stock_xy && z < stock_top_z {
            let depth = stock_top_z - z;

            // If cut depth exceeds tool stickout length, the collet/holder collides with the stock top surface
            if depth > holder.stickout_length {
                findings.push(CollisionFinding {
                    severity: Severity::Error,
                    code: "TOOL_HOLDER_COLLISION".into(),
                    message: format!(
                        "Plunge depth {depth:.2}mm exceeds tool stickout length {:.2}mm; holder (⌀{:.2}mm) collision at (X{x:.2}, Y{y:.2}, Z{z:.2})",
                        holder.stickout_length,
                        holder_radius * 2.0,
                    ),
                    segment_index: idx,
                    plunge_depth: depth,
                });
            }
        }
    }

    findings
}
