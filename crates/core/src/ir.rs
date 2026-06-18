//! The Dry IR — L2 motion dialect (v0).
//!
//! A `Toolpath` is an ordered stream of resolved `Segment`s (machine-agnostic moves with absolute
//! state). This is the level `simulate`/`verify`/`optimise` operate on (`docs/01-architecture.md` §1).
//!
//! v0 carries the fields as `f64`; unit-typing (via [`crate::units`]), the general toolframe
//! (orientation), the channel registry, and the binary/columnar encoding land in P0.2/P0.3. JSON
//! (de)serialisation is provided now so the engine can be driven by the conformance corpora.

use serde::{Deserialize, Serialize};

/// One resolved move from `start` to `end` (absolute, mm). An axis is `None` when undefined before it
/// is first set (e.g. the very first positioning move). `length` is the true path length (arc length
/// for arcs; 0 for a pure positioning move). `volume`/`filament` are the material this move deposits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: [Option<f64>; 3],
    pub end: [Option<f64>; 3],
    pub travel: bool,
    /// Feedrate, mm/min.
    pub speed: f64,
    /// Path length, mm.
    pub length: f64,
    /// Deposited material volume, mm³.
    pub volume: f64,
    /// Feedstock consumed, mm.
    pub filament: f64,
    pub width: Option<f64>,
    pub height: Option<f64>,
    /// `"line"` or `"arc"` (v0; becomes an enum in P1).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Arc centre `(cx, cy)` in absolute mm — present only when `kind == "arc"`.
    #[serde(default)]
    pub centre: Option<[f64; 2]>,
    /// Arc direction: `true` → G2 (clockwise), `false` → G3.
    #[serde(default)]
    pub clockwise: bool,
}

fn default_kind() -> String {
    "line".to_string()
}

/// A resolved toolpath: an ordered stream of moves. The `version` tags the Dry IR schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toolpath {
    #[serde(default)]
    pub version: u32,
    pub segments: Vec<Segment>,
}

impl Toolpath {
    pub fn from_json(s: &str) -> Result<Toolpath, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Toolpath serialises")
    }
}
