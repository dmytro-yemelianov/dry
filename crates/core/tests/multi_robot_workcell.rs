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

#[test]
fn test_dual_robot_waypoint_interpolation_and_velocity_scaling() {
    use dry_core::{calculate_clearance_velocity_scale, interpolate_dual_robot_waypoint};

    let w1 = DualRobotWaypoint {
        time_s: 0.0,
        joints_1: RobotJoints6 { j1_deg: 0.0, j2_deg: 0.0, j3_deg: 0.0, j4_deg: 0.0, j5_deg: 0.0, j6_deg: 0.0 },
        joints_2: RobotJoints6 { j1_deg: 100.0, j2_deg: 0.0, j3_deg: 0.0, j4_deg: 0.0, j5_deg: 0.0, j6_deg: 0.0 },
        sync_flag: None,
    };

    let w2 = DualRobotWaypoint {
        time_s: 10.0,
        joints_1: RobotJoints6 { j1_deg: 50.0, j2_deg: 20.0, j3_deg: 0.0, j4_deg: 0.0, j5_deg: 0.0, j6_deg: 0.0 },
        joints_2: RobotJoints6 { j1_deg: 150.0, j2_deg: 40.0, j3_deg: 0.0, j4_deg: 0.0, j5_deg: 0.0, j6_deg: 0.0 },
        sync_flag: Some(8),
    };

    // Halfway interpolation at t=5.0
    let w_mid = interpolate_dual_robot_waypoint(&w1, &w2, 5.0);
    assert_eq!(w_mid.time_s, 5.0);
    assert!((w_mid.joints_1.j1_deg - 25.0).abs() < 1e-4);
    assert!((w_mid.joints_2.j1_deg - 125.0).abs() < 1e-4);

    // Dynamic clearance scaling
    // Large distance (600mm >= 500mm safe) -> 1.0 (no scaling)
    assert_eq!(calculate_clearance_velocity_scale(600.0, 100.0, 500.0, 0.2), 1.0);
    // Critical distance (50mm <= 100mm) -> min_scale 0.2
    assert_eq!(calculate_clearance_velocity_scale(50.0, 100.0, 500.0, 0.2), 0.2);
    // Intermediate distance (300mm halfway) -> 0.6
    let scale_mid = calculate_clearance_velocity_scale(300.0, 100.0, 500.0, 0.2);
    assert!((scale_mid - 0.6).abs() < 1e-4);
}
