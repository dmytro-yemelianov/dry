//! Multi-Robot Workcell & Dual-Arm Synchronization (Track E / Robotic Automation).
//!
//! Models coordinated dual-robot manufacturing workcells (e.g., dual KUKA/ABB/Fanuc 6-axis arms)
//! performing cooperative additive deposition and subtractive milling with real-time link collision checking.

use crate::emit::{Robot6AxisModel, RobotJoints6};
use serde::{Deserialize, Serialize};

/// Definition of a robotic manipulator inside a cooperative workcell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkcellRobot {
    /// Identifier / name (e.g., "Robot_1_Additive", "Robot_2_Subtractive").
    pub id: String,
    /// Kinematic Denavit-Hartenberg parameter model.
    pub model: Robot6AxisModel,
    /// Base position offset `[x, y, z]` in global workcell world coordinates (mm).
    pub base_offset: [f64; 3],
    /// Link bounding sphere radii `[r1, r2, r3, r4, r5, r6]` for collision checking (mm).
    pub link_radii: [f64; 6],
}

impl WorkcellRobot {
    pub fn new(id: impl Into<String>, model: Robot6AxisModel, base_offset: [f64; 3]) -> Self {
        Self {
            id: id.into(),
            model,
            base_offset,
            link_radii: [120.0, 100.0, 90.0, 80.0, 70.0, 60.0],
        }
    }
}

/// A synchronized time-stamped waypoint across two cooperative robots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DualRobotWaypoint {
    /// Timestamp along trajectory (seconds).
    pub time_s: f64,
    /// Joint state for Robot 1.
    pub joints_1: RobotJoints6,
    /// Joint state for Robot 2.
    pub joints_2: RobotJoints6,
    /// Synchronization flag ID (optional, e.g. 10 for barrier sync).
    pub sync_flag: Option<u32>,
}

/// Result of dual-robot workcell collision check.
#[derive(Debug, Clone, PartialEq)]
pub struct DualRobotCollisionResult {
    /// True if all robot links remain beyond the safety clearance margin.
    pub safe: bool,
    /// Minimum clearance distance observed between any link pair (mm).
    pub min_distance_mm: f64,
    /// The robot link index pair with closest approach (e.g. (3, 4)).
    pub closest_link_pair: (usize, usize),
}

/// A clearance verdict for input the check cannot evaluate.
///
/// Both clearance functions compare distances against a required margin, and **every comparison
/// against `NaN` is false** — so a non-finite joint angle or base offset left `safe` at its initial
/// `true` and reported `min_distance_mm = inf`. The check failed *open*: it answered "safe" about a
/// pose it had not evaluated, which is the one answer a collision check must never give by default.
///
/// These functions return a plain verdict rather than a `Result`, so the refusal is expressed in the
/// verdict itself: not safe, zero clearance. A caller reading `safe` gets the conservative answer,
/// and a caller reading `min_distance_mm` sees `0.0` rather than an `inf` that looks like abundant
/// room.
fn unevaluatable_clearance() -> DualRobotCollisionResult {
    DualRobotCollisionResult {
        safe: false,
        min_distance_mm: 0.0,
        closest_link_pair: (0, 0),
    }
}

/// Whether every number this check depends on is finite.
fn robot_inputs_are_finite(r: &WorkcellRobot, joints: &[&RobotJoints6], margin: f64) -> bool {
    margin.is_finite()
        && r.base_offset.iter().all(|v| v.is_finite())
        && r.link_radii.iter().all(|v| v.is_finite())
        && joints
            .iter()
            .all(|j| j.to_radians().iter().all(|v| v.is_finite()))
}

