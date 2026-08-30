//! Digital Twin Physics & Thermal / Deflection Simulator.
//!
//! Multi-physics analytical simulation of metal cutting and polymer deposition mechanics:
//! instantaneous cutting forces (Kienzle / Merchant shear model), cantilever tool deflection,
//! adiabatic shear zone temperature rise, Taylor tool wear life, and chatter stability boundaries.

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Workpiece material property database.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WorkpieceMaterial {
    Aluminum6061,
    Steel4140,
    TitaniumTi6Al4V,
    Inconel718,
    ThermoplasticPLA,
    ThermoplasticPEEK,
}

impl WorkpieceMaterial {
    /// Specific cutting force coefficient ($k_c$, in $\text{N/mm}^2$).
    pub fn specific_cutting_force_n_mm2(self) -> f64 {
        match self {
            Self::Aluminum6061 => 700.0,
            Self::Steel4140 => 2100.0,
            Self::TitaniumTi6Al4V => 2800.0,
            Self::Inconel718 => 3500.0,
            Self::ThermoplasticPLA => 85.0,
            Self::ThermoplasticPEEK => 220.0,
        }
    }

    /// Density ($\rho$, in $\text{kg/m}^3$).
    pub fn density_kg_m3(self) -> f64 {
        match self {
            Self::Aluminum6061 => 2700.0,
            Self::Steel4140 => 7850.0,
            Self::TitaniumTi6Al4V => 4430.0,
            Self::Inconel718 => 8190.0,
            Self::ThermoplasticPLA => 1240.0,
            Self::ThermoplasticPEEK => 1320.0,
        }
    }

    /// Specific heat capacity ($C_p$, in $\text{J}/(\text{kg}\cdot\text{K})$).
    pub fn specific_heat_j_kg_k(self) -> f64 {
        match self {
            Self::Aluminum6061 => 896.0,
            Self::Steel4140 => 486.0,
            Self::TitaniumTi6Al4V => 526.0,
            Self::Inconel718 => 435.0,
            Self::ThermoplasticPLA => 1800.0,
            Self::ThermoplasticPEEK => 1400.0,
        }
    }

    /// Taylor tool life constant $C$ (m/min).
    pub fn taylor_constant_c(self) -> f64 {
        match self {
            Self::Aluminum6061 => 350.0,
            Self::Steel4140 => 180.0,
            Self::TitaniumTi6Al4V => 75.0,
            Self::Inconel718 => 40.0,
            Self::ThermoplasticPLA => 800.0,
            Self::ThermoplasticPEEK => 450.0,
        }
    }

    /// Taylor tool life exponent $n$.
    pub fn taylor_exponent_n(self) -> f64 {
        match self {
            Self::Aluminum6061 => 0.30,
            Self::Steel4140 => 0.20,
            Self::TitaniumTi6Al4V => 0.14,
            Self::Inconel718 => 0.11,
            Self::ThermoplasticPLA => 0.45,
            Self::ThermoplasticPEEK => 0.35,
        }
    }
}

/// Cutting tool physical parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuttingToolGeometry {
    pub diameter_mm: f64,
    pub flute_count: usize,
    pub stickout_length_mm: f64,
    pub core_diameter_ratio: f64,
    pub modulus_gpa: f64,
    pub corner_radius_mm: f64,
}

impl Default for CuttingToolGeometry {
    fn default() -> Self {
        Self {
            diameter_mm: 10.0,
            flute_count: 4,
            stickout_length_mm: 35.0,
            core_diameter_ratio: 0.75,
            modulus_gpa: 600.0, // Solid tungsten carbide
            corner_radius_mm: 0.5,
        }
    }
}

/// Dynamic machining operation parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachiningOperationParams {
    pub spindle_rpm: f64,
    pub feedrate_mm_min: f64,
    pub axial_depth_ap_mm: f64,
    pub radial_depth_ae_mm: f64,
    pub ambient_temp_c: f64,
}

/// Physical simulation output metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsAnalysisReport {
    pub cutting_speed_m_min: f64,
    pub feed_per_tooth_mm: f64,
    pub material_removal_rate_cm3_min: f64,
    pub tangential_force_n: f64,
    pub spindle_power_kw: f64,
    pub spindle_torque_nm: f64,
    pub tool_deflection_um: f64,
    /// Shear-zone temperature (°C). **Clamped:** the modelled rise is bounded to 1200 °C above
    /// ambient, so a result at `ambient + 1200` is the ceiling rather than a prediction — check
    /// [`Self::model_saturated`] before reading it as one.
    pub shear_temperature_c: f64,
    /// Taylor tool life (minutes). **Clamped** to `[0.1, 10000.0]`; `0.1` is the floor, not an
    /// estimate that the tool lasts six seconds. See [`Self::model_saturated`].
    pub estimated_tool_life_min: f64,
    pub surface_roughness_ra_um: f64,
    pub chatter_risk: bool,
    /// True when a clamp bound the result, so at least one field above is a guardrail rather than a
    /// computed value.
    ///
    /// The clamps are deliberate — they stop absurd inputs producing absurd numbers — but a clamped
    /// value is indistinguishable from a real one in the report, and both of these are read as
    /// process predictions. Titanium at 251 m/min (roughly five times any sane cutting speed for it)
    /// saturates *both* at once: `shear_temperature_c` pegs at 1220 and `estimated_tool_life_min` at
    /// 0.1. Those are the model declining to answer, and a caller is entitled to know that.
    ///
    /// Same principle as [`crate::verify::Report::rules_evaluated`]: a result must say what it
    /// actually established.
    pub model_saturated: bool,
}

