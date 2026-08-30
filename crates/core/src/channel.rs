//! Typed Process Channel Registry & Compatibility Policy (D1.5 / Track D).
//!
//! Provides extensible, schema-validated process channels for multi-process CAM operations
//! (FFF, Laser, Plasma, CNC Milling, Lathe, DED AM).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Classification of channel continuity and interpolation semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelKind {
    /// Value is continuous and can be linearly interpolated between waypoints (e.g. flow, speed, power).
    Continuous,
    /// Value is discrete and changes step-wise at specific waypoints (e.g. tool ID, valve open/close).
    Discrete,
    /// Value is modal and remains active until explicitly overridden (e.g. target temperature, spindle RPM).
    Modal,
}

/// The typed scalar or discrete value held by a process channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChannelValue {
    Scalar(f64),
    Integer(i64),
    Flag(bool),
    Text(String),
}

impl ChannelValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Scalar(v) => Some(*v),
            Self::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Flag(b) => Some(*b),
            _ => None,
        }
    }
}

/// Metadata and validation contract for a registered process channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelDefinition {
    /// Unique identifier for this channel (e.g. `temperature`, `spindle_rpm`, `laser_power`).
    pub id: String,
    /// Physical unit string (e.g. `degC`, `rpm`, `watts`, `mm/s`).
    pub unit: Option<String>,
    /// Continuity and interpolation kind.
    pub kind: ChannelKind,
    /// Default baseline value if omitted.
    pub default_value: Option<ChannelValue>,
    /// Human-readable description of this channel.
    pub description: Option<String>,
}

/// Extensible registry of active process channels.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ChannelRegistry {
    channels: BTreeMap<String, ChannelDefinition>,
}

impl ChannelRegistry {
    /// Create a new registry populated with standard built-in process channels.
    pub fn standard() -> Self {
        let mut reg = Self::default();

        reg.register(ChannelDefinition {
            id: "temperature".into(),
            unit: Some("degC".into()),
            kind: ChannelKind::Modal,
            default_value: Some(ChannelValue::Scalar(200.0)),
            description: Some("Extruder nozzle target temperature in Celsius".into()),
        });

        reg.register(ChannelDefinition {
            id: "bed_temperature".into(),
            unit: Some("degC".into()),
            kind: ChannelKind::Modal,
            default_value: Some(ChannelValue::Scalar(60.0)),
            description: Some("Heated bed target temperature in Celsius".into()),
        });

        reg.register(ChannelDefinition {
            id: "fan_speed".into(),
            unit: Some("fraction".into()),
            kind: ChannelKind::Continuous,
            default_value: Some(ChannelValue::Scalar(0.0)),
            description: Some("Part cooling fan PWM duty cycle (0.0 to 1.0)".into()),
        });

        reg.register(ChannelDefinition {
            id: "flow_multiplier".into(),
            unit: Some("ratio".into()),
            kind: ChannelKind::Continuous,
            default_value: Some(ChannelValue::Scalar(1.0)),
            description: Some("Volumetric extrusion flow multiplier".into()),
        });

        reg.register(ChannelDefinition {
            id: "spindle_rpm".into(),
            unit: Some("rpm".into()),
            kind: ChannelKind::Modal,
            default_value: Some(ChannelValue::Scalar(0.0)),
            description: Some("Spindle rotational speed in RPM".into()),
        });

        reg.register(ChannelDefinition {
            id: "laser_power".into(),
            unit: Some("watts".into()),
            kind: ChannelKind::Continuous,
            default_value: Some(ChannelValue::Scalar(0.0)),
            description: Some("Laser optical power output in Watts or PWM fraction".into()),
        });

        reg.register(ChannelDefinition {
            id: "active_tool".into(),
            unit: None,
            kind: ChannelKind::Discrete,
            default_value: Some(ChannelValue::Integer(0)),
            description: Some("Active tool / head index".into()),
        });

        reg
    }

    /// Register a new channel definition.
    pub fn register(&mut self, def: ChannelDefinition) {
        self.channels.insert(def.id.clone(), def);
    }

    /// Retrieve a channel definition by ID.
    pub fn get(&self, id: &str) -> Option<&ChannelDefinition> {
        self.channels.get(id)
    }

    /// Returns true if the channel ID is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.channels.contains_key(id)
    }

    /// Validate a channel map against the registered channel definitions.
    pub fn validate_map(&self, map: &BTreeMap<String, ChannelValue>) -> Result<(), String> {
        for (id, val) in map {
            if let Some(def) = self.get(id) {
                if let (ChannelKind::Continuous, ChannelValue::Text(_)) = (&def.kind, val) {
                    return Err(format!("Continuous channel '{id}' cannot hold text values"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_channel_registry() {
        let reg = ChannelRegistry::standard();
        assert!(reg.contains("temperature"));
        assert!(reg.contains("bed_temperature"));
        assert!(reg.contains("fan_speed"));
        assert!(reg.contains("flow_multiplier"));
        assert!(reg.contains("spindle_rpm"));
        assert!(reg.contains("laser_power"));
        assert!(reg.contains("active_tool"));

        let temp = reg.get("temperature").unwrap();
        assert_eq!(temp.kind, ChannelKind::Modal);
        assert_eq!(temp.unit.as_deref(), Some("degC"));
        assert_eq!(temp.default_value.as_ref().unwrap().as_f64(), Some(200.0));
    }

    #[test]
    fn test_custom_channel_registration_and_validation() {
        let mut reg = ChannelRegistry::standard();
        reg.register(ChannelDefinition {
            id: "plasma_arc_voltage".into(),
            unit: Some("volts".into()),
            kind: ChannelKind::Continuous,
            default_value: Some(ChannelValue::Scalar(125.0)),
            description: Some("Torch height control reference voltage".into()),
        });

        assert!(reg.contains("plasma_arc_voltage"));

        let mut valid_map = BTreeMap::new();
        valid_map.insert("plasma_arc_voltage".into(), ChannelValue::Scalar(130.0));
        assert!(reg.validate_map(&valid_map).is_ok());

        let mut invalid_map = BTreeMap::new();
        invalid_map.insert(
            "plasma_arc_voltage".into(),
            ChannelValue::Text("high".into()),
        );
        assert!(reg.validate_map(&invalid_map).is_err());
    }
}
