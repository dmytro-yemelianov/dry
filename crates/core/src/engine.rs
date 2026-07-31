//! The engine — analyses and lowerings over the Dry IR (`docs/01-architecture.md` §7).
//!
//! P0 provides `simulate`: a pure fold over an L2 [`Toolpath`] producing print metrics. It is
//! validated byte-for-number against the FullControl oracle (`docs/03-conformance.md`); the accounting
//! below mirrors FullControl's *observed behaviour* (clean-room — reproduced, not copied):
//! time = Σ length/speed·60 (speed is mm/min), split into print (extruding) and travel; distances and
//! material summed likewise; filament-only prime moves use filament/speed for duration without faking XYZ
//! path length; `segment_count` counts moves with non-zero duration; `max_flow_rate` is the peak per-move
//! volumetric flow.

use crate::ir::Toolpath;
use crate::units::{Feedrate, Flow, Length, Time, Volume};
use serde::{Deserialize, Serialize};

/// Print metrics for a toolpath. Each field is unit-typed ([`Time`] seconds, [`Length`] mm,
/// [`Volume`] mm³, [`Flow`] mm³/s) and `#[serde(transparent)]`, so the JSON stays bare numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Metrics {
    pub total_time_s: Time,
    pub print_time_s: Time,
    pub travel_time_s: Time,
    pub extruding_distance: Length,
    pub travel_distance: Length,
    pub extruded_volume: Volume,
    pub filament_length: Length,
    pub segment_count: u64,
    pub max_flow_rate: Flow,
}

/// The duration of a move, or `None` when it has none.
///
/// Two distinct cases return `None` and must not be conflated: a segment that does not move (a
/// dwell, a channel-only or manual-g-code segment) has *no* duration to compute, while a segment
/// that moves at a non-positive feedrate has an *undefined* one. Zero means "not stated by the
/// source program" — motion before the first `F` inherits the machine's modal feedrate — and is the
/// branch `Dry.Semantics.SimulateMetrics.segmentMotionTime` models exactly (`FM1.SIMULATE_METRICS`,
/// pinned by `proofs/fixtures/simulate-metrics-refinement-v0.json`), so it keeps contributing
/// nothing. A *negative* feedrate is outside that modelled branch (the claim excludes "invalid or
/// zero speed behavior outside the modeled branch"); it used to yield a negative duration that was
/// subtracted from `total_time_s`, and is now un-timeable instead. Ingress refuses it besides:
/// see `gcode/lift.rs` and `codec/threemf.rs`.
pub(crate) fn segment_motion_time(s: &crate::ir::Segment) -> Option<Time> {
    let distance = if s.length > Length::ZERO {
        s.length
    } else if s.filament != Length::ZERO {
        // a filament-only prime/retract is timed against the extruder axis.
        Length::mm(s.filament.value().abs())
    } else {
        return None; // nothing to time
    };
    if !s.speed.value().is_finite() || s.speed <= Feedrate::ZERO {
        return None; // undefined duration
    }
    Some(distance / s.speed)
}

/// Fold a streaming iterator of segments into print metrics.
pub fn simulate_stream<I>(segments: I) -> Result<Metrics, crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<crate::ir::Segment, crate::codec::CodecError>>,
{
    let mut m = Metrics::default();
    for res in segments {
        let s = res?;
        // material accrues on every move (a zero-length move deposits nothing).
        m.extruded_volume = m.extruded_volume + s.volume;
        m.filament_length = m.filament_length + s.filament;

        // a dwell contributes only time (it does not move).
        if let Some(secs) = s.dwell_s {
            m.total_time_s = m.total_time_s + Time(secs);
        }

        if let Some(t) = segment_motion_time(&s) {
            m.total_time_s = m.total_time_s + t;
            m.segment_count += 1;
            if s.travel {
                m.travel_time_s = m.travel_time_s + t;
                m.travel_distance = m.travel_distance + s.length;
            } else {
                m.print_time_s = m.print_time_s + t;
                m.extruding_distance = m.extruding_distance + s.length;
            }
            let flow = s.volume / t; // mm³/s
            if flow > m.max_flow_rate {
                m.max_flow_rate = flow;
            }
        }
    }
    Ok(m)
}

/// Fold a toolpath into its print metrics.
pub fn simulate(tp: &Toolpath) -> Metrics {
    simulate_stream(tp.segments.iter().cloned().map(Ok)).unwrap()
}
