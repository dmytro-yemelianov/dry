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

/// `solve_ik` refuses what carries no pose, instead of returning `Ok` with `NaN` in every joint.
///
/// The reach check is `cos_phi.abs() > 1.0`, and `NaN.abs() > 1.0` is false — so before these
/// guards a non-finite input sailed past it and every joint came back `NaN`, the class H1.1/H1.2
/// closed on every other ingress.
#[test]
fn solve_ik_refuses_input_that_carries_no_pose() {
    let robot = Robot6AxisModel::kuka_kr6_r900();
    let prev = RobotJoints6::new(0.0, -90.0, 90.0, 0.0, 0.0, 0.0);
    let up = [0.0, 0.0, 1.0];

    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = robot
            .solve_ik([bad, 0.0, 400.0], up, &prev)
            .expect_err("a non-finite TCP point must be refused");
        assert!(err.contains("must be finite"), "{err}");
    }

    // A zero direction determines no wrist centre; the 5-axis path already refuses it.
    let err = robot
        .solve_ik([400.0, 0.0, 400.0], [0.0, 0.0, 0.0], &prev)
        .expect_err("a zero tool direction must be refused");
    assert!(err.contains("finite non-zero magnitude"), "{err}");

    let err = robot
        .solve_ik([400.0, 0.0, 400.0], [f64::NAN, 0.0, 1.0], &prev)
        .expect_err("a non-finite tool direction must be refused");
    assert!(err.contains("finite non-zero magnitude"), "{err}");

    let bad_prev = RobotJoints6::new(0.0, -90.0, 90.0, f64::NAN, 0.0, 0.0);
    let err = robot
        .solve_ik([400.0, 0.0, 400.0], up, &bad_prev)
        .expect_err("a non-finite previous joint state must be refused");
    assert!(err.contains("previous joint state"), "{err}");
}

/// The tool direction scales the `d6` wrist-centre offset, so a non-unit vector used raw displaces
/// the wrist centre. Normalising makes a direction's *length* irrelevant, which is what the 5-axis
/// path already guarantees through `unit_orientation`.
#[test]
fn solve_ik_is_invariant_to_tool_direction_length() {
    let robot = Robot6AxisModel::kuka_kr6_r900();
    let prev = RobotJoints6::new(0.0, -90.0, 90.0, 0.0, 0.0, 0.0);
    let tcp = [400.0, 0.0, 400.0];

    let unit = robot.solve_ik(tcp, [0.0, 0.0, 1.0], &prev).unwrap();
    for scale in [0.25, 7.0, 1000.0] {
        let scaled = robot.solve_ik(tcp, [0.0, 0.0, scale], &prev).unwrap();
        assert_eq!(
            unit.to_radians(),
            scaled.to_radians(),
            "a direction scaled by {scale} must give the same joints"
        );
    }
}

/// `solve_ik` is a 5-DOF solve in a six-joint shape: J6 is never determined. Pinned so the shape
/// cannot start looking solved without the doc comment and the numeric-boundary entry moving too.
#[test]
fn solve_ik_does_not_determine_j6() {
    let robot = Robot6AxisModel::kuka_kr6_r900();
    let prev = RobotJoints6::new(0.0, -90.0, 90.0, 0.0, 0.0, 30.0);
    let joints = robot
        .solve_ik([400.0, 0.0, 400.0], [0.0, 0.0, 1.0], &prev)
        .unwrap();
    assert_eq!(
        joints.j6_deg, 0.0,
        "J6 is not solved; it must read 0 rather than appear to carry the previous roll"
    );
}
