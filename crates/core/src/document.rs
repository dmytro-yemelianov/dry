//! Versioned Dry IR document envelopes (D1.3, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Every Dry document serialized over the wire or stored on disk carries a standard schema envelope
//! containing `$schema`, `dialect`, `metadata`, `root_frame`, and `elements`.
//!
//! Standard dialects:
//! - `dry.intent/1`: Declarative manufacturing features, pockets, drilling, deposition strategies.
//! - `dry.path/1`: Non-modal path operations with explicit document state and named coordinate frames.
//! - `dry.motion/1`: Absolute machine coordinates, toolpath IR (L2).

use crate::frame::FrameId;
use serde::{Deserialize, Serialize};

/// Standard dialect identifiers supported by the Dry engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dialect {
    #[serde(rename = "dry.intent/1")]
    IntentV1,
    #[serde(rename = "dry.path/1")]
    PathV1,
    #[serde(rename = "dry.motion/1")]
    MotionV1,
    #[serde(untagged)]
    Custom(String),
}

impl Dialect {
    /// Return the canonical wire string for this dialect.
    pub fn as_str(&self) -> &str {
        match self {
            Dialect::IntentV1 => "dry.intent/1",
            Dialect::PathV1 => "dry.path/1",
            Dialect::MotionV1 => "dry.motion/1",
            Dialect::Custom(s) => s.as_str(),
        }
    }
}

/// Standard metadata header for a Dry document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
}

/// A versioned Dry document envelope wrapping dialect-specific elements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentEnvelope<T> {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub dialect: Dialect,
    #[serde(default)]
    pub metadata: DocumentMetadata,
    #[serde(default = "default_root_frame")]
    pub root_frame: FrameId,
    pub elements: Vec<T>,
}

fn default_root_frame() -> FrameId {
    FrameId::Design
}

/// Error encountered during document validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentValidationError {
    pub message: String,
}

impl DocumentValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DocumentValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for DocumentValidationError {}

impl<T> DocumentEnvelope<T> {
    /// Create a new document envelope with default metadata and root frame.
    pub fn new(dialect: Dialect, elements: Vec<T>) -> Self {
        Self {
            schema: None,
            dialect,
            metadata: DocumentMetadata::default(),
            root_frame: default_root_frame(),
            elements,
        }
    }

    /// Validate the document envelope structure.
    pub fn validate(&self) -> Result<(), DocumentValidationError> {
        if self.elements.is_empty() {
            return Err(DocumentValidationError::new(
                "document must contain at least one element",
            ));
        }
        Ok(())
    }
}
