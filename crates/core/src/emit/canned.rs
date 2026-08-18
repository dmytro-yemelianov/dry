//! CNC canned machining cycles for RS-274 / Fanuc dialects (D3.1, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Implements industry-standard canned cycles:
//! - `G81`: Direct drilling cycle (rapid to R plane, feed to Z depth, rapid retract to R plane).
//! - `G83`: Deep hole peck drilling cycle with incremental peck depth Q and retracts.

use serde::{Deserialize, Serialize};

/// Parameters for a standard G81 drilling cycle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrillCycle {
    pub x: f64,
    pub y: f64,
    pub z_depth: f64,
    pub r_plane: f64,
    pub feedrate_mm_min: f64,
}

impl DrillCycle {
    /// Emit RS-274 / Fanuc G81 block string.
    pub fn emit_rs274(&self) -> String {
        format!(
            "G81 X{:.3} Y{:.3} Z{:.3} R{:.3} F{:.1}",
            self.x, self.y, self.z_depth, self.r_plane, self.feedrate_mm_min
        )
    }
}

/// Parameters for a standard G83 deep-hole peck drilling cycle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PeckDrillCycle {
    pub x: f64,
    pub y: f64,
    pub z_depth: f64,
    pub r_plane: f64,
    pub peck_depth_q: f64,
    pub feedrate_mm_min: f64,
}

impl PeckDrillCycle {
    /// Emit RS-274 / Fanuc G83 block string.
    pub fn emit_rs274(&self) -> String {
        format!(
            "G83 X{:.3} Y{:.3} Z{:.3} R{:.3} Q{:.3} F{:.1}",
            self.x, self.y, self.z_depth, self.r_plane, self.peck_depth_q, self.feedrate_mm_min
        )
    }
}

/// Emit cycle cancel block (G80).
pub fn emit_cycle_cancel() -> &'static str {
    "G80"
}