/// Run full digital twin physics simulation.
pub fn analyze_machining_physics(
    tool: &CuttingToolGeometry,
    material: WorkpieceMaterial,
    params: &MachiningOperationParams,
) -> PhysicsAnalysisReport {
    let d_mm = tool.diameter_mm.max(0.1);
    let rpm = params.spindle_rpm.max(1.0);
    let z = tool.flute_count.max(1) as f64;
    let vf = params.feedrate_mm_min.max(1.0);
    let ap = params.axial_depth_ap_mm.max(0.0);
    let ae = params.radial_depth_ae_mm.max(0.0).min(d_mm);

    // 1. Kinematics
    let vc = (PI * d_mm * rpm) / 1000.0; // m/min
    let fz = vf / (rpm * z); // mm/tooth
    let mrr = (ap * ae * vf) / 1000.0; // cm^3/min

    // 2. Cutting forces (Kienzle model)
    let kc = material.specific_cutting_force_n_mm2();
    let chip_area = ap * fz * (ae / d_mm).sqrt().min(1.0);
    let ft = kc * chip_area; // Tangential force (N)

    // 3. Power & Torque
    let power_kw = (ft * (vc / 60.0)) / 1000.0; // kW
    let torque_nm = (ft * (d_mm / 2.0)) / 1000.0; // N*m

    // 4. Cantilever deflection: delta = (F * L^3) / (3 * E * I)
    let core_d = d_mm * tool.core_diameter_ratio;
    let i_second_moment = (PI * core_d.powi(4)) / 64.0; // mm^4
    let e_n_mm2 = tool.modulus_gpa * 1000.0; // GPa -> N/mm^2
    let l_mm = tool.stickout_length_mm;
    let deflection_mm = (ft * l_mm.powi(3)) / (3.0 * e_n_mm2 * i_second_moment);
    let deflection_um = deflection_mm * 1000.0;

    // 5. Thermal shear zone temperature
    let rho = material.density_kg_m3();
    let cp = material.specific_heat_j_kg_k();
    let thermal_rise = (0.85 * ft * (vc / 60.0)) / (rho * (mrr * 1e-6 / 60.0).max(1e-12) * cp);
    const MAX_MODELLED_RISE_C: f64 = 1200.0;
    let clamped_rise = thermal_rise.clamp(0.0, MAX_MODELLED_RISE_C);
    let temp_saturated = thermal_rise > MAX_MODELLED_RISE_C || !thermal_rise.is_finite();
    let shear_temp_c = params.ambient_temp_c + clamped_rise;

    // 6. Taylor tool life: T = (C / vc)^(1/n)
    let c_taylor = material.taylor_constant_c();
    let n_taylor = material.taylor_exponent_n();
    const MIN_TOOL_LIFE_MIN: f64 = 0.1;
    const MAX_TOOL_LIFE_MIN: f64 = 10000.0;
    let (tool_life_min, life_saturated) = if vc > 1.0 {
        let raw = (c_taylor / vc).powf(1.0 / n_taylor);
        (
            raw.clamp(MIN_TOOL_LIFE_MIN, MAX_TOOL_LIFE_MIN),
            !(MIN_TOOL_LIFE_MIN..=MAX_TOOL_LIFE_MIN).contains(&raw) || !raw.is_finite(),
        )
    } else {
        // Below 1 m/min there is no meaningful Taylor curve; the ceiling is a stand-in, not a result.
        (MAX_TOOL_LIFE_MIN, true)
    };

    // 7. Theoretical surface roughness: Ra = fz^2 / (32 * r_eps)
    let r_eps = tool.corner_radius_mm.max(0.1);
    let ra_um = (fz.powi(2) / (32.0 * r_eps)) * 1000.0;

    // 8. Chatter risk
    let ld_ratio = l_mm / d_mm;
    let chatter_risk = ld_ratio > 4.5 || deflection_um > 25.0;

    PhysicsAnalysisReport {
        cutting_speed_m_min: vc,
        feed_per_tooth_mm: fz,
        material_removal_rate_cm3_min: mrr,
        tangential_force_n: ft,
        spindle_power_kw: power_kw,
        spindle_torque_nm: torque_nm,
        tool_deflection_um: deflection_um,
        shear_temperature_c: shear_temp_c,
        estimated_tool_life_min: tool_life_min,
        surface_roughness_ra_um: ra_um,
        chatter_risk,
        model_saturated: temp_saturated || life_saturated,
    }
}
