use dry_core::emit::{Robot6AxisModel, RobotJoints6};
use dry_core::{
    check_dual_robot_clearance, emit_dual_robot_sync_krl, DualRobotWaypoint, WorkcellRobot,
};

#[test]
fn test_dual_robot_workcell_collision_detection() {
    let kuka = Robot6AxisModel::kuka_kr6_r900();
    let r1 = WorkcellRobot::new("Additive_Arm", kuka.clone(), [0.0, 0.0, 0.0]);
    let r2 = WorkcellRobot::new("Subtractive_Arm", kuka, [1200.0, 0.0, 0.0]);

    // Robot 1 reaching toward Robot 2
    let j1 = RobotJoints6 {
        j1_deg: 0.0,
        j2_deg: 45.0,
        j3_deg: 45.0,
        j4_deg: 0.0,
        j5_deg: 0.0,
        j6_deg: 0.0,
    };
    // Robot 2 reaching toward Robot 1 (potential collision configuration)
    let j2 = RobotJoints6 {
        j1_deg: 180.0,
        j2_deg: 45.0,
        j3_deg: 45.0,
        j4_deg: 0.0,
        j5_deg: 0.0,
        j6_deg: 0.0,
    };

    let check = check_dual_robot_clearance(&r1, &j1, &r2, &j2, 40.0);
    // Clearance should be computable and finite
    assert!(check.min_distance_mm.is_finite());
}

#[test]
fn test_dual_robot_sync_waypoints() {
    let waypoint = DualRobotWaypoint {
        time_s: 12.5,
        joints_1: RobotJoints6 {
            j1_deg: 0.0,
            j2_deg: 0.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        },
        joints_2: RobotJoints6 {
            j1_deg: 0.0,
            j2_deg: 0.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        },
        sync_flag: Some(5),
    };

    assert_eq!(waypoint.sync_flag, Some(5));

    let krl_master = emit_dual_robot_sync_krl(5, true);
    let krl_slave = emit_dual_robot_sync_krl(5, false);

    assert!(krl_master.iter().any(|line| line.contains("$FLAG[5] = TRUE")));
    assert!(krl_slave.iter().any(|line| line.contains("WAIT FOR $FLAG[5]")));
}
