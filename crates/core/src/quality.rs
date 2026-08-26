//! Surface roughness & cusp height quality analytics (D4.3, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Provides exact geometric formulas to predict surface finish on 3D contoured toolpaths:
//! - Theoretical scallop / cusp height ($h = R - \sqrt{R^2 - (s/2)^2}$) for ball-nose and bull-nose cutters.
//! - Arithmetic average surface roughness ($R_a$) estimation in micrometres ($\mu\text{m}$).

use serde::{Deserialize, Serialize};

/// Theoretical surface quality analytics for a sculptured toolpath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceQualityReport {
    /// Tool corner / ball radius in millimetres.
    pub tool_radius_mm: f64,
    /// Stepover distance between adjacent passes in millimetres.
    pub stepover_mm: f64,
    /// Maximum theoretical cusp / scallop height in millimetres.
    pub cusp_height_mm: f64,
    /// Estimated arithmetic surface roughness $R_a$ in micrometres ($\mu\text{m}$).
    pub roughness_ra_um: f64,
}

/// Calculate the theoretical cusp height in millimetres for a cutter of radius $R$ and stepover $s$.
pub fn calculate_cusp_height(tool_radius: f64, stepover: f64) -> Result<f64, &'static str> {
    if tool_radius <= 0.0 || !tool_radius.is_finite() {
        return Err("tool radius must be positive and finite");
    }
    if stepover <= 0.0 || !stepover.is_finite() {
        return Err("stepover must be positive and finite");
    }
    if stepover > 2.0 * tool_radius {
        return Err("stepover cannot exceed cutter diameter (leaves uncut ridge)");
    }

    let half_step = stepover * 0.5;
    let rad_sq = tool_radius * tool_radius;
    let half_sq = half_step * half_step;

    let cusp = tool_radius - (rad_sq - half_sq).sqrt();
    Ok(cusp)
}

/// Estimate $R_a$ surface roughness in micrometres ($\mu\text{m}$) from cusp height in millimetres.
///
/// Under idealized spherical scallop geometry, $R_a \approx \frac{h}{4}$ (in mm), converted to $\mu\text{m}$ ($\times 1000$).
pub fn estimate_surface_roughness_ra(cusp_height_mm: f64) -> f64 {
    (cusp_height_mm * 0.25) * 1000.0
}

/// Generate a complete surface quality report for a given cutter radius and stepover.
pub fn evaluate_surface_quality(
    tool_radius_mm: f64,
    stepover_mm: f64,
) -> Result<SurfaceQualityReport, &'static str> {
    let cusp_height_mm = calculate_cusp_height(tool_radius_mm, stepover_mm)?;
    let roughness_ra_um = estimate_surface_roughness_ra(cusp_height_mm);

    Ok(SurfaceQualityReport {
        tool_radius_mm,
        stepover_mm,
        cusp_height_mm,
        roughness_ra_um,
    })
}

#[path = "quality/mrr.rs"]
pub mod mrr;
pub use self::mrr::{calculate_mrr, estimate_cutting_power_kw, evaluate_mrr, MrrReport};
