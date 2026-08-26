//! Tool library & multi-tool registry schema (D2.3, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Provides typed definitions for physical tools (end mills, drills, lasers, nozzles, torches),
//! a named tool registry, and standard RS-274 / Fanuc tool change block generation (`T01 M06`, `G43 H01`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The physical tool category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    EndMill,
    BallNose,
    ChamferMill,
    Drill,
    Laser,
    ExtruderNozzle,
    PlasmaTorch,
    WaterjetNozzle,
}

/// A structured physical machine tool definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique identifier for the tool (e.g. "flat_endmill_6mm").
    pub id: String,
    /// Controller tool slot / index number (e.g. 1 for T01).
    pub number: u32,
    /// Human-readable label.
    pub name: String,
    /// Category of tool.
    pub kind: ToolKind,
    /// Tool cutting diameter in millimetres.
    pub diameter: f64,
    /// Flute cutting length in millimetres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flute_length: Option<f64>,
    /// Number of cutting flutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flute_count: Option<u32>,
    /// Maximum safe spindle RPM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rpm: Option<f64>,
    /// Tool length offset along Z axis (mm).
    #[serde(default)]
    pub offset_z: f64,
}

impl ToolDefinition {
    /// Create a new tool definition with default parameters.
    pub fn new(
        id: impl Into<String>,
        number: u32,
        name: impl Into<String>,
        kind: ToolKind,
        diameter: f64,
    ) -> Self {
        Self {
            id: id.into(),
            number,
            name: name.into(),
            kind,
            diameter,
            flute_length: None,
            flute_count: None,
            max_rpm: None,
            offset_z: 0.0,
        }
    }

    /// Validate physical tool invariants.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.diameter <= 0.0 || !self.diameter.is_finite() {
            return Err("tool diameter must be positive and finite");
        }
        if let Some(fl) = self.flute_length {
            if fl <= 0.0 || !fl.is_finite() {
                return Err("flute length must be positive and finite");
            }
        }
        if let Some(rpm) = self.max_rpm {
            if rpm <= 0.0 || !rpm.is_finite() {
                return Err("max RPM must be positive and finite");
            }
        }
        Ok(())
    }

    /// Emit standard RS-274 / Fanuc tool change G-code blocks (`T{n} M06` and `G43 H{n}`).
    pub fn emit_tool_change(&self) -> Vec<String> {
        vec![
            format!("T{:02} M06 ; Tool Change: {}", self.number, self.name),
            format!("G43 H{:02} ; Tool Length Offset", self.number),
        ]
    }
}

/// A registry / catalog of machine tools.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolDefinition>,
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool definition.
    pub fn register(&mut self, tool: ToolDefinition) -> Result<(), &'static str> {
        tool.validate()?;
        self.tools.insert(tool.id.clone(), tool);
        Ok(())
    }

    /// Look up a tool by its unique string ID.
    pub fn get(&self, id: &str) -> Option<&ToolDefinition> {
        self.tools.get(id)
    }

    /// Look up a tool by its controller slot number ($T_n$).
    pub fn get_by_number(&self, number: u32) -> Option<&ToolDefinition> {
        self.tools.values().find(|t| t.number == number)
    }

    /// Total number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}
