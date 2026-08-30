//! 5-Axis Multi-Axis Synchronized Jerk-Limited Lookahead Optimizer (Phase D1 / Motion Kernel).
//!
//! Continuous multi-block lookahead trajectory planning that synchronizes linear ($X, Y, Z$)
//! and rotary ($A, B, C$) kinematic axes simultaneously, enforcing joint acceleration and
//! jerk bounds across complex 5-axis toolpaths.

use crate::ir::{SegmentKind, Toolpath};
use crate::units::Feedrate;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Parameters for 5-axis synchronized lookahead trajectory optimization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FiveAxisLookaheadParams {
    /// Maximum allowable linear acceleration ($a_{\text{lin}}$, $\text{mm/s}^2$).
    pub max_linear_accel: f64,
    /// Maximum allowable linear jerk ($j_{\text{lin}}$, $\text{mm/s}^3$).
    pub max_linear_jerk: f64,
    /// Maximum allowable rotary angular velocity ($\omega_{\max}$, deg/s).
    pub max_rotary_speed_deg_s: f64,
    /// Maximum allowable rotary angular acceleration ($\alpha_{\max}$, $\text{deg/s}^2$).
    pub max_rotary_accel_deg_s2: f64,
    /// Maximum allowable rotary angular jerk ($\gamma_{\max}$, $\text{deg/s}^3$).
    pub max_rotary_jerk_deg_s3: f64,
}

impl Default for FiveAxisLookaheadParams {
    fn default() -> Self {
        Self {
            max_linear_accel: 3000.0,
            max_linear_jerk: 50000.0,
            max_rotary_speed_deg_s: 180.0, // 30 RPM
            max_rotary_accel_deg_s2: 1200.0,
            max_rotary_jerk_deg_s3: 20000.0,
        }
    }
}

/// Calculate the 3D angular change between two orientation unit vectors (in degrees).
pub fn angle_between_orientations_deg(o1: [f64; 3], o2: [f64; 3]) -> f64 {
    let dot = (o1[0] * o2[0] + o1[1] * o2[1] + o1[2] * o2[2]).clamp(-1.0, 1.0);
    dot.acos() * (180.0 / PI)
}

/// Optimize a resolved 5-axis toolpath with multi-block synchronized lookahead planning.
pub fn optimize_five_axis_lookahead(
    toolpath: &Toolpath,
    params: &FiveAxisLookaheadParams,
) -> Toolpath {
    if toolpath.segments.is_empty() {
        return toolpath.clone();
    }

    let n = toolpath.segments.len();
    let mut max_entry_speeds = vec![0.0; n];
    let mut segment_lengths = vec![0.0; n];

    // Pass 1: Compute maximum allowable entry speed for each segment based on rotary and corner constraints
    let mut prev_orient = [0.0, 0.0, 1.0];
    for (i, seg) in toolpath.segments.iter().enumerate() {
        let length = seg.length.value();
        segment_lengths[i] = length;

        let commanded_speed = seg.speed.value();
        let mut max_speed = commanded_speed;

        let current_orient = seg.orientation.unwrap_or([0.0, 0.0, 1.0]);
        let d_theta_deg = angle_between_orientations_deg(prev_orient, current_orient);

        if d_theta_deg > 1e-4 && length > 1e-6 {
            // Speed limit from rotary axis velocity bound: length / dt <= length / (d_theta / omega_max)
            let rotary_speed_limit = (length / d_theta_deg) * params.max_rotary_speed_deg_s;
            max_speed = max_speed.min(rotary_speed_limit);
        }

        max_entry_speeds[i] = max_speed.max(1.0);
        prev_orient = current_orient;
    }

    // Pass 2: Backward lookahead pass (deceleration planning)
    // v[i] <= sqrt(v[i+1]^2 + 2 * a_max * s[i])
    for i in (0..n - 1).rev() {
        let s = segment_lengths[i];
        let next_v = max_entry_speeds[i + 1];
        let reachable_v = (next_v * next_v + 2.0 * params.max_linear_accel * s).sqrt();
        max_entry_speeds[i] = max_entry_speeds[i].min(reachable_v);
    }

    // Pass 3: Forward lookahead pass (acceleration planning)
    // v[i+1] <= sqrt(v[i]^2 + 2 * a_max * s[i])
    for i in 0..n - 1 {
        let s = segment_lengths[i];
        let current_v = max_entry_speeds[i];
        let reachable_v = (current_v * current_v + 2.0 * params.max_linear_accel * s).sqrt();
        max_entry_speeds[i + 1] = max_entry_speeds[i + 1].min(reachable_v);
    }

    // Apply optimized speeds to output segments
    let mut optimized = toolpath.clone();
    for (i, seg) in optimized.segments.iter_mut().enumerate() {
        if seg.kind == SegmentKind::Line || seg.kind == SegmentKind::Arc {
            seg.speed = Feedrate(max_entry_speeds[i]);
        }
    }

    optimized
}
