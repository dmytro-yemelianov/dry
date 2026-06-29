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
use crate::verify::Contracts;

pub use self::adaptive_speed::{adaptive_speed, adaptive_speed_with_params};
pub use self::arc::arc_fit;
pub use self::coasting::{coasting, coasting_with_dist};
pub use self::merge::merge_collinear;
pub use self::travel::travel_reorder;
pub use self::z_hop::{z_hop, z_hop_with_params};

/// The optimisation aggressiveness a caller opts into. Every mode is gated against the verifier the same
/// way (see [`apply_gated`]): a span's rewrite is kept only when it introduces no new error rule
/// (`docs/11-profiles-and-reports.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizeMode {
    /// Geometry canonicalisation only (`merge_collinear` then `arc_fit`), gated per span.
    Safe,
    /// `Safe` plus conservative `adaptive_speed` shaping (junction/curvature feedrate scaling); no
    /// reordering, coasting or z-hop.
    Balanced,
    /// The full order-changing pipeline on top of `Balanced`: also `coasting`, `travel_reorder` and
    /// `z_hop`.
    Max,
}

/// The standard L2 optimisation pipeline exposed by every adapter. It only runs geometry-local passes
/// that do not reorder authored/source motion.
pub fn optimize_pipeline(tp: &Toolpath) -> Toolpath {
    arc_fit(&merge_collinear(tp))
}

/// The `safe` optimisation pipeline: exactly the geometry-canonicalisation subset (`merge_collinear`
/// then `arc_fit`). It is the body of [`optimize_pipeline`] under a name that pins its `safe`-mode role;
/// it deliberately excludes the adaptive-speed / coasting / travel-reorder / z-hop passes (reserved for
/// `balanced`/`max`).
pub fn safe_pipeline(tp: &Toolpath) -> Toolpath {
    arc_fit(&merge_collinear(tp))
}

/// The `balanced` optimisation pipeline: the [`safe_pipeline`] geometry subset followed by conservative
/// `adaptive_speed` shaping (junction/curvature feedrate scaling). It deliberately stops short of the
/// order-changing `coasting` / `travel_reorder` / `z_hop` passes (reserved for `max`).
pub fn balanced_pipeline(tp: &Toolpath) -> Toolpath {
    adaptive_speed(&safe_pipeline(tp))
}

/// The `max` optimisation pipeline: the full order-changing body
/// (`merge_collinear` → `arc_fit` → `adaptive_speed` → `coasting` → `travel_reorder` → `z_hop`). This may
/// reduce travel/segment count but can change thermal/seam/process sequencing, so it is only reached
/// through the per-span gate.
pub fn max_pipeline(tp: &Toolpath) -> Toolpath {
    optimize_aggressive_pipeline(tp)
}

/// The IR→IR pipeline a given [`OptimizeMode`] runs before the gate decides whether to keep it.
fn pipeline_for(mode: OptimizeMode, tp: &Toolpath) -> Toolpath {
    match mode {
        OptimizeMode::Safe => safe_pipeline(tp),
        OptimizeMode::Balanced => balanced_pipeline(tp),
        OptimizeMode::Max => max_pipeline(tp),
    }
}

/// The outcome of a single gated rewrite ([`apply_gated`] / [`apply_safe_gated`]).
#[derive(Debug, Clone)]
pub struct GatedResult {
    /// The rewritten toolpath when accepted, or the input verbatim when rejected.
    pub toolpath: Toolpath,
    /// Whether the rewrite was accepted (introduced no new error rule).
    pub accepted: bool,
    /// The error rule ids the rewrite would have *introduced* (empty when accepted).
    pub new_error_rules: Vec<String>,
}

/// Run the pipeline for `mode` and accept the result only if it introduces no **new** error rule
/// relative to the input under `contracts`. Pre-existing input errors do not block; new warning-only
/// findings do not block. On rejection the input is returned verbatim, with the offending rule ids in
/// `new_error_rules`. Apply this per motion span so a rejected span passes through while its neighbours
/// are still rewritten.
pub fn apply_gated(tp: &Toolpath, contracts: &Contracts, mode: OptimizeMode) -> GatedResult {
    use crate::verify::{verify, Severity};
    use std::collections::BTreeSet;

    let error_rules = |tp: &Toolpath| -> BTreeSet<String> {
        verify(tp, contracts)
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.rule.clone())
            .collect()
    };

    let pre_errors = error_rules(tp);
    let rewritten = pipeline_for(mode, tp);
    let post_errors = error_rules(&rewritten);
    let new_error_rules: Vec<String> = post_errors.difference(&pre_errors).cloned().collect();

    if new_error_rules.is_empty() {
        GatedResult {
            toolpath: rewritten,
            accepted: true,
            new_error_rules: vec![],
        }
    } else {
        GatedResult {
            toolpath: tp.clone(),
            accepted: false,
            new_error_rules,
        }
    }
}

/// The `safe`-mode gate: [`apply_gated`] with [`OptimizeMode::Safe`]. Kept as a thin wrapper for the
/// existing callers/tests.
pub fn apply_safe_gated(tp: &Toolpath, contracts: &Contracts) -> GatedResult {
    apply_gated(tp, contracts, OptimizeMode::Safe)
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
