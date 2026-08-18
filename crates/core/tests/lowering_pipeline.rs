use dry_core::{
    lower_document_envelope, Dialect, DocumentEnvelope, FeatureNode, FeaturePose, NodeId, Op,
    ResolveParams,
};

#[test]
fn test_lower_document_envelope_generates_motion_and_provenance() {
    let feature1 = FeatureNode::Feature {
        name: Some("square_base".into()),
        pose: FeaturePose {
            x: 0.0,
            y: 0.0,
            z: 0.2,
            ..Default::default()
        },
        ops: vec![
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
        ],
    };

    let feature2 = FeatureNode::Feature {
        name: Some("raised_flange".into()),
        pose: FeaturePose {
            x: 50.0,
            y: 50.0,
            z: 1.0,
            ..Default::default()
        },
        ops: vec![
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(1.0),
            },
            Op::Move {
                x: Some(20.0),
                y: Some(0.0),
                z: Some(1.0),
            },
        ],
    };

    let envelope = DocumentEnvelope::new(Dialect::PathV1, vec![feature1, feature2]);

    let (motion_doc, provenance) =
        lower_document_envelope(&envelope, &ResolveParams::default()).expect("lowering succeeds");

    assert_eq!(motion_doc.dialect, Dialect::MotionV1);
    assert!(!motion_doc.elements.is_empty());

    let span1 = provenance.get_span(&NodeId::new("square_base")).expect("must have span for base");
    let span2 = provenance.get_span(&NodeId::new("raised_flange")).expect("must have span for flange");

    assert_eq!(span1.start, 0);
    assert_eq!(span1.end, span2.start);
    assert!(span2.end > span2.start);

    // Finding node by segment index
    assert_eq!(provenance.find_node_for_segment(0), Some(&NodeId::new("square_base")));
    assert_eq!(provenance.find_node_for_segment(span2.start), Some(&NodeId::new("raised_flange")));
}
