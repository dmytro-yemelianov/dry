use dry_core::{Robot6AxisModel, RobotJoints6};

#[test]
fn test_kuka_kr6_fk_and_ik() {
    let robot = Robot6AxisModel::kuka_kr6_r900();

    let initial_joints = RobotJoints6::new(0.0, -90.0, 90.0, 0.0, 0.0, 0.0);
    let (tcp_pos, tool_orient) = robot.solve_fk_pose(&initial_joints);

    // Solve inverse kinematics for the computed TCP position & orientation
    let solved_joints = robot
        .solve_ik(tcp_pos, tool_orient, &initial_joints)
        .expect("IK should solve reachable pose");

    // Recompute FK from solved joints and check position match
    let resolved_tcp = robot.solve_fk(&solved_joints);
    let error = ((tcp_pos[0] - resolved_tcp[0]).powi(2)
        + (tcp_pos[1] - resolved_tcp[1]).powi(2)
        + (tcp_pos[2] - resolved_tcp[2]).powi(2))
    .sqrt();

    assert!(error < 1e-3, "FK(IK(P)) error {error} mm must be < 1 µm");
}

#[test]
fn test_wrist_singularity_hold() {
    let robot = Robot6AxisModel::kuka_kr6_r900();
    let prev_joints = RobotJoints6::new(15.0, -80.0, 75.0, 45.0, 0.0, 10.0);

    // Compute exact FK pose where J5 is 0.0 (wrist singularity)
    let (tcp_pos, tool_orient) = robot.solve_fk_pose(&prev_joints);
    let result = robot.solve_ik(tcp_pos, tool_orient, &prev_joints);

    assert!(
        result.is_ok(),
        "Wrist singularity must be handled gracefully"
    );
    let joints = result.unwrap();
    assert!(
        (joints.j4_deg - 45.0).abs() < 1e-5,
        "J4 must hold previous angle (45.0 deg) during wrist singularity, got {}",
        joints.j4_deg
    );
    assert!((joints.j5_deg - 0.0).abs() < 1e-5, "J5 must remain 0 deg");
}
