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
    /// `"line"`, `"arc"`, or `"dwell"` (v0; becomes an enum in P1).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Arc centre `(cx, cy)` in absolute coordinates — present only when `kind == "arc"`.
    #[serde(default)]
    pub centre: Option<[Length; 2]>,
    /// Arc direction: `true` → G2 (clockwise), `false` → G3.
    #[serde(default)]
    pub clockwise: bool,

    // ---- process channels (§3): typed, defaulted, propagated by `resolve`. Each is omitted from the
    // wire form when unset, so a motion-only toolpath serialises byte-identically to a channel-free IR.
    /// Nozzle temperature (°C) in effect for this move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Part-cooling fan (0..1) in effect for this move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fan: Option<f64>,
    /// Flow multiplier applied to the deposited volume — omitted when the default (1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<f64>,
    /// Active tool index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<u32>,
    /// Dwell duration (s) — present only when `kind == "dwell"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dwell_s: Option<f64>,
    /// Toolframe orientation: the tool-direction unit vector `(i, j, k)`. `None` ⇒ identity (+Z), i.e.
    /// 3-axis. Carrying it makes non-planar / 5-axis a first-class IR property (§2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<[f64; 3]>,
    /// Spline control points — present only when kind == "spline".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control_points: Option<Vec<[Length; 3]>>,
}

fn default_kind() -> String {
    "line".to_string()
}

/// The self-describing IR **header**: optional provenance and declared invariants. Every field is
/// omitted from the wire form when empty, so a toolpath with no header (`Toolpath.meta == None`)
/// serialises byte-identically to a header-free IR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Meta {
    /// The tool (and version) that produced this toolpath, e.g. `"dry 0.0.0"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    /// The length unit the coordinates are in, e.g. `"mm"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    /// A content hash of the source design (hex), for provenance/caching.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    /// Declared contract names the toolpath claims to satisfy (see [`crate::verify`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invariants: Vec<String>,
}

/// A resolved toolpath: an ordered stream of moves. The `version` tags the Dry IR schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toolpath<I = Vec<Segment>> {
    #[serde(default)]
    pub version: u32,
    /// Optional self-describing header (provenance + declared invariants). Absent ⇒ no `meta` key on
    /// the wire (byte-identity with a header-free IR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub segments: I,
}

impl Toolpath<Vec<Segment>> {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Toolpath serialises")
    }

    /// Encode to the compact columnar binary form (`docs/01-architecture.md` §6; see [`crate::codec`]).
    pub fn to_bytes(&self) -> Vec<u8> {
        crate::codec::encode(self)
    }

    /// Decode from the columnar binary form. Lossless: `from_bytes(&to_bytes()) == self`.
    pub fn from_bytes(buf: &[u8]) -> Result<Self, crate::codec::CodecError> {
        crate::codec::decode(buf)
    }
}

impl<I> IntoIterator for Toolpath<I>
where
    I: IntoIterator,
{
    type Item = I::Item;
    type IntoIter = I::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.into_iter()
    }
}
