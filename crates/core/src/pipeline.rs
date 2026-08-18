//! End-to-end dialect lowering pipeline with provenance mapping (D1.5, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Lowers high-level feature documents (`DocumentEnvelope<FeatureNode>`) into machine-executable
//! motion documents (`DocumentEnvelope<Segment>`) while automatically compiling an exact
//! input-node $\to$ output-segment [`ProvenanceMap`].

use crate::document::{Dialect, DocumentEnvelope};
use crate::features::{expand_features, FeatureNode, FeatureProgram};
use crate::ir::Segment;
use crate::provenance::{NodeId, ProvenanceMap, SegmentSpan};
use crate::resolve::{resolve, ResolveParams};
use serde::{Deserialize, Serialize};

/// Error encountered during document lowering pipeline execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineError {
    pub message: String,
}

impl PipelineError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PipelineError {}

/// Lower a feature document envelope into a motion document envelope with provenance mapping.
pub fn lower_document_envelope(
    envelope: &DocumentEnvelope<FeatureNode>,
    params: &ResolveParams,
) -> Result<(DocumentEnvelope<Segment>, ProvenanceMap), PipelineError> {
    let mut provenance = ProvenanceMap::new();
    let mut all_segments = Vec::new();

    for (index, feature_node) in envelope.elements.iter().enumerate() {
        let node_id = match feature_node {
            FeatureNode::Feature { name: Some(n), .. } => NodeId::new(n.clone()),
            _ => NodeId::new(format!("feature_{index}")),
        };

        let start_segment = all_segments.len();

        let program = FeatureProgram {
            features: vec![feature_node.clone()],
        };

        let design = expand_features(&program)
            .map_err(|e| PipelineError::new(format!("failed to expand feature {index}: {e}")))?;

        let toolpath = resolve(&design, params);
        all_segments.extend(toolpath.segments);

        let end_segment = all_segments.len();
        provenance.insert(node_id, SegmentSpan::new(start_segment, end_segment));
    }

    let motion_envelope = DocumentEnvelope {
        schema: envelope.schema.clone(),
        dialect: Dialect::MotionV1,
        metadata: envelope.metadata.clone(),
        root_frame: envelope.root_frame,
        elements: all_segments,
    };

    Ok((motion_envelope, provenance))
}
