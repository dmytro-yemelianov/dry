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

/// Checks safety clearance and collision between two 6-axis robots across all 6 intermediate link spheres.
pub fn check_dual_robot_clearance(
    r1: &WorkcellRobot,
    j1: &RobotJoints6,
    r2: &WorkcellRobot,
    j2: &RobotJoints6,
    safety_margin_mm: f64,
) -> DualRobotCollisionResult {
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
    if current_dist_mm >= safe_dist_mm {
        1.0
    } else if current_dist_mm <= critical_dist_mm {
        min_scale.clamp(0.05, 1.0)
    } else {
        let frac = (current_dist_mm - critical_dist_mm) / (safe_dist_mm - critical_dist_mm);
        (min_scale + frac * (1.0 - min_scale)).clamp(min_scale, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