/// Checks safety clearance and collision between two 6-axis robots across all 6 intermediate link spheres.
pub fn check_dual_robot_clearance(
    r1: &WorkcellRobot,
    j1: &RobotJoints6,
    r2: &WorkcellRobot,
    j2: &RobotJoints6,
    safety_margin_mm: f64,
) -> DualRobotCollisionResult {
    // Fail closed. See `unevaluatable_clearance`: a NaN anywhere in the inputs made every distance
    // comparison false and the verdict came back `safe = true`.
    if !robot_inputs_are_finite(r1, &[j1], safety_margin_mm)
        || !robot_inputs_are_finite(r2, &[j2], safety_margin_mm)
    {
        return unevaluatable_clearance();
    }
    let links1 = r1.model.solve_all_link_positions(j1);
    let links2 = r2.model.solve_all_link_positions(j2);

    let mut min_distance = f64::INFINITY;
    let mut closest_pair = (5, 5);
    let mut overall_safe = true;

    for (i, l1) in links1.iter().enumerate() {
        let w1 = [
            l1[0] + r1.base_offset[0],
            l1[1] + r1.base_offset[1],
            l1[2] + r1.base_offset[2],
        ];

        for (j, l2) in links2.iter().enumerate() {
            let w2 = [
                l2[0] + r2.base_offset[0],
                l2[1] + r2.base_offset[1],
                l2[2] + r2.base_offset[2],
            ];

            let dx = w1[0] - w2[0];
            let dy = w1[1] - w2[1];
            let dz = w1[2] - w2[2];
            let dist = libm::sqrt(dx * dx + dy * dy + dz * dz);

            let required_clearance = r1.link_radii[i] + r2.link_radii[j] + safety_margin_mm;
            if dist < required_clearance {
                overall_safe = false;
            }

            if dist < min_distance {
                min_distance = dist;
                closest_pair = (i, j);
            }
        }
    }

    DualRobotCollisionResult {
        safe: overall_safe,
        min_distance_mm: min_distance,
        closest_link_pair: closest_pair,
    }
}

/// Generates KUKA KRL multi-channel synchronization commands for dual robots.
pub fn emit_dual_robot_sync_krl(flag_id: u32, is_master: bool) -> Vec<String> {
    if is_master {
        vec![
            format!("; --- Sync Barrier (Master) Flag {flag_id} ---"),
            format!("$FLAG[{flag_id}] = TRUE"),
            format!("WAIT FOR NOT $FLAG[{flag_id}]"),
        ]
    } else {
        vec![
            format!("; --- Sync Barrier (Slave) Flag {flag_id} ---"),
            format!("WAIT FOR $FLAG[{flag_id}]"),
            format!("$FLAG[{flag_id}] = FALSE"),
        ]
    }
}

/// Generates ABB RAPID MultiMove multi-task synchronization barrier commands.
pub fn emit_dual_robot_sync_rapid(sync_id: &str, task_list: &[&str]) -> Vec<String> {
    let task_ids = if task_list.is_empty() {
        "tasks_all".to_string()
    } else {
        task_list.join(", ")
    };
    vec![
        format!("! --- ABB MultiMove Sync Barrier \"{sync_id}\" ---"),
        format!("WaitSyncTask {sync_id}, [{task_ids}];"),
    ]
}

/// Interpolates joint states linearly between two dual-robot waypoints.
pub fn interpolate_dual_robot_waypoint(
    w1: &DualRobotWaypoint,
    w2: &DualRobotWaypoint,
    t: f64,
) -> DualRobotWaypoint {
    let alpha = if (w2.time_s - w1.time_s).abs() < 1e-6 {
        0.0
    } else {
        ((t - w1.time_s) / (w2.time_s - w1.time_s)).clamp(0.0, 1.0)
    };

    let lerp = |a: f64, b: f64| a + alpha * (b - a);

    let j1 = RobotJoints6 {
        j1_deg: lerp(w1.joints_1.j1_deg, w2.joints_1.j1_deg),
        j2_deg: lerp(w1.joints_1.j2_deg, w2.joints_1.j2_deg),
        j3_deg: lerp(w1.joints_1.j3_deg, w2.joints_1.j3_deg),
        j4_deg: lerp(w1.joints_1.j4_deg, w2.joints_1.j4_deg),
        j5_deg: lerp(w1.joints_1.j5_deg, w2.joints_1.j5_deg),
        j6_deg: lerp(w1.joints_1.j6_deg, w2.joints_1.j6_deg),
    };

    let j2 = RobotJoints6 {
        j1_deg: lerp(w1.joints_2.j1_deg, w2.joints_2.j1_deg),
        j2_deg: lerp(w1.joints_2.j2_deg, w2.joints_2.j2_deg),
        j3_deg: lerp(w1.joints_2.j3_deg, w2.joints_2.j3_deg),
        j4_deg: lerp(w1.joints_2.j4_deg, w2.joints_2.j4_deg),
        j5_deg: lerp(w1.joints_2.j5_deg, w2.joints_2.j5_deg),
        j6_deg: lerp(w1.joints_2.j6_deg, w2.joints_2.j6_deg),
    };

    DualRobotWaypoint {
        time_s: t,
        joints_1: j1,
        joints_2: j2,
        sync_flag: if alpha >= 1.0 { w2.sync_flag } else { None },
    }
}

