//! Material Removal Rate (MRR) & spindle cutting power analytics (D4.4, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Provides instantaneous and volumetric cutting power analytics:
//! - $\text{MRR} = a_p \times a_e \times v_f$ ($\text{cm}^3/\text{min}$)
//! - Spindle Cutting Power $P_c = \frac{\text{MRR} \times k_c}{60 \times 10^3 \times \eta}$ ($\text{kW}$)

use serde::{Deserialize, Serialize};

/// Report of material removal rate and required spindle cutting power.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MrrReport {
    /// Axial depth of cut $a_p$ in millimetres.
    pub depth_of_cut_mm: f64,
    /// Radial width of cut $a_e$ in millimetres (stepover).
    pub width_of_cut_mm: f64,
    /// Cutting feedrate $v_f$ in millimetres per minute.
    pub feedrate_mm_min: f64,
    /// Volumetric material removal rate in cubic centimetres per minute ($\text{cm}^3/\text{min}$).
    pub mrr_cm3_min: f64,
    /// Estimated mechanical spindle power draw in kilowatts ($\text{kW}$).
    pub cutting_power_kw: f64,
}

/// Calculate volumetric Material Removal Rate (MRR) in $\text{cm}^3/\text{min}$.
///
/// Formula: $\text{MRR} = \frac{a_p \times a_e \times v_f}{1000.0}$
pub fn calculate_mrr(depth_of_cut_mm: f64, width_of_cut_mm: f64, feedrate_mm_min: f64) -> f64 {
    if depth_of_cut_mm <= 0.0 || width_of_cut_mm <= 0.0 || feedrate_mm_min <= 0.0 {
        return 0.0;
    }
    // mm^3/min to cm^3/min (/ 1000)
    (depth_of_cut_mm * width_of_cut_mm * feedrate_mm_min) / 1000.0
}

/// Estimate required spindle cutting power in kilowatts ($\text{kW}$).
///
/// Formula: $P_c = \frac{\text{MRR} \times k_c}{60 \times 1000 \times \eta}$
/// - `mrr_cm3_min`: Volumetric MRR in $\text{cm}^3/\text{min}$.
/// - `specific_cutting_force_n_mm2`: Material specific cutting force $k_c$ in $\text{N/mm}^2$ (e.g. 700 for Al 6061, 2100 for Steel 4140).
/// - `efficiency`: Spindle / drivetrain mechanical efficiency $\eta \in (0, 1]$ (default ~0.85).
pub fn estimate_cutting_power_kw(
    mrr_cm3_min: f64,
    specific_cutting_force_n_mm2: f64,
    efficiency: f64,
) -> f64 {
    if mrr_cm3_min <= 0.0 || specific_cutting_force_n_mm2 <= 0.0 {
        return 0.0;
    }
    let eta = efficiency.clamp(0.1, 1.0);
    // (cm^3/min * N/mm^2) / (60 * 1000 * eta) = kW
    (mrr_cm3_min * specific_cutting_force_n_mm2) / (60.0 * 1000.0 * eta)
}

/// Generate a full MRR and spindle power analytics report.
pub fn evaluate_mrr(
    depth_of_cut_mm: f64,
    width_of_cut_mm: f64,
    feedrate_mm_min: f64,
    specific_cutting_force_n_mm2: f64,
    efficiency: f64,
) -> MrrReport {
    let mrr_cm3_min = calculate_mrr(depth_of_cut_mm, width_of_cut_mm, feedrate_mm_min);
    let cutting_power_kw =
        estimate_cutting_power_kw(mrr_cm3_min, specific_cutting_force_n_mm2, efficiency);

    MrrReport {
        depth_of_cut_mm,
        width_of_cut_mm,
        feedrate_mm_min,
        mrr_cm3_min,
        cutting_power_kw,
    }
}
