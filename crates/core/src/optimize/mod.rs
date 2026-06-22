//! L2 optimisation passes — IR→IR transforms (`docs/01-architecture.md` §4). Conservative passes such
//! as `merge_collinear` preserve simulation metrics; shape-canonicalising passes such as `arc_fit` may
//! replace chord polylines with native controller arcs and therefore recompute geometric length/time.
//!
//! `merge_collinear` is the first: it coalesces consecutive collinear moves that share *all* process
//! state (feedrate, bead, channels, orientation, travel/extrude) into one longer move — dropping the
//! redundant intermediate point. Length, volume and filament are summed, so `simulate` is unchanged
//! except for the (now lower) segment count.

mod adaptive_speed;
mod arc;
mod coasting;
mod merge;
mod travel;
mod z_hop;

#[cfg(test)]
mod tests;

use crate::ir::Toolpath;

pub use self::adaptive_speed::{adaptive_speed, adaptive_speed_with_params};
pub use self::arc::arc_fit;
pub use self::coasting::{coasting, coasting_with_dist};
pub use self::merge::merge_collinear;
pub use self::travel::travel_reorder;
pub use self::z_hop::{z_hop, z_hop_with_params};

/// The standard L2 optimisation pipeline exposed by every adapter. It only runs geometry-local passes
/// that do not reorder authored/source motion.
pub fn optimize_pipeline(tp: &Toolpath) -> Toolpath {
    arc_fit(&merge_collinear(tp))
}

/// An order-changing L2 optimisation pipeline. This may reduce travel but can change thermal/seam/process
/// sequencing, so callers should expose it as an explicit opt-in.
pub fn optimize_aggressive_pipeline(tp: &Toolpath) -> Toolpath {
    let tp = merge_collinear(tp);
    let tp = arc_fit(&tp);
    let tp = adaptive_speed(&tp);
    let tp = coasting(&tp);
    let tp = travel_reorder(&tp);
    z_hop(&tp)
}
