//! Process pass classification & role tagging (D2.4, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Categorizes toolpath moves by their functional machining or manufacturing role:
//! - Roughing vs Finishing passes in CNC milling.
//! - Perimeters vs Infill vs Support in additive manufacturing (FFF).
//! - Lead-in vs Lead-out in plasma/waterjet cutting.

use serde::{Deserialize, Serialize};

/// The functional machining / printing role of a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassRole {
    /// High material removal roughing pass.
    Roughing,
    /// High precision finishing pass.
    Finishing,
    /// Outer boundary contour (additive or subtractive).
    Perimeter,
    /// Internal volume infill pattern.
    Infill,
    /// Sacrificial support structure.
    Support,
    /// Tangential / linear approach before cutting.
    LeadIn,
    /// Tangential / linear departure after cutting.
    LeadOut,
    /// Non-cutting rapid positioning traverse.
    Travel,
}

impl PassRole {
    pub fn as_str(self) -> &'static str {
        match self {
            PassRole::Roughing => "roughing",
            PassRole::Finishing => "finishing",
            PassRole::Perimeter => "perimeter",
            PassRole::Infill => "infill",
            PassRole::Support => "support",
            PassRole::LeadIn => "lead_in",
            PassRole::LeadOut => "lead_out",
            PassRole::Travel => "travel",
        }
    }

    /// Standard diagnostic hex color for visualizer rendering.
    pub fn default_color(self) -> &'static str {
        match self {
            PassRole::Roughing => "#2563eb",  // Blue
            PassRole::Finishing => "#16a34a", // Green
            PassRole::Perimeter => "#9333ea", // Purple
            PassRole::Infill => "#ca8a04",    // Yellow
            PassRole::Support => "#9ca3af",   // Gray
            PassRole::LeadIn => "#06b6d4",    // Cyan
            PassRole::LeadOut => "#f97316",   // Orange
            PassRole::Travel => "#ef4444",    // Red
        }
    }
}
