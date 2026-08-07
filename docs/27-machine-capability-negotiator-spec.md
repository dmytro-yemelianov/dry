# Machine Capability Negotiator Specification

**Version:** 1.0.0  
**Status:** Normative Standard  
**Document ID:** `DRY-SPEC-2026-V27`

---

## 1. Objective

The Machine Capability Negotiator is the failure-closed gate in `dry-core` that validates an $L_2$ toolpath (`Toolpath` struct) against a target machine profile (`spec/machine-capability.schema.json`) before $L_3$ target code emission.

If a toolpath contains moves or channel states that exceed physical limits or are unsupported by the target machine firmware, compilation fails closed at the IR gate with structured, located diagnostics.

---

## 2. Capability Validation Rules

### 2.1 Spatial Envelope (Build Volume)
- Rule: Every segment endpoint $[X, Y, Z]$ must lie strictly inside $[X_{\min..\max}, Y_{\min..\max}, Z_{\min..\max}]$.
- Failure Diagnostic: `OUT_OF_BOUNDS_MOVE` with segment index and coordinates.

### 2.2 Volumetric Flow Limit ($Q_{\max}$)
- Rule: For extruding segments, flow rate $Q = \frac{\text{volume}}{\text{length} / \text{speed}}$ must not exceed $Q_{\max}$.
- Failure Diagnostic: `EXCESSIVE_FLOW_RATE` with requested $Q$ vs $Q_{\max}$.

### 2.3 Spindle/Laser Power Channel Support
- Rule: If any segment has `power: Some(val)` where `val > 0`, the target machine profile must set `supports_power_channel: true` and specify `max_spindle_rpm`.
- Failure Diagnostic: `UNSUPPORTED_POWER_CHANNEL` if the target machine lacks laser/spindle capability.

### 2.4 Toolframe Orientation Vectors (5-Axis)
- Rule: If any segment specifies `orientation: Some([i, j, k])` where $[i, j, k] \neq [0, 0, 1]$, the target profile must specify `axes >= 5` and valid `rotary_axes` (`ab`, `ac`, or `bc`).
- Failure Diagnostic: `UNSUPPORTED_5_AXIS_ORIENTATION` if the target is a 3-axis machine.

---

## 3. Data Structures (`crates/core/src/profile/capability.rs`)

```rust
use crate::ir::Toolpath;
use crate::units::{Feedrate, Length, Volume};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineCapability {
    pub version: u32,
    pub process_family: String,
    pub axes: u8,
    pub rotary_axes: Option<String>,
    pub build_volume: Option<[[f64; 2]; 3]>,
    pub max_volumetric_flow_mm3_s: Option<f64>,
    pub max_feedrate_mm_min: Option<f64>,
    pub max_spindle_rpm: Option<f64>,
    pub supports_power_channel: bool,
    pub supports_toolframe_orientation: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityViolation {
    pub rule_id: String,
    pub segment_index: usize,
    pub message: String,
}

impl MachineCapability {
    pub fn validate(&self, toolpath: &Toolpath) -> Result<(), Vec<CapabilityViolation>> {
        let mut violations = Vec::new();
        for (idx, seg) in toolpath.segments.iter().enumerate() {
            // 1. 5-Axis orientation check
            if let Some(orient) = seg.orientation {
                if (orient[0].abs() > 1e-6 || orient[1].abs() > 1e-6 || (orient[2] - 1.0).abs() > 1e-6)
                    && !self.supports_toolframe_orientation {
                    violations.push(CapabilityViolation {
                        rule_id: "UNSUPPORTED_5_AXIS_ORIENTATION".into(),
                        segment_index: idx,
                        message: format!("Segment {} requires 5-axis orientation {:?}, but machine is 3-axis", idx, orient),
                    });
                }
            }
            // 2. Power channel check
            if let Some(pwr) = seg.power {
                if pwr > 0.0 && !self.supports_power_channel {
                    violations.push(CapabilityViolation {
                        rule_id: "UNSUPPORTED_POWER_CHANNEL".into(),
                        segment_index: idx,
                        message: format!("Segment {} requests power {}, but machine lacks power channel support", idx, pwr),
                    });
                }
            }
        }
        if violations.is_empty() { Ok(()) } else { Err(violations) }
    }
}
```