/// Dynamically scales cooperative velocity factor based on minimum observed inter-arm distance.
///
/// When inter-arm clearance approaches `critical_dist_mm`, feedrate automatically scales down
/// towards `min_scale` to prevent high-speed collisions in shared workspaces.
pub fn calculate_clearance_velocity_scale(
    current_dist_mm: f64,
    critical_dist_mm: f64,
    safe_dist_mm: f64,
    min_scale: f64,
) -> f64 {
    // Fail slow. `current_dist_mm >= safe_dist_mm` is false for a `NaN` clearance, and so is the
    // `<=` below, so a non-finite input fell through to the interpolation and returned `NaN` — a
    // scale factor that silently destroys whatever feedrate it multiplies. The conservative answer
    // for a clearance nobody can measure is the slowest one this function is allowed to return,
    // which is the same answer it gives inside the critical distance.
    if !current_dist_mm.is_finite()
        || !critical_dist_mm.is_finite()
        || !safe_dist_mm.is_finite()
        || !min_scale.is_finite()
    {
        // Not `min_scale.clamp(..)`: `clamp` returns NaN for a NaN self, so when `min_scale` is
        // itself the non-finite argument, clamping it would return the NaN this branch exists to
        // avoid. `SLOWEST` is the floor the finite path already clamps to.
        const SLOWEST: f64 = 0.05;
        return if min_scale.is_finite() {
            min_scale.clamp(SLOWEST, 1.0)
        } else {
            SLOWEST
        };
    }
    if current_dist_mm >= safe_dist_mm {
        1.0
    } else if current_dist_mm <= critical_dist_mm {
        min_scale.clamp(0.05, 1.0)
    } else {
        let frac = (current_dist_mm - critical_dist_mm) / (safe_dist_mm - critical_dist_mm);
        (min_scale + frac * (1.0 - min_scale)).clamp(min_scale, 1.0)
    }
}

/// Minimum Euclidean distance between two 3D line segments `[p1, p2]` and `[q1, q2]`.
pub fn segment_to_segment_distance_3d(
    p1: [f64; 3],
    p2: [f64; 3],
    q1: [f64; 3],
    q2: [f64; 3],
) -> f64 {
    let u = [p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]];
    let v = [q2[0] - q1[0], q2[1] - q1[1], q2[2] - q1[2]];
    let w = [p1[0] - q1[0], p1[1] - q1[1], p1[2] - q1[2]];

    let a = u[0] * u[0] + u[1] * u[1] + u[2] * u[2];
    let b = u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let c = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    let d = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
    let e = v[0] * w[0] + v[1] * w[1] + v[2] * w[2];

    let d_denom = a * c - b * b;
    let (t_n, t_d) = if d_denom < 1e-12 {
        (e, c)
    } else {
        let s_n = b * e - c * d;
        if s_n < 0.0 {
            (e, c)
        } else if s_n > d_denom {
            (e + b, c)
        } else {
            (a * e - b * d, d_denom)
        }
    };

    let tc = if t_n < 0.0 {
        0.0
    } else if t_n > t_d {
        1.0
    } else if t_d.abs() < 1e-12 {
        0.0
    } else {
        t_n / t_d
    };

    let sc = if a.abs() < 1e-12 {
        0.0
    } else {
        ((b * tc - d) / a).clamp(0.0, 1.0)
    };

    let dx = w[0] + (sc * u[0]) - (tc * v[0]);
    let dy = w[1] + (sc * u[1]) - (tc * v[1]);
    let dz = w[2] + (sc * u[2]) - (tc * v[2]);

    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Continuous swept-capsule collision check between two dual-robot motion spans.
