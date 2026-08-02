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
mod krl;
mod spline;
mod step_nc;

#[cfg(test)]
mod tests;

#[allow(deprecated)]
pub use self::gcode::emit;
pub(crate) use self::gcode::num as format_number;
pub(crate) use self::gcode::num_checked as format_number_checked;
pub use self::gcode::{emit_stream, emit_stream_to_writer, CncFrame, EmitParams, FirmwareFlavor};
pub use self::kinematics::{Kinematics, REFERENCE_FIVE_AXIS_LIMITS, REFERENCE_FIVE_AXIS_MACHINE};
pub use self::krl::{KrlFrame, KrlTransform};
// `verify` resolves rotary angles through the same state the emitter threads, so a rotary limit
// is judged against the program that will actually be written. Crate-internal: it is emitter
// mechanics, not public API.
pub(crate) use self::kinematics::RotaryState;
pub use self::spline::SplineFlatteningIterator;
pub use self::step_nc::emit_step_nc;
