//! `emit` — lower an L2 [`crate::ir::Toolpath`] to motion g-code (Marlin), reproducing FullControl's bytes
//! (`docs/03-conformance.md`, the strictest gate). Clean-room: the formatting rules below are Dry's
//! independent reimplementation of FullControl's *observed* output, not its code.
//!
//! Rules (per move): `G1` when extruding, `G0` when travelling; `F<speed>` only when the feedrate
//! changes; an axis `X`/`Y`/`Z` only when it changes; in relative-E mode the extruding move carries
//! `E<filament>` (a travel carries none, unless `travel_g1_e0`). Numbers are `{:.6}` with trailing
//! zeros and a trailing `.` stripped (so `1000.000000`→`1000`, `0.200000`→`0.2`, `0`→`0`).

mod canned;
mod chunked_stream;
mod gcode;
mod kinematics;
mod krl;
mod laser;
mod plasma;
mod rapid;
mod spline;
mod step_nc;
mod template;

#[cfg(test)]
mod tests;

pub use self::canned::{emit_cycle_cancel, DrillCycle, PeckDrillCycle};
pub use self::chunked_stream::emit_gcode_chunks;
#[allow(deprecated)]
pub use self::gcode::emit;
pub(crate) use self::gcode::num as format_number;
pub(crate) use self::gcode::num_checked as format_number_checked;
pub use self::gcode::{emit_stream, emit_stream_to_writer, CncFrame, EmitParams, FirmwareFlavor};
pub use self::kinematics::{
    DhParam, Kinematics, Robot6AxisModel, RobotJoints6, REFERENCE_FIVE_AXIS_LIMITS,
    REFERENCE_FIVE_AXIS_MACHINE,
};
pub use self::krl::{KrlFrame, KrlTransform};
pub use self::laser::{emit_grbl_laser, LaserError, LaserMode, LaserParams};
pub use self::plasma::{emit_plasma_waterjet, CuttingParams, LeadInType};
pub use self::rapid::emit_rapid_to_writer;
pub use self::template::{render_template, GcodeTemplate, TemplateContext};
// `verify` resolves rotary angles through the same state the emitter threads, so a rotary limit
// is judged against the program that will actually be written. Crate-internal: it is emitter
// mechanics, not public API.
pub(crate) use self::kinematics::RotaryState;
pub use self::spline::SplineFlatteningIterator;
pub use self::step_nc::emit_step_nc;
