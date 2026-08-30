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

    assert!(krl_master
        .iter()
        .any(|line| line.contains("$FLAG[5] = TRUE")));
    assert!(krl_slave
        .iter()
        .any(|line| line.contains("WAIT FOR $FLAG[5]")));
}

#[test]
fn test_dual_robot_waypoint_interpolation_and_velocity_scaling() {
    use dry_core::{calculate_clearance_velocity_scale, interpolate_dual_robot_waypoint};

    let w1 = DualRobotWaypoint {
        time_s: 0.0,
        joints_1: RobotJoints6 {
            j1_deg: 0.0,
            j2_deg: 0.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        },
        joints_2: RobotJoints6 {
            j1_deg: 100.0,
            j2_deg: 0.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        },
        sync_flag: None,
    };

    let w2 = DualRobotWaypoint {
        time_s: 10.0,
        joints_1: RobotJoints6 {
            j1_deg: 50.0,
            j2_deg: 20.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        },
        joints_2: RobotJoints6 {
            j1_deg: 150.0,
            j2_deg: 40.0,
            j3_deg: 0.0,
            j4_deg: 0.0,
            j5_deg: 0.0,
            j6_deg: 0.0,
        },
        sync_flag: Some(8),
    };

    // Halfway interpolation at t=5.0
    let w_mid = interpolate_dual_robot_waypoint(&w1, &w2, 5.0);
    assert_eq!(w_mid.time_s, 5.0);
    assert!((w_mid.joints_1.j1_deg - 25.0).abs() < 1e-4);
    assert!((w_mid.joints_2.j1_deg - 125.0).abs() < 1e-4);

    // Dynamic clearance scaling
    // Large distance (600mm >= 500mm safe) -> 1.0 (no scaling)
    assert_eq!(
        calculate_clearance_velocity_scale(600.0, 100.0, 500.0, 0.2),
        1.0
    );
    // Critical distance (50mm <= 100mm) -> min_scale 0.2
    assert_eq!(
        calculate_clearance_velocity_scale(50.0, 100.0, 500.0, 0.2),
        0.2
    );
    // Intermediate distance (300mm halfway) -> 0.6
    let scale_mid = calculate_clearance_velocity_scale(300.0, 100.0, 500.0, 0.2);
    assert!((scale_mid - 0.6).abs() < 1e-4);
}

#[test]
fn test_abb_rapid_dual_robot_sync() {
    use dry_core::emit_dual_robot_sync_rapid;

    let rapid_lines = emit_dual_robot_sync_rapid("sync_point_1", &["T_ROB1", "T_ROB2"]);
    assert_eq!(rapid_lines.len(), 2);
    assert!(rapid_lines[1].contains("WaitSyncTask sync_point_1, [T_ROB1, T_ROB2];"));
}

/// A collision check must never answer "safe" about a pose it could not evaluate.
///
/// Every distance comparison against `NaN` is false, so a non-finite joint angle, base offset, link
/// radius or safety margin left `safe` at its initial `true` and reported `min_distance_mm = inf` —
/// abundant clearance, from arithmetic that never happened. Both entry points now fail closed.
#[test]
fn clearance_fails_closed_on_input_it_cannot_evaluate() {
    use dry_core::{check_dual_robot_clearance, Robot6AxisModel, RobotJoints6, WorkcellRobot};

    let model = Robot6AxisModel::kuka_kr6_r900();
    let robot = |id: &str, dx: f64| WorkcellRobot {
        id: id.into(),
        model: model.clone(),
        base_offset: [dx, 0.0, 0.0],
        link_radii: [60.0; 6],
    };
    let good = RobotJoints6::new(0.0, -90.0, 90.0, 0.0, 0.0, 0.0);

    // Baseline: two robots half a metre apart are genuinely clear, and say so.
    let r1 = robot("a", 0.0);
    let r2 = robot("b", 500.0);
    let clear = check_dual_robot_clearance(&r1, &good, &r2, &good, 50.0);
    assert!(clear.safe);
    assert!(clear.min_distance_mm.is_finite() && clear.min_distance_mm > 0.0);

    for bad_joints in [
        RobotJoints6::new(f64::NAN, -90.0, 90.0, 0.0, 0.0, 0.0),
        RobotJoints6::new(0.0, f64::INFINITY, 90.0, 0.0, 0.0, 0.0),
    ] {
        let v = check_dual_robot_clearance(&r1, &bad_joints, &r2, &good, 50.0);
        assert!(!v.safe, "a non-finite joint must not report safe");
        assert_eq!(
            v.min_distance_mm, 0.0,
            "and must not report abundant clearance"
        );
    }

    // A non-finite margin, base offset or link radius is the same hazard by another route.
    let v = check_dual_robot_clearance(&r1, &good, &r2, &good, f64::NAN);
    assert!(!v.safe, "a non-finite safety margin must not report safe");

    let skewed = WorkcellRobot {
        base_offset: [f64::NAN, 0.0, 0.0],
        ..robot("c", 500.0)
    };
    assert!(!check_dual_robot_clearance(&r1, &good, &skewed, &good, 50.0).safe);

    let fat = WorkcellRobot {
        link_radii: [f64::INFINITY; 6],
        ..robot("d", 500.0)
    };
    assert!(!check_dual_robot_clearance(&r1, &good, &fat, &good, 50.0).safe);
}
