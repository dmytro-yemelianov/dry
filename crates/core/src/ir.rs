//! The Dry IR — L2 motion dialect (v0).
//!
//! A `Toolpath` is an ordered stream of resolved `Segment`s (machine-agnostic moves with absolute
//! state). This is the level `simulate`/`verify`/`optimise` operate on (`docs/01-architecture.md` §1).
//!
//! v0's fields are **unit-typed** (via [`crate::units`]): coordinates and lengths are [`Length`],
//! the feedrate is [`Feedrate`], deposited material is [`Volume`]. Each quantity is
//! `#[serde(transparent)]`, so the JSON wire format is unchanged (bare numbers) and stays byte-identical
//! to the conformance corpora. The general toolframe (orientation), the channel registry, and the
//! binary/columnar encoding land in P0.3+.

use crate::units::{Feedrate, Length, Volume};
use serde::{Deserialize, Serialize};

/// One resolved move from `start` to `end` (absolute). An axis is `None` when undefined before it is
/// first set (e.g. the very first positioning move). `length` is the true path length (arc length for
/// arcs; 0 for a pure positioning move). `volume`/`filament` are the material this move deposits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start: [Option<Length>; 3],
    pub end: [Option<Length>; 3],
    pub travel: bool,
    /// Feedrate (g-code `F`).
    pub speed: Feedrate,
    /// Path length.
    pub length: Length,
    /// Deposited material volume.
    pub volume: Volume,
    /// Feedstock consumed (a length of filament).
    pub filament: Length,
    pub width: Option<Length>,
    pub height: Option<Length>,
    /// `"line"` or `"arc"` (v0; becomes an enum in P1).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Arc centre `(cx, cy)` in absolute coordinates — present only when `kind == "arc"`.
    #[serde(default)]
    pub centre: Option<[Length; 2]>,
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
