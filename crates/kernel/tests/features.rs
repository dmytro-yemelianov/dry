use kmet_kernel::{
    expand_features, expand_features_with_limits, resolve_checked, Design, ExpandLimits,
    FeatureNode, FeaturePose, FeatureProgram, Op, ResolveParams,
};

fn feature(ops: Vec<Op>, pose: FeaturePose) -> FeatureNode {
    FeatureNode::Feature {
        name: None,
        pose,
        ops,
    }
}

fn line_feature() -> FeatureNode {
    feature(
        vec![
            Op::Geometry {
                width: Some(0.6),
                height: Some(0.2),
            },
            Op::Extruder { on: true },
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(0.2),
            },
            Op::Move {
                x: Some(10.0),
                y: None,
                z: None,
            },
        ],
        FeaturePose::default(),
    )
}

#[test]
fn repeat_expands_to_the_hand_written_l1_equivalent() {
    let program = FeatureProgram {
        features: vec![FeatureNode::Repeat {
            count: 2,
            step: FeaturePose {
                x: 20.0,
                ..FeaturePose::default()
            },
            child: Box::new(line_feature()),
        }],
    };
    let expanded = expand_features(&program).unwrap();
    let hand = Design {
        ops: vec![
            Op::Geometry {
                width: Some(0.6),
                height: Some(0.2),
            },
            Op::Extruder { on: true },
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(0.2),
            },
            Op::Move {
                x: Some(10.0),
                y: Some(0.0),
                z: Some(0.2),
            },
            Op::Geometry {
                width: Some(0.6),
                height: Some(0.2),
            },
            Op::Extruder { on: true },
            Op::Move {
                x: Some(20.0),
                y: Some(0.0),
                z: Some(0.2),
            },
            Op::Move {
                x: Some(30.0),
                y: Some(0.0),
                z: Some(0.2),
            },
        ],
    };

    assert_eq!(
        serde_json::to_value(&expanded.ops).unwrap(),
        serde_json::to_value(&hand.ops).unwrap()
    );
    assert_eq!(
        resolve_checked(&expanded, &ResolveParams::default())
            .unwrap()
            .to_json(),
        resolve_checked(&hand, &ResolveParams::default())
            .unwrap()
            .to_json()
    );
}

#[test]
fn groups_preserve_source_order() {
    let program = FeatureProgram {
        features: vec![FeatureNode::Group {
            children: vec![
                feature(
                    vec![Op::Temperature { nozzle: 205.0 }],
                    FeaturePose::default(),
                ),
                feature(vec![Op::Fan { speed: 0.5 }], FeaturePose::default()),
            ],
        }],
    };
    let expanded = expand_features(&program).unwrap();
    assert!(matches!(
        expanded.ops.as_slice(),
        [Op::Temperature { nozzle: 205.0 }, Op::Fan { speed: 0.5 }]
    ));
}

#[test]
fn feature_pose_transforms_points_arcs_and_orientation() {
    let program = FeatureProgram {
        features: vec![feature(
            vec![
                Op::Move {
                    x: Some(1.0),
                    y: Some(2.0),
                    z: Some(3.0),
                },
                Op::Arc {
                    cx: 0.0,
                    cy: 2.0,
                    x: Some(0.0),
                    y: Some(1.0),
                    z: None,
                    clockwise: false,
                },
                Op::Orient {
                    i: 1.0,
                    j: 0.0,
                    k: 0.0,
                },
            ],
            FeaturePose {
                x: 10.0,
                y: 20.0,
                z: 4.0,
                rotate_z_deg: 90.0,
            },
        )],
    };
    let expanded = expand_features(&program).unwrap();

    let Op::Move { x, y, z } = &expanded.ops[0] else {
        panic!("expected transformed move");
    };
    assert!((x.unwrap() - 8.0).abs() < 1e-12);
    assert!((y.unwrap() - 21.0).abs() < 1e-12);
    assert!((z.unwrap() - 7.0).abs() < 1e-12);

    let Op::Arc {
        cx,
        cy,
        x,
        y,
        z,
        clockwise,
    } = &expanded.ops[1]
    else {
        panic!("expected transformed arc");
    };
    assert!((*cx - 8.0).abs() < 1e-12);
    assert!((*cy - 20.0).abs() < 1e-12);
    assert!((x.unwrap() - 9.0).abs() < 1e-12);
    assert!((y.unwrap() - 20.0).abs() < 1e-12);
    assert!((z.unwrap() - 7.0).abs() < 1e-12);
    assert!(!clockwise);

    let Op::Orient { i, j, k } = &expanded.ops[2] else {
        panic!("expected transformed orientation");
    };
    assert!(i.abs() < 1e-12);
    assert!((*j - 1.0).abs() < 1e-12);
    assert!(k.abs() < 1e-12);
}

