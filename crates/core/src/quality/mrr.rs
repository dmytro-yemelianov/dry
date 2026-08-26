//! Material Removal Rate (MRR) & spindle cutting power analytics (D4.4, `docs/04-tasks.md` — unplanned series D2–D4).
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
///
/// Returns `0.0` for any input that is not a positive finite number. `<= 0.0` alone is false for
/// NaN, so a NaN dimension used to flow straight through into the product and out into the report.
pub fn calculate_mrr(depth_of_cut_mm: f64, width_of_cut_mm: f64, feedrate_mm_min: f64) -> f64 {
    let positive_finite = |v: f64| v.is_finite() && v > 0.0;
    if !positive_finite(depth_of_cut_mm)
        || !positive_finite(width_of_cut_mm)
        || !positive_finite(feedrate_mm_min)
    {
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
///
/// Returns `0.0` for any input outside its stated domain, including non-finite ones.
pub fn estimate_cutting_power_kw(
    mrr_cm3_min: f64,
    specific_cutting_force_n_mm2: f64,
    efficiency: f64,
) -> f64 {
    let positive_finite = |v: f64| v.is_finite() && v > 0.0;
    if !positive_finite(mrr_cm3_min)
        || !positive_finite(specific_cutting_force_n_mm2)
        || !positive_finite(efficiency)
    {
        return 0.0;
    }
    // Power scales as 1/eta, so raising a too-small efficiency *lowers* the estimate. The previous
    // `clamp(0.1, 1.0)` silently turned eta = 0.05 into 0.1 and halved the predicted power draw —
    // an under-estimate of what the spindle has to supply, which is the unsafe direction to round.
    // Only the physically impossible upper end is clamped, and that errs high.
    let eta = efficiency.min(1.0);
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