pub fn check_continuous_dual_robot_trajectory(
    r1: &WorkcellRobot,
    w1_start: &DualRobotWaypoint,
    w1_end: &DualRobotWaypoint,
    r2: &WorkcellRobot,
    w2_start: &DualRobotWaypoint,
    w2_end: &DualRobotWaypoint,
    safety_margin_mm: f64,
) -> DualRobotCollisionResult {
    // Fail closed, for the same reason as `check_dual_robot_clearance`.
    if !robot_inputs_are_finite(
        r1,
        &[&w1_start.joints_1, &w1_end.joints_1],
        safety_margin_mm,
    ) || !robot_inputs_are_finite(
        r2,
        &[&w2_start.joints_2, &w2_end.joints_2],
        safety_margin_mm,
    ) {
        return unevaluatable_clearance();
    }
    let links1_start = r1.model.solve_all_link_positions(&w1_start.joints_1);
    let links1_end = r1.model.solve_all_link_positions(&w1_end.joints_1);
    let links2_start = r2.model.solve_all_link_positions(&w2_start.joints_2);
    let links2_end = r2.model.solve_all_link_positions(&w2_end.joints_2);

    let mut min_distance = f64::INFINITY;
    let mut closest_pair = (5, 5);
    let mut overall_safe = true;

    for i in 0..links1_start.len().min(links1_end.len()) {
        let p1 = [
            links1_start[i][0] + r1.base_offset[0],
            links1_start[i][1] + r1.base_offset[1],
            links1_start[i][2] + r1.base_offset[2],
        ];
        let p2 = [
            links1_end[i][0] + r1.base_offset[0],
            links1_end[i][1] + r1.base_offset[1],
            links1_end[i][2] + r1.base_offset[2],
        ];

        for j in 0..links2_start.len().min(links2_end.len()) {
            let q1 = [
                links2_start[j][0] + r2.base_offset[0],
                links2_start[j][1] + r2.base_offset[1],
                links2_start[j][2] + r2.base_offset[2],
            ];
            let q2 = [
                links2_end[j][0] + r2.base_offset[0],
                links2_end[j][1] + r2.base_offset[1],
                links2_end[j][2] + r2.base_offset[2],
            ];

            let dist = segment_to_segment_distance_3d(p1, p2, q1, q2);
            let required_clearance = r1.link_radii[i] + r2.link_radii[j] + safety_margin_mm;

            if dist < required_clearance {
                overall_safe = false;
            }

            if dist < min_distance {
                min_distance = dist;
                closest_pair = (i, j);
            }
        }
    }

    DualRobotCollisionResult {
        safe: overall_safe,
        min_distance_mm: min_distance,
        closest_link_pair: closest_pair,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_to_segment_distance_parallel_and_skew() {
        // Parallel segments separated by 10mm in Z
        let d_parallel = segment_to_segment_distance_3d(
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 10.0],
        );
        assert!((d_parallel - 10.0).abs() < 1e-5);

        // Orthogonal skew segments intersecting in XY projection with 5mm Z clearance
        let d_skew = segment_to_segment_distance_3d(
            [-5.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [0.0, -5.0, 5.0],
            [0.0, 5.0, 5.0],
        );
        assert!((d_skew - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_dual_robot_workcell_safety_check() {
        let model = Robot6AxisModel::kuka_kr6_r900();
        let r1 = WorkcellRobot::new("Robot1", model.clone(), [0.0, 0.0, 0.0]);
        let r2 = WorkcellRobot::new("Robot2", model, [1500.0, 0.0, 0.0]);

        let j_home = RobotJoints6 {
            j1_deg: 0.0,
            j2_deg: 0.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        };
        let result = check_dual_robot_clearance(&r1, &j_home, &r2, &j_home, 50.0);

        assert!(result.safe, "Robots 1500mm apart at home must be clear");
        assert!(result.min_distance_mm > 500.0);
    }

    #[test]
    fn test_dual_robot_sync_emission() {
        let master_lines = emit_dual_robot_sync_krl(10, true);
        let slave_lines = emit_dual_robot_sync_krl(10, false);

        assert_eq!(master_lines.len(), 3);
        assert_eq!(slave_lines.len(), 3);
        assert!(master_lines[1].contains("$FLAG[10] = TRUE"));
        assert!(slave_lines[1].contains("WAIT FOR $FLAG[10]"));
    }
}
