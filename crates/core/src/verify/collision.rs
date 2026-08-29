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

        let (ux, uy, uz) = if let Some(orient) = seg.orientation {
            let mag = libm::sqrt(orient[0] * orient[0] + orient[1] * orient[1] + orient[2] * orient[2]);
            if mag > 1e-6 {
                (orient[0] / mag, orient[1] / mag, orient[2] / mag)
            } else {
                (0.0, 0.0, 1.0)
            }
        } else {
            (0.0, 0.0, 1.0)
        };

        let tilt_radial_drop = holder_radius * libm::sqrt(ux * ux + uy * uy);

        // Check sample points along the holder axis from stickout to collet end
        let num_samples = 6;
        let sample_step = holder.collet_length.max(15.0) / (num_samples as f64);

        for s in 0..=num_samples {
            let dist = holder.stickout_length + (s as f64 * sample_step);
            let hx = x + ux * dist;
            let hy = y + uy * dist;
            let hz = z + uz * dist;
            let lowest_holder_z = hz - tilt_radial_drop;

            let in_stock_xy = hx >= stock_bounds[0] - holder_radius
                && hx <= stock_bounds[1] + holder_radius
                && hy >= stock_bounds[2] - holder_radius
                && hy <= stock_bounds[3] + holder_radius;

            if in_stock_xy && lowest_holder_z < stock_top_z && hz >= stock_bounds[4] {
                let is_5axis = ux.abs() > 1e-4 || uy.abs() > 1e-4;
                let code = if is_5axis {
                    "TOOL_HOLDER_5AXIS_COLLISION"
                } else {
                    "TOOL_HOLDER_COLLISION"
                };

                let depth = stock_top_z - z;
                findings.push(CollisionFinding {
                    severity: Severity::Error,
                    code: code.into(),
                    message: format!(
                        "Tool holder (⌀{:.2}mm) collides with stock top (lowest point Z{lowest_holder_z:.2}mm < Z{stock_top_z:.2}mm); centerline at (X{hx:.2}, Y{hy:.2}, Z{hz:.2})",
                        holder_radius * 2.0,
                    ),
                    segment_index: idx,
                    plunge_depth: depth,
                });
                break;
            }
        }
    }

    findings
}
