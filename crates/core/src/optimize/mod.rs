//! L2 optimisation passes — IR→IR transforms (`docs/01-architecture.md` §4). Conservative passes such
//! as `merge_collinear` preserve simulation metrics; shape-canonicalising passes such as `arc_fit` may
//! replace chord polylines with native controller arcs and therefore recompute geometric length/time.
//!
//! `merge_collinear` is the first: it coalesces consecutive collinear moves that share *all* process
//! state (feedrate, bead, channels, orientation, travel/extrude) into one longer move — dropping the
//! redundant intermediate point. Length, volume and filament are summed, so `simulate` is unchanged
//! except for the (now lower) segment count.

mod arc;
mod merge;
mod travel;

#[cfg(test)]
mod tests;

use crate::ir::Toolpath;

pub use self::arc::arc_fit;
pub use self::merge::merge_collinear;
pub use self::travel::travel_reorder;

/// The standard L2 optimisation pipeline exposed by every adapter. This is intentionally named at the
/// core boundary so CLI, wasm, and SDKs do not drift into different meanings for "optimized".
pub fn optimize_pipeline(tp: &Toolpath) -> Toolpath {
    travel_reorder(&arc_fit(&merge_collinear(tp)))
}
