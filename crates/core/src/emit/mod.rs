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
mod step_nc;

#[cfg(test)]
mod tests;

#[allow(deprecated)]
pub use self::gcode::emit;
pub(crate) use self::gcode::num as format_number;
pub(crate) use self::gcode::num_checked as format_number_checked;
pub use self::gcode::{emit_stream, emit_stream_to_writer, CncFrame, EmitParams, FirmwareFlavor};
pub use self::kinematics::{Kinematics, REFERENCE_FIVE_AXIS_MACHINE};
pub use self::spline::SplineFlatteningIterator;
pub use self::step_nc::emit_step_nc;
