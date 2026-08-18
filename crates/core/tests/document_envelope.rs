use dry_core::{Dialect, DocumentEnvelope, DocumentMetadata, FrameId};

#[test]
fn test_document_envelope_serialization_and_validation() {
    let mut envelope = DocumentEnvelope::new(
        Dialect::PathV1,
        vec![serde_json::json!({
            "op": "move",
            "x": 10.0,
            "y": 20.0,
            "z": 0.2
        })],
    );
    envelope.metadata = DocumentMetadata {
        title: Some("Sample Part".into()),
        author: Some("Dry Engine".into()),
        generator: Some("dry-core 0.7.0".into()),
        units: Some("mm".into()),
    };
    envelope.root_frame = FrameId::Workpiece;

    assert!(envelope.validate().is_ok());

    let json = serde_json::to_string(&envelope).expect("must serialize envelope");
    assert!(json.contains("dry.path/1"));
    assert!(json.contains("Sample Part"));
    assert!(json.contains("workpiece"));

    let deserialized: DocumentEnvelope<serde_json::Value> =
        serde_json::from_str(&json).expect("must deserialize envelope");
    assert_eq!(deserialized.dialect, Dialect::PathV1);
    assert_eq!(deserialized.root_frame, FrameId::Workpiece);
    assert_eq!(deserialized.elements.len(), 1);
}

#[test]
fn test_document_envelope_empty_validation_fails() {
    let empty_envelope: DocumentEnvelope<serde_json::Value> =
        DocumentEnvelope::new(Dialect::IntentV1, vec![]);
    assert!(empty_envelope.validate().is_err());
}
