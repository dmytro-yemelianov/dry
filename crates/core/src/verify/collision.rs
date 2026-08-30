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

/// A defined axial segment along a stepped/tapered tool holder assembly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolHolderSection {
    /// Diameter at this segment (mm).
    pub diameter: f64,
    /// Axial length of this segment (mm).
    pub length: f64,
}

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
    /// Optional discrete stepped/tapered profile sections from holder face upward.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<ToolHolderSection>>,
}

impl Default for ToolHolder {
    fn default() -> Self {
        Self {
            holder_diameter: 50.0, // ER32 holder body
            stickout_length: 25.0, // 25mm exposed cutter stickout
            collet_diameter: 40.0, // ER32 collet nut
            collet_length: 30.0,
            sections: None,
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

/// One sampled point on the holder axis, and the stock it is being tested against.
///
/// Both holder paths — the stepped-section profile and the single-body fallback — ask exactly this
/// question and differ only in which radius they ask it with, so the geometry lives here once. That
/// is also a hard requirement rather than a tidiness preference: the numeric-boundary inventory pins
/// this decision by *source anchor* (`FM1.F64.VERIFY.COLLISION.HOLDER_DEPTH`, `proofs/verify-numeric-
/// boundaries-v0.toml`), and an anchor must occur exactly once in the file.
struct HolderSample {
    /// Sampled point on the holder centreline (mm).
    centre: [f64; 3],
    /// Radius of the holder body at this sample (mm).
    radius: f64,
    /// Z of the tool tip for the segment under test (mm) — the plunge datum, not the sample.
    tip_z: f64,
    /// Unit tool-direction vector, used only to classify the finding as 3- or 5-axis.
    axis: [f64; 3],
}

impl HolderSample {
    /// The lowest point of the holder body at this sample: the centreline dropped by the radial
    /// component the tilt swings below it. Vertical (`axis = +Z`) leaves it at the centreline.
    fn lowest_z(&self) -> f64 {
        let tilt = libm::sqrt(self.axis[0] * self.axis[0] + self.axis[1] * self.axis[1]);
        self.centre[2] - self.radius * tilt
    }

    /// Does the body at this sample overlap the stock top plane?
    ///
    /// The footprint is expanded by this sample's radius so the test asks whether the *assembly*
    /// overlaps the stock, not whether the centreline is over it.
    fn collides(&self, stock_bounds: [f64; 6]) -> bool {
        let [hx, hy, hz] = self.centre;
        let in_stock_xy = hx >= stock_bounds[0] - self.radius
            && hx <= stock_bounds[1] + self.radius
            && hy >= stock_bounds[2] - self.radius
            && hy <= stock_bounds[3] + self.radius;
        in_stock_xy && self.lowest_z() < stock_bounds[5] && hz >= stock_bounds[4]
    }

    /// Build the finding for a sample [`Self::collides`] has already accepted. `label` is the only
    /// thing the two holder paths disagree about.
    fn finding(
        &self,
        stock_bounds: [f64; 6],
        label: &str,
        segment_index: usize,
    ) -> CollisionFinding {
        let stock_top_z = stock_bounds[5];
        let lowest_holder_z = self.lowest_z();
        let [hx, hy, hz] = self.centre;
        let is_5axis = self.axis[0].abs() > 1e-4 || self.axis[1].abs() > 1e-4;
        let code = if is_5axis {
            "TOOL_HOLDER_5AXIS_COLLISION"
        } else {
            "TOOL_HOLDER_COLLISION"
        };
        let depth = stock_top_z - self.tip_z;
        CollisionFinding {
            severity: Severity::Error,
            code: code.into(),
            message: format!(
                "{label} (⌀{:.2}mm) collides with stock top (lowest point Z{lowest_holder_z:.2}mm < Z{stock_top_z:.2}mm); centerline at (X{hx:.2}, Y{hy:.2}, Z{hz:.2})",
                self.radius * 2.0,
            ),
            segment_index,
            plunge_depth: depth,
        }
    }
}

/// Check toolpath for tool holder & collet interference against a defined stock volume `[min_x, max_x, min_y, max_y, min_z, max_z]`.
pub fn check_tool_holder_collision(
    toolpath: &Toolpath,
    holder: &ToolHolder,
    stock_bounds: [f64; 6],
) -> Vec<CollisionFinding> {
    let mut findings = Vec::new();

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
            let mag =
                libm::sqrt(orient[0] * orient[0] + orient[1] * orient[1] + orient[2] * orient[2]);
            if mag > 1e-6 {
                (orient[0] / mag, orient[1] / mag, orient[2] / mag)
            } else {
                (0.0, 0.0, 1.0)
            }
        } else {
            (0.0, 0.0, 1.0)
        };

        if let Some(ref sections) = holder.sections {
            let mut cumulative_dist = holder.stickout_length;
            let mut collided = false;
            for sec in sections {
                let sec_radius = sec.diameter.max(0.0) / 2.0;
                let num_sec_samples = 3;
                let step = sec.length.max(1.0) / (num_sec_samples as f64);
                for s in 0..=num_sec_samples {
                    let dist = cumulative_dist + (s as f64 * step);
                    let sample = HolderSample {
                        centre: [x + ux * dist, y + uy * dist, z + uz * dist],
                        radius: sec_radius,
                        tip_z: z,
                        axis: [ux, uy, uz],
                    };
                    if sample.collides(stock_bounds) {
                        findings.push(sample.finding(stock_bounds, "Tool holder section", idx));
                        collided = true;
                        break;
                    }
                }
                if collided {
                    break;
                }
                cumulative_dist += sec.length;
            }
            if collided {
                continue;
            }
        }

        // Check sample points along the holder axis from stickout to collet end
        let num_samples = 6;
        let sample_step = holder.collet_length.max(15.0) / (num_samples as f64);

        for s in 0..=num_samples {
            let dist = holder.stickout_length + (s as f64 * sample_step);
            let sample = HolderSample {
                centre: [x + ux * dist, y + uy * dist, z + uz * dist],
                radius: holder_radius,
                tip_z: z,
                axis: [ux, uy, uz],
            };
            if sample.collides(stock_bounds) {
                findings.push(sample.finding(stock_bounds, "Tool holder", idx));
                break;
            }
        }
    }

    findings
}
