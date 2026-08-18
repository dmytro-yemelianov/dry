//! Provenance mapping and source node tracking (D1.4, `docs/20-dry-ir-ecosystem-implementation-plan.md` §6.5).
//!
//! Tracks lowering and optimization transformations from high-level authoring Node IDs down to
//! exact ranges of lowered L2 segments, enabling fine-grained diagnostics and debugging.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A stable identifier for an L0/L1 authoring node.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// A contiguous span of lowered L2 segment indices `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentSpan {
    pub start: usize,
    pub end: usize,
}

impl SegmentSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Check if this span contains the given segment index.
    pub fn contains(&self, index: usize) -> bool {
        index >= self.start && index < self.end
    }

    /// Number of segments covered by this span.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Bidirectional map between authoring `NodeId` and lowered `SegmentSpan`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceMap {
    node_to_span: HashMap<NodeId, SegmentSpan>,
}

impl ProvenanceMap {
    pub fn new() -> Self {
        Self {
            node_to_span: HashMap::new(),
        }
    }

    /// Record the lowered segment span for a given node.
    pub fn insert(&mut self, node_id: NodeId, span: SegmentSpan) {
        self.node_to_span.insert(node_id, span);
    }

    /// Lookup the segment span for an authoring node.
    pub fn get_span(&self, node_id: &NodeId) -> Option<SegmentSpan> {
        self.node_to_span.get(node_id).copied()
    }

    /// Lookup which authoring node generated a specific segment index.
    pub fn find_node_for_segment(&self, segment_index: usize) -> Option<&NodeId> {
        self.node_to_span
            .iter()
            .find(|(_, span)| span.contains(segment_index))
            .map(|(node_id, _)| node_id)
    }
}