#[test]
fn nested_repeat_and_feature_poses_compose_parent_first() {
    let program = FeatureProgram {
        features: vec![FeatureNode::Repeat {
            count: 2,
            step: FeaturePose {
                rotate_z_deg: 90.0,
                ..FeaturePose::default()
            },
            child: Box::new(feature(
                vec![Op::Move {
                    x: Some(0.0),
                    y: Some(0.0),
                    z: Some(0.0),
                }],
                FeaturePose {
                    x: 10.0,
                    ..FeaturePose::default()
                },
            )),
        }],
    };
    let expanded = expand_features(&program).unwrap();
    let endpoints: Vec<[f64; 3]> = expanded
        .ops
        .iter()
        .map(|op| match op {
            Op::Move { x, y, z } => [x.unwrap(), y.unwrap(), z.unwrap()],
            _ => panic!("expected move"),
        })
        .collect();
    assert!((endpoints[0][0] - 10.0).abs() < 1e-12);
    assert!(endpoints[0][1].abs() < 1e-12);
    assert!(endpoints[1][0].abs() < 1e-12);
    assert!((endpoints[1][1] - 10.0).abs() < 1e-12);
}

#[test]
fn feature_requires_a_self_contained_local_position() {
    let program = FeatureProgram {
        features: vec![feature(
            vec![Op::Move {
                x: Some(1.0),
                y: None,
                z: Some(0.2),
            }],
            FeaturePose::default(),
        )],
    };
    let error = expand_features(&program).unwrap_err();
    assert!(error.to_string().contains("y is undefined"));
}

#[test]
fn transformed_manual_gcode_is_rejected() {
    let program = FeatureProgram {
        features: vec![feature(
            vec![Op::ManualGcode {
                text: "G28".to_owned(),
            }],
            FeaturePose {
                x: 10.0,
                ..FeaturePose::default()
            },
        )],
    };
    let error = expand_features(&program).unwrap_err();
    assert!(error.to_string().contains("cannot be transformed safely"));
}

#[test]
fn expansion_limits_bound_recursive_growth() {
    let program = FeatureProgram {
        features: vec![FeatureNode::Repeat {
            count: 3,
            step: FeaturePose::default(),
            child: Box::new(line_feature()),
        }],
    };
    let error = expand_features_with_limits(
        &program,
        ExpandLimits {
            max_ops: 7,
            max_nodes: 100,
            max_depth: 10,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("max expanded ops (7)"));
}

#[test]
fn feature_program_wire_shape_round_trips() {
    let json = r#"{
      "features": [{
        "kind": "repeat",
        "count": 2,
        "step": {"x": 20},
        "child": {
          "kind": "feature",
          "name": "line",
          "ops": [
            {"op": "move", "x": 0, "y": 0, "z": 0.2},
            {"op": "move", "x": 10, "y": null, "z": null}
          ]
        }
      }]
    }"#;
    let program: FeatureProgram = serde_json::from_str(json).unwrap();
    let expanded = expand_features(&program).unwrap();
    assert_eq!(expanded.ops.len(), 4);
    assert_eq!(
        serde_json::to_value(&program).unwrap()["features"][0]["kind"],
        "repeat"
    );
}

#[test]
fn feature_program_rejects_unknown_fields() {
    let json = r#"{
      "features": [{
        "kind": "feature",
        "pose": {"rotation": 90},
        "ops": [{"op": "move", "x": 0, "y": 0, "z": 0.2}]
      }]
    }"#;
    let error = serde_json::from_str::<FeatureProgram>(json).unwrap_err();
    assert!(error.to_string().contains("unknown field `rotation`"));
}
