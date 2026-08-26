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
