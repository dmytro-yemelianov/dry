use dry_core::{expand_features, FeatureNode, FeaturePose, FeatureProgram, Op, Quaternion};
use std::f64::consts::PI;

#[test]
fn test_feature_3d_rotation_transforms_linear_and_orientation() {
    // A horizontal move from (0,0,0) to (10,0,0) with orientation (0,0,1) (+Z normal)
    let local_ops = vec![
        Op::Move {
            x: Some(0.0),
            y: Some(0.0),
            z: Some(0.0),
        },
        Op::Orient {
            i: 0.0,
            j: 0.0,
            k: 1.0,
        },
        Op::Move {
            x: Some(10.0),
            y: Some(0.0),
            z: Some(0.0),
        },
    ];

    // 90 deg rotation around Y axis (maps +X to -Z, and +Z to +X)
    let q_y90 = Quaternion::from_axis_angle(0.0, 1.0, 0.0, PI / 2.0);
    let pose = FeaturePose {
        x: 50.0,
        y: 20.0,
        z: 100.0,
        rotate_z_deg: 0.0,
        rotation: Some(q_y90),
        frame: None,
    };

    let program = FeatureProgram {
        features: vec![FeatureNode::Feature {
            name: Some("angled_flange".into()),
            pose,
            ops: local_ops,
        }],
    };

    let design = expand_features(&program).expect("feature expansion must succeed");
    assert_eq!(design.ops.len(), 3);

    // First move: (0,0,0) placed at (50, 20, 100)
    match &design.ops[0] {
        Op::Move { x, y, z } => {
            assert!((x.unwrap() - 50.0).abs() < 1e-6);
            assert!((y.unwrap() - 20.0).abs() < 1e-6);
            assert!((z.unwrap() - 100.0).abs() < 1e-6);
        }
        _ => panic!("expected Move op"),
    }

    // Orient op: (0, 0, 1) rotated around Y by 90° -> (1, 0, 0) (+X normal)
    match &design.ops[1] {
        Op::Orient { i, j, k } => {
            assert!((i - 1.0).abs() < 1e-6);
            assert!(j.abs() < 1e-6);
            assert!(k.abs() < 1e-6);
        }
        _ => panic!("expected Orient op"),
    }

    // Second move: (10,0,0) rotated around Y by 90° is (0,0,-10), placed at (50, 20, 90)
    match &design.ops[2] {
        Op::Move { x, y, z } => {
            assert!((x.unwrap() - 50.0).abs() < 1e-6);
            assert!((y.unwrap() - 20.0).abs() < 1e-6);
            assert!((z.unwrap() - 90.0).abs() < 1e-6);
        }
        _ => panic!("expected Move op"),
    }
}

/// D1.2 introduced `FeaturePose.rotation` without routing it through the pose finiteness gate that
/// `rotate_z_deg` clears. The failure mode was silence rather than NaN: composition normalises via
/// `Quaternion::new`, which substitutes the identity whenever the norm is zero or non-finite, so a
/// garbage rotation placed the feature unrotated with nothing to indicate it had been discarded.
#[test]
fn test_non_finite_rotation_is_rejected_not_silently_dropped() {
    let non_finite = [
        Quaternion {
            x: f64::NAN,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        Quaternion {
            x: 0.0,
            y: f64::INFINITY,
            z: 0.0,
            w: 1.0,
        },
        Quaternion {
            x: 0.0,
            y: 0.0,
            z: f64::NEG_INFINITY,
            w: 1.0,
        },
        Quaternion {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: f64::NAN,
        },
    ];

    for q in non_finite {
        let program = FeatureProgram {
            features: vec![FeatureNode::Feature {
                name: Some("bad_rotation".into()),
                pose: FeaturePose {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    rotate_z_deg: 0.0,
                    rotation: Some(q),
                    frame: None,
                },
                ops: vec![Op::Move {
                    x: Some(10.0),
                    y: Some(0.0),
                    z: Some(0.0),
                }],
            }],
        };
        let error = expand_features(&program).expect_err("non-finite rotation must be rejected");
        assert!(
            format!("{error:?}").contains("must be finite"),
            "expected a finiteness error for {q:?}, got {error:?}"
        );
    }
}

/// A quaternion whose norm is zero — exactly, or by underflow — is not a rotation. `Quaternion::new`
/// maps it to the identity, so accepting it would be the same silent placement as above.
#[test]
fn test_degenerate_norm_rotation_is_rejected() {
    let degenerate = [
        Quaternion {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        },
        Quaternion {
            x: 1e-200,
            y: 1e-200,
            z: 1e-200,
            w: 1e-200,
        },
        Quaternion {
            x: 1e200,
            y: 1e200,
            z: 1e200,
            w: 1e200,
        },
    ];

    for q in degenerate {
        let program = FeatureProgram {
            features: vec![FeatureNode::Feature {
                name: Some("degenerate".into()),
                pose: FeaturePose {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    rotate_z_deg: 0.0,
                    rotation: Some(q),
                    frame: None,
                },
                ops: vec![Op::Move {
                    x: Some(10.0),
                    y: Some(0.0),
                    z: Some(0.0),
                }],
            }],
        };
        let error = expand_features(&program).expect_err("degenerate rotation must be rejected");
        assert!(
            format!("{error:?}").contains("unit quaternion"),
            "expected a unit-quaternion error for {q:?}, got {error:?}"
        );
    }
}

/// The valid path is unchanged: a unit quaternion still rotates, and a pose without one still uses
/// the planar cos/sin route.
#[test]
fn test_valid_rotations_still_accepted() {
    for rotation in [
        None,
        Some(Quaternion::from_axis_angle(0.0, 0.0, 1.0, PI / 2.0)),
    ] {
        let program = FeatureProgram {
            features: vec![FeatureNode::Feature {
                name: Some("ok".into()),
                pose: FeaturePose {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    rotate_z_deg: 0.0,
                    rotation,
                    frame: None,
                },
                ops: vec![Op::Move {
                    x: Some(10.0),
                    y: Some(0.0),
                    z: Some(0.0),
                }],
            }],
        };
        expand_features(&program).expect("a valid pose must still expand");
    }
}
