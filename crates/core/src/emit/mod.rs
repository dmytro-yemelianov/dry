//! `emit` — lower an L2 [`Toolpath`] to motion g-code (Marlin), reproducing FullControl's bytes
//! (`docs/03-conformance.md`, the strictest gate). Clean-room: the formatting rules below are Dry's
//! independent reimplementation of FullControl's *observed* output, not its code.
//!
//! Rules (per move): `G1` when extruding, `G0` when travelling; `F<speed>` only when the feedrate
//! changes; an axis `X`/`Y`/`Z` only when it changes; in relative-E mode the extruding move carries
//! `E<filament>` (a travel carries none, unless `travel_g1_e0`). Numbers are `{:.6}` with trailing
//! zeros and a trailing `.` stripped (so `1000.000000`→`1000`, `0.200000`→`0.2`, `0`→`0`).

mod gcode;
mod kinematics;
mod spline;

#[cfg(test)]
mod tests;

pub use self::gcode::{emit, emit_stream, emit_stream_to_writer, EmitParams};
pub use self::kinematics::Kinematics;
pub use self::spline::SplineFlatteningIterator;
