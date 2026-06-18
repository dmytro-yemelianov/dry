//! The engine — analyses and lowerings over the Dry IR (`docs/01-architecture.md` §7).
//!
//! P0 provides `simulate`: a pure fold over an L2 [`Toolpath`] producing print metrics. It is
//! validated byte-for-number against the FullControl oracle (`docs/03-conformance.md`); the accounting
//! below mirrors FullControl's *observed behaviour* (clean-room — reproduced, not copied):
//! time = Σ length/speed·60 (speed is mm/min), split into print (extruding) and travel; distances and
//! material summed likewise; `segment_count` counts moves with non-zero length; `max_flow_rate` is the
//! peak per-move volumetric flow.

use crate::ir::Toolpath;
use serde::{Deserialize, Serialize};

/// Print metrics for a toolpath. Times in seconds, distances in mm, volume in mm³, flow in mm³/s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Metrics {
    pub total_time_s: f64,
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub extruding_distance: f64,
    pub travel_distance: f64,
    pub extruded_volume: f64,
    pub filament_length: f64,
    pub segment_count: u64,
    pub max_flow_rate: f64,
}

/// Fold a toolpath into its print metrics.
pub fn simulate(tp: &Toolpath) -> Metrics {
    let mut m = Metrics::default();
    for s in &tp.segments {
        // material accrues on every move (a zero-length move deposits nothing).
        m.extruded_volume += s.volume;
        m.filament_length += s.filament;

        if s.length > 0.0 && s.speed != 0.0 {
            let t = s.length / s.speed * 60.0; // mm / (mm/min) → minutes → seconds
            m.total_time_s += t;
            m.segment_count += 1;
            if s.travel {
                m.travel_time_s += t;
                m.travel_distance += s.length;
            } else {
                m.print_time_s += t;
                m.extruding_distance += s.length;
            }
            let flow = s.volume / t; // mm³/s
            if flow > m.max_flow_rate {
                m.max_flow_rate = flow;
            }
        }
    }
    m
}
