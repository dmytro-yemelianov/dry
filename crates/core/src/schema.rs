//! JSON Schema definitions for Dry IR versioned dialects (D1.6, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).

/// Retrieve the JSON Schema for a requested dialect identifier.
pub fn get_dialect_schema(dialect: &str) -> Option<&'static str> {
    match dialect {
        "dry.intent/1" | "intent/1" | "intent" => Some(INTENT_V1_SCHEMA),
        "dry.path/1" | "path/1" | "path" => Some(PATH_V1_SCHEMA),
        "dry.motion/1" | "motion/1" | "motion" => Some(MOTION_V1_SCHEMA),
        "dry.tool/1" | "tool/1" | "tool" => Some(TOOL_V1_SCHEMA),
        _ => None,
    }
}

pub const INTENT_V1_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "DryIntentV1",
  "description": "Dry IR L0 Intent Dialect (v1)",
  "type": "object",
  "required": ["dialect", "elements"],
  "properties": {
    "$schema": { "type": "string" },
    "dialect": { "type": "string", "enum": ["dry.intent/1"] },
    "metadata": { "type": "object" },
    "root_frame": { "type": "string" },
    "elements": { "type": "array" }
  }
}"#;

pub const PATH_V1_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "DryPathV1",
  "description": "Dry IR L1 Path Dialect (v1)",
  "type": "object",
  "required": ["dialect", "elements"],
  "properties": {
    "$schema": { "type": "string" },
    "dialect": { "type": "string", "enum": ["dry.path/1"] },
    "metadata": { "type": "object" },
    "root_frame": { "type": "string" },
    "elements": { "type": "array" }
  }
}"#;

pub const MOTION_V1_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "DryMotionV1",
  "description": "Dry IR L2 Motion Dialect (v1)",
  "type": "object",
  "required": ["dialect", "elements"],
  "properties": {
    "$schema": { "type": "string" },
    "dialect": { "type": "string", "enum": ["dry.motion/1"] },
    "metadata": { "type": "object" },
    "root_frame": { "type": "string" },
    "elements": { "type": "array" }
  }
}"#;

pub const TOOL_V1_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "DryToolV1",
  "description": "Dry Tool Registry Schema (v1)",
  "type": "object",
  "required": ["id", "number", "name", "kind", "diameter"],
  "properties": {
    "id": { "type": "string" },
    "number": { "type": "integer", "minimum": 1 },
    "name": { "type": "string" },
    "kind": { "type": "string" },
    "diameter": { "type": "number", "exclusiveMinimum": 0 },
    "flute_length": { "type": "number" },
    "flute_count": { "type": "integer" },
    "max_rpm": { "type": "number" },
    "offset_z": { "type": "number" }
  }
}"#;
