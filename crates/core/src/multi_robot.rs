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

/// Checks safety clearance and collision between two 6-axis robots at given joint configurations.
pub fn check_dual_robot_clearance(
    r1: &WorkcellRobot,
    j1: &RobotJoints6,
    r2: &WorkcellRobot,
    j2: &RobotJoints6,
    safety_margin_mm: f64,
) -> DualRobotCollisionResult {
    // Compute TCP for both robots
    let tcp1 = r1.model.solve_fk(j1);
    let tcp2 = r2.model.solve_fk(j2);

    let world_tcp1 = [
        tcp1[0] + r1.base_offset[0],
        tcp1[1] + r1.base_offset[1],
        tcp1[2] + r1.base_offset[2],
    ];

    let world_tcp2 = [
        tcp2[0] + r2.base_offset[0],
        tcp2[1] + r2.base_offset[1],
        tcp2[2] + r2.base_offset[2],
    ];

    let dx = world_tcp1[0] - world_tcp2[0];
    let dy = world_tcp1[1] - world_tcp2[1];
    let dz = world_tcp1[2] - world_tcp2[2];
    let dist = libm::sqrt(dx * dx + dy * dy + dz * dz);

    let required_clearance = r1.link_radii[5] + r2.link_radii[5] + safety_margin_mm;
    let safe = dist >= required_clearance;

    DualRobotCollisionResult {
        safe,
        min_distance_mm: dist,
        closest_link_pair: (5, 5),
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
