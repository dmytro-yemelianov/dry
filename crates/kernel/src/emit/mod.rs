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
// is judged against the program that will actually be written. Emitter mechanics, not authoring
// API — public only so `drymachina-verify` can reach it across the crate boundary (plan Task 1).
pub use self::kinematics::RotaryState;
// The kinematic geometry of a `Kinematics`, an extension trait because the enum itself lives in
// `drymachina-contracts` (plan Task 3). Public, like `RotaryState` above and for the same reason: the
// rotary rules resolve their angles through it, and that call now crosses a crate boundary. `Joints`
// and `Rotary` travel with it because they are what its methods hand back (plan Task 4).
pub use self::kinematics::{Joints, KinematicsExt, Rotary};
pub use self::spline::SplineFlatteningIterator;
pub use self::step_nc::emit_step_nc;
