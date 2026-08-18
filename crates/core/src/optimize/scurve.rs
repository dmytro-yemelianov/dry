//! Jerk-bounded 7-phase S-curve motion profile planner (D4.5, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Continuous-acceleration trajectory planner that bounds the rate of change of acceleration ($\frac{da}{dt} \le j_{\max}$),
//! eliminating high-frequency vibration, ringing, and motor resonance during high-speed printing and HSM CNC milling.

use serde::{Deserialize, Serialize};

/// Input parameters for an S-curve acceleration profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SCurveParams {
    /// Starting feedrate / velocity ($v_0$, mm/s).
    pub v_start: f64,
    /// Target feedrate / velocity ($v_1$, mm/s).
    pub v_target: f64,
    /// Maximum allowable acceleration ($a_{\max}$, $\text{mm/s}^2$).
    pub max_acceleration: f64,
    /// Maximum allowable jerk ($j_{\max}$, $\text{mm/s}^3$).
    pub max_jerk: f64,
}

/// Computed 7-phase S-curve motion trajectory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SCurveProfile {
    /// Duration of the jerk ramp-up phase ($t_j$, s).
    pub t_jerk_inc: f64,
    /// Duration of the constant acceleration phase ($t_a$, s).
    pub t_const_acc: f64,
    /// Duration of the jerk ramp-down phase ($t_j$, s).
    pub t_jerk_dec: f64,
    /// Total acceleration / transition duration ($T = 2t_j + t_a$, s).
    pub total_duration: f64,
    /// Total displacement / distance traveled during transition ($s$, mm).
    pub total_distance: f64,
    /// Peak acceleration reached during transition ($a_{\text{peak}}$, $\text{mm/s}^2$).
    pub peak_acceleration: f64,
}

/// Calculate a jerk-bounded S-curve acceleration / deceleration profile.
pub fn calculate_scurve_profile(params: &SCurveParams) -> Result<SCurveProfile, &'static str> {
    if params.max_acceleration <= 0.0 || !params.max_acceleration.is_finite() {
        return Err("max_acceleration must be positive and finite");
    }
    if params.max_jerk <= 0.0 || !params.max_jerk.is_finite() {
        return Err("max_jerk must be positive and finite");
    }
    if params.v_start < 0.0 || params.v_target < 0.0 {
        return Err("velocities must be non-negative");
    }

    let delta_v = (params.v_target - params.v_start).abs();
    if delta_v < 1e-9 {
        return Ok(SCurveProfile {
            t_jerk_inc: 0.0,
            t_const_acc: 0.0,
            t_jerk_dec: 0.0,
            total_duration: 0.0,
            total_distance: 0.0,
            peak_acceleration: 0.0,
        });
    }

    let a_max = params.max_acceleration;
    let j_max = params.max_jerk;

    // Velocity change required to reach a_max through jerk ramp-up and ramp-down
    let delta_v_j = (a_max * a_max) / j_max;

    let (t_j, t_a, peak_a) = if delta_v < delta_v_j {
        // a_max is not reached; triangular acceleration profile
        let peak_a = (delta_v * j_max).sqrt();
        let t_j = peak_a / j_max;
        (t_j, 0.0, peak_a)
    } else {
        // a_max is reached; trapezoidal acceleration profile
        let t_j = a_max / j_max;
        let t_a = (delta_v - delta_v_j) / a_max;
        (t_j, t_a, a_max)
    };

    let total_duration = 2.0 * t_j + t_a;
    let v_min = params.v_start.min(params.v_target);
    // Distance integral: s = v_min * T + 0.5 * delta_v * T
    let total_distance = v_min * total_duration + 0.5 * delta_v * total_duration;

    Ok(SCurveProfile {
        t_jerk_inc: t_j,
        t_const_acc: t_a,
        t_jerk_dec: t_j,
        total_duration,
        total_distance,
        peak_acceleration: peak_a,
    })
}
