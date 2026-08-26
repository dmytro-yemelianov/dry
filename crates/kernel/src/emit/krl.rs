//! KUKA Robot Language (KRL) program emission.
//!
//! # What this is, and what it is not
//!
//! The programs written here are **structurally well-formed against the grammar recorded below and
//! have never been executed on a KUKA controller or on a KUKA simulator.** Nothing in this module,
//! and nothing in the test suite, establishes that a KRC will run the output. What *is* established
//! is checked by an oracle outside Dry: `tools/krl_check.sh` parses emitted programs with the
//! independently-authored ANTLR grammar `kuka/krl.g4` from `antlr/grammars-v4` (Jan Schlößin,
//! 2010–2011, from the reverse-engineering study arXiv:1009.5004; ANTLR4 port by Tom Everett, 2016).
//! That grammar is nobody's here, which is the whole point — a program accepted by Dry's own parser
//! proves only that Dry agrees with itself. See `docs/22-krl-emit.md`.
//!
//! # The subset Dry emits
//!
//! ```text
//! program        ::= "DEF" name "(" ")" NL comment* frame_setup statement* "END" NL
//! frame_setup    ::= tool_line base_line
//! tool_line      ::= "$TOOL" "=" frame NL
//! base_line      ::= "$BASE" "=" frame NL
//! statement      ::= apo_line | vel_line | motion | dwell
//! apo_line       ::= "$APO.CDIS" "=" real NL
//! vel_line       ::= "$VEL.CP" "=" real NL
//! motion         ::= ptp | lin | circ
//! ptp            ::= "PTP" pose NL
//! lin            ::= "LIN" pose approx? NL
//! circ           ::= "CIRC" pose "," pose approx? NL
//! dwell          ::= "WAIT" "SEC" real NL
//! approx         ::= "C_DIS"
//! pose           ::= "{" "E6POS" ":" component ("," component)* "}"
//! component      ::= ("X"|"Y"|"Z"|"A"|"B"|"C") real
//! frame          ::= "{" component ("," component)* "}"
//! real           ::= "-"? digit+ "." digit*
//! comment        ::= ";" <any text to end of line> NL
//! name           ::= (letter|"_") (letter|digit|"_"){0,23}
//! ```
//!
//! Two placement rules the grammar above does not express: `apo_line` is written at most once per
//! program and only immediately before the first `lin`/`circ`, and `vel_line` only immediately
//! before a `lin`/`circ` whose velocity differs from the last one written. Both exist so that no
//! line states machine state that no instruction goes on to reference (ADR 0002 §4).
//!
//! Every production above is a subset of the external grammar's, which is what
//! `tools/krl_check.sh` actually enforces; this listing is what Dry *intends* to stay inside.
//! There is no passthrough production: [`SegmentKind::ManualGcode`] carries *g-code*
//! (`spec/dry-ir-v0.schema.json`), which is not KRL, and is refused rather than copied through.
//!
//! # Conventions, and where they come from
//!
//! **Orientation.** KUKA writes an orientation as ZYX Euler angles applied to the moving frame:
//! `A` about Z, then `B` about the new Y, then `C` about the resulting X — i.e.
//! `R = Rz(A)·Ry(B)·Rx(C)` (KUKA System Software operating/programming manual, "Euler angles ZYX";
//! restated in RoboDK's *Robot Euler Angles* primer and Mecademic's orientation tutorial).
//!
//! [`crate::ir::Segment::orientation`] is a unit **tool-axis** vector: `[0, 0, 1]` is the untilted
//! tool, pointing away from the work. A KUKA `$TOOL` frame's Z axis points the other way — out of
//! the flange, along the tool, *toward* the work — so the pose must satisfy `R·[0,0,1] = −d`. With
//! the roll pinned at `C = 180°` that solves exactly to
//!
//! ```text
//! A = atan2(j, i)      B = acos(k)      C = 180
//! ```
//!
//! which is [`Kinematics::Bc`] with zero offsets, already implemented in
//! [`super::kinematics`] — so this module resolves through it rather than re-deriving the trig, and
//! inherits its singular-cone hold (at `d = ±Z` the `A` angle is exactly as undetermined as that
//! model's `C` word, and is held rather than swung to zero). The untilted tool comes out as
//! `A 0, B 0, C 180`, the canonical KUKA tool-pointing-down pose, which is the check that the sign
//! convention is the right way round.
//!
//! Dry's IR carries no roll about the tool axis, so `C` is a choice, not a measurement; `180°` is
//! the choice that makes the untilted case canonical. `B` is `acos(k) ∈ [0°, 180°]` and is
//! deliberately **not** folded into KUKA's standardised readback interval `[−90°, 90°]`: the fold
//! `(A, B, C) ≡ (A±180, 180−B, C±180)` is exact but discontinuous at `B = 90°`, and a program that
//! jumps `A` by half a turn because the tool crossed horizontal commands a reorientation the
//! geometry never asked for. A controller normalises on readback; a program does not have to.
//!
//! **Velocity.** `$VEL.CP` is the Cartesian path velocity in **m/s**;
//! [`crate::units::Feedrate`] is mm/min, so the conversion is `f / 60000`. It governs CP motion
//! (`LIN`, `CIRC`) only, which is why it is emitted before those and not before `PTP`. **`PTP`
//! velocity comes from `$VEL_AXIS[]`, a percentage of each joint's maximum, which Dry cannot derive
//! from a Cartesian feedrate and therefore does not set** — a `PTP` here runs at whatever the
//! controller was last told. The emitted banner says so in the program itself.
//!
//! **Approximation.** `$APO.CDIS` plus `C_DIS` on the instruction is the only blending Dry emits,
//! and only when the caller supplies [`KrlFrame::approx_mm`] *and* the program goes on to contain a
//! CP instruction that carries `C_DIS`. Absent either, every motion is exact positioning and no
//! `$APO` line is written at all: setting an approximation distance that no instruction references
//! would be a vacuous emission (ADR 0002 §4), which is why the line is written lazily at the first
//! `LIN`/`CIRC` rather than in the frame prologue. `PTP` approximation is a different pair
//! (`C_PTP` with `$APO.CPTP` in percent) and is not emitted.
//!
//! **`CIRC`.** Dry previously wrote a `CIRC … C<i> D<j>` centre offset, which is not KRL — it was
//! an RS-274 `I`/`J` pair under two spare letters. Real KRL takes an **auxiliary point** and an end
//! point: `CIRC aux, end`. This module computes the auxiliary point at the midpoint of the swept
//! angle, which fixes both the plane and the direction of travel, so clockwise and counter-clockwise
//! arcs are no longer indistinguishable. `CA` (circular angle) is deliberately not emitted: with
//! three points the sweep is already determined, and `CA` would *override* the programmed end point
//! rather than confirm it.

use super::gcode::{num_checked, write_line, EmitParams};
use super::kinematics::{Kinematics, KinematicsExt, RotaryState};
use super::SplineFlatteningIterator;
use crate::codec::CodecError;
use crate::ir::SegmentKind;
use crate::units::{Feedrate, Length};
use serde::{Deserialize, Serialize};

/// The `DEF` identifier used when the caller names no program.
const DEFAULT_PROGRAM_NAME: &str = "dry";

/// KUKA's limit on the length of a KRL name.
const MAX_NAME_LEN: usize = 24;

/// The roll about the tool axis written into every pose, in degrees. See the module docs: the IR
/// carries no roll, and `180°` is what makes the untilted tool the canonical `A 0, B 0, C 180`.
const TOOL_ROLL_DEG: f64 = 180.0;

/// mm/min per m/s — the whole of the `Feedrate` → `$VEL.CP` conversion.
const MM_PER_MIN_PER_M_PER_S: f64 = 60_000.0;

/// Decimal places for `$VEL.CP`.
///
/// Not a tolerance and not tunable policy: `1e-12 m/s` is `6e-8 mm/min`, an order of magnitude below
/// the `1e-6 mm/min` that [`num_checked`]'s `{v:.6}` can print on the g-code side. Anything coarser
/// would let two feedrates the rest of the emitter can tell apart collapse to one `$VEL.CP`.
///
/// It does set one refusal boundary, so it is stated here rather than left to be inferred: a
/// feedrate below half an ulp of this resolution — `3e-8 mm/min`, i.e. `5e-13 m/s` — rounds to the
/// literal `0.0`, and [`vel_cp`] refuses it for that reason. The predicate is exact (the formatted
/// literal either is zero or is not), so it carries no epsilon of its own; like the g-code side's
/// `{v:.6}` coordinate resolution it is a *format* constant that a `proofs/` entry records as an
/// assumption rather than as a budget.
const VEL_CP_DECIMALS: usize = 12;

/// The model the orientation is resolved through. `Bc` with zero offsets *is* the KUKA ZYX-Euler
/// decomposition with the roll pinned (module docs), so this is a reuse of
/// [`super::kinematics`], not a second derivation — including its singular-cone hold.
const ORIENTATION_MODEL: Kinematics = Kinematics::Bc {
    pivot_offset: [0.0, 0.0, 0.0],
    rotary_offset: [0.0, 0.0],
};

/// The three lines Dry puts at the top of every program it writes.
///
/// In the program rather than only in the docs on purpose: the file is what reaches an operator, and
/// the two things it must not let them assume are that this has run somewhere and that `PTP` speed
/// is under control.
///
/// It says *checkable with*, not *checked against*. The emitter runs no grammar; `tools/krl_check.sh`
/// is a separate command, and the only file in this repository that has been through it is the
/// golden. A banner asserting a check that did not run on the program carrying it would be exactly
/// the circular claim this target's rewrite exists to remove.
const BANNER: [&str; 5] = [
    ";  Emitted by dry: never run on a KUKA controller or simulator.",
    ";  The structure of THIS program has not been checked either -- dry emits KRL,",
    ";  it does not parse it. tools/krl_check.sh checks a file against an external",
    ";  KRL grammar that nobody here wrote.",
    ";  PTP speed is $VEL_AXIS[] (percent of maximum), which dry does not set.",
];

/// A KRL `FRAME`: a translation in mm and a ZYX-Euler orientation in degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct KrlTransform {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub a: f64,
    #[serde(default)]
    pub b: f64,
    #[serde(default)]
    pub c: f64,
}

impl KrlTransform {
    fn components(&self) -> [(char, f64); 6] {
        [
            ('X', self.x),
            ('Y', self.y),
            ('Z', self.z),
            ('A', self.a),
            ('B', self.b),
            ('C', self.c),
        ]
    }

    fn validate(&self, what: &str) -> Result<(), String> {
        for (letter, value) in self.components() {
            if !value.is_finite() {
                return Err(format!("{what}.{letter} must be finite, got {value}"));
            }
        }
        Ok(())
    }
}

/// The `DEF` wrapper and the frame Dry pins ahead of the motion.
///
/// The default is the whole point of the type: an identity `$TOOL`/`$BASE` and no approximation.
/// Leaving those unstated would run the program against whatever tool and base the operator last
/// selected, which is the same hazard [`super::CncFrame`] closes by always writing a `G54`.
///
/// **The identity `$TOOL` puts the TCP at the flange**, so an emitted `X`/`Y`/`Z` is a flange point
/// and the tool's own length is not accounted for anywhere. Supply [`KrlFrame::tool`] with the real
/// flange→TCP transform to make the emitted coordinates tool-tip coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct KrlFrame {
    /// The `DEF` routine identifier. `None` ⇒ [`DEFAULT_PROGRAM_NAME`]. KUKA requires the routine
    /// name to match the module's file name, which only the caller writing the file knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_name: Option<String>,
    /// `$TOOL`, the flange→TCP transform. Default identity; see the type docs.
    #[serde(default)]
    pub tool: KrlTransform,
    /// `$BASE`, the world→work transform. Default identity.
    #[serde(default)]
    pub base: KrlTransform,
    /// `$APO.CDIS` in mm, and a `C_DIS` on every CP instruction. `None` ⇒ exact positioning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approx_mm: Option<f64>,
}

impl KrlFrame {
    /// Validate the frame the way [`super::CncFrame::validate`] validates its own: the fields are
    /// `pub` and `Deserialize`, so a frame reaches the emitter without ever passing through a
    /// profile, and a bad one renders as a syntactically plausible program with a nonsense tool.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(name) = &self.program_name {
            validate_program_name(name)?;
        }
        self.tool.validate("$TOOL")?;
        self.base.validate("$BASE")?;
        if let Some(mm) = self.approx_mm {
            if !(mm.is_finite() && mm > 0.0) {
                return Err(format!(
                    "krl_frame.approx_mm must be finite and > 0, got {mm}"
                ));
            }
        }
        Ok(())
    }
}

/// KRL names are `[A-Za-z_][A-Za-z0-9_]*`, at most 24 characters.
///
/// Refused rather than sanitised: a name Dry quietly rewrote would no longer match the file name the
/// caller chose, and KUKA rejects a module whose `DEF` and file name disagree.
fn validate_program_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(format!(
            "krl_frame.program_name must be 1..={MAX_NAME_LEN} characters, got {} in {name:?}",
            name.len()
        ));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or_default();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "krl_frame.program_name must start with a letter or underscore, got {name:?}"
        ));
    }
    if let Some(bad) = chars.find(|c| !(c.is_ascii_alphanumeric() || *c == '_')) {
        return Err(format!(
            "krl_frame.program_name may only contain letters, digits and underscores, got {bad:?} \
             in {name:?}"
        ));
    }
    Ok(())
}

/// Format a KRL `REAL` literal.
///
/// The external grammar's `FLOATLITERAL` needs the decimal point that [`super::gcode::num`] strips,
/// and an integer-looking component in a `FRAME` aggregate is an `INT` literal being coerced. Both
/// parse; writing the point keeps the emitted type unambiguous at no cost.
fn real(v: f64, word: impl std::fmt::Display) -> Result<String, CodecError> {
    let s = num_checked(v, word)?;
    Ok(if s.contains('.') { s } else { format!("{s}.0") })
}

/// Format `$VEL.CP` in m/s from a mm/min feedrate, refusing a value KRL cannot execute.
///
/// A `$VEL.CP` of zero or less is not a slow move, it is a controller fault, and `emit` is the last
/// gate before a machine (ADR 0002 §4). Two guards, because one is not enough: the first is on the
/// *input*, the second on the *literal about to be written*. A feedrate of `1e-8 mm/min` is finite
/// and positive and still prints as `0.0` at [`VEL_CP_DECIMALS`], so checking only the input let the
/// exact line this function exists to prevent reach the program.
fn vel_cp(mm_per_min: f64) -> Result<String, CodecError> {
    if !(mm_per_min.is_finite() && mm_per_min > 0.0) {
        return Err(CodecError::Other(format!(
            "cannot emit $VEL.CP from a feedrate of {mm_per_min} mm/min: KRL path velocity must be \
             finite and > 0"
        )));
    }
    let s = format!(
        "{:.*}",
        VEL_CP_DECIMALS,
        mm_per_min / MM_PER_MIN_PER_M_PER_S
    );
    // Exact, not a tolerance: the question is whether the emitted characters denote zero.
    if s.parse::<f64>().is_ok_and(|v| v == 0.0) {
        return Err(CodecError::Other(format!(
            "cannot emit $VEL.CP from a feedrate of {mm_per_min} mm/min: in m/s it rounds to 0 at \
             the {VEL_CP_DECIMALS} decimal places KRL path velocity is written with, and \
             $VEL.CP = 0 is a controller fault"
        )));
    }
    let s = s.trim_end_matches('0');
    Ok(if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_string()
    })
}

/// Render a `FRAME` aggregate, all six components stated.
fn frame_literal(t: &KrlTransform, what: &str) -> Result<String, CodecError> {
    let mut parts = Vec::with_capacity(6);
    for (letter, value) in t.components() {
        parts.push(format!(
            "{letter} {}",
            real(value, format!("{what}.{letter}"))?
        ));
    }
    Ok(format!("{{{}}}", parts.join(", ")))
}

/// Render an `E6POS` aggregate from already-formatted `LETTER value` components.
fn pose_literal(components: &[String]) -> String {
    format!("{{E6POS: {}}}", components.join(", "))
}

/// The signed angle swept from `start` to `end` about the centre, in radians.
///
/// Exact comparisons only, no epsilon: `rem_euclid` puts the raw difference in `[0, 2π)` and the two
/// branches place it in `(0, 2π]` or `[−2π, 0)`. A difference of exactly zero is a full turn, which
/// the caller has already refused — it is here so the function is total.
fn signed_sweep(start: f64, end: f64, clockwise: bool) -> f64 {
    let tau = std::f64::consts::TAU;
    let forward = (end - start).rem_euclid(tau);
    if clockwise {
        if forward == 0.0 {
            -tau
        } else {
            forward - tau
        }
    } else if forward == 0.0 {
        tau
    } else {
        forward
    }
}

/// Refuse a kinematic model whose offsets this target would silently drop.
///
/// [`Kinematics`] describes a machine tool's rotary table or tilting head: `pivot_offset` and
/// `rotary_offset` move the *linear* axes into a rotated machine frame. An `E6POS` is a TCP pose in
/// `$BASE`, so applying that transform to it would be a category error, and this module does not.
/// With zero offsets nothing is dropped — all three models then describe the same tool direction, and
/// the direction is all a robot pose needs. With non-zero offsets there is real geometry in the
/// parameter that the emitted program would not carry, so the program is refused instead.
fn require_zero_offsets(model: &Kinematics) -> Result<(), CodecError> {
    let (Kinematics::Ab {
        pivot_offset,
        rotary_offset,
    }
    | Kinematics::Ac {
        pivot_offset,
        rotary_offset,
    }
    | Kinematics::Bc {
        pivot_offset,
        rotary_offset,
    }) = model;
    if pivot_offset
        .iter()
        .chain(rotary_offset.iter())
        .any(|v| *v != 0.0)
    {
        return Err(CodecError::Other(format!(
            "KRL emit cannot carry the machine-tool offsets in {model:?}: an E6POS is a TCP pose in \
             $BASE, not a point in a rotated table frame. Move the offset into $BASE/$TOOL instead."
        )));
    }
    Ok(())
}

/// Emit a KRL program for a stream of segments.
///
/// Reached from [`super::emit_stream_to_writer`], which dispatches
/// [`super::FirmwareFlavor::RobotKrl`] here before any g-code word is formed. Shares that function's
/// partial-write contract: lines reach `writer` as they are produced, so a refusal raised mid-program
/// leaves a prefix behind — and a KRL prefix is missing its `END`, so it is not a loadable module.
pub(super) fn emit_krl_to_writer<I, W>(
    segments: I,
    p: &EmitParams,
    writer: &mut W,
) -> Result<(), CodecError>
where
    I: IntoIterator<Item = Result<crate::ir::Segment, CodecError>>,
    W: std::io::Write,
{
    let frame = &p.krl_frame;
    frame.validate().map_err(CodecError::Other)?;
    if p.five_axis {
        p.kinematics.validate().map_err(CodecError::Other)?;
        require_zero_offsets(&p.kinematics)?;
    }

    let segments = SplineFlatteningIterator::new(segments.into_iter());
    let mut first_line = true;
    let name = frame
        .program_name
        .as_deref()
        .unwrap_or(DEFAULT_PROGRAM_NAME);
    write_line(writer, &mut first_line, &format!("DEF {name} ( )"))?;
    for line in BANNER {
        write_line(writer, &mut first_line, line)?;
    }
    write_line(
        writer,
        &mut first_line,
        &format!("  $TOOL = {}", frame_literal(&frame.tool, "$TOOL")?),
    )?;
    write_line(
        writer,
        &mut first_line,
        &format!("  $BASE = {}", frame_literal(&frame.base, "$BASE")?),
    )?;
    // `$APO.CDIS` is *not* written here. See the loop below: it goes out lazily, at the first CP
    // instruction that will carry `C_DIS`, so a PTP-only or dwell-only program never states an
    // approximation distance nothing references (ADR 0002 §4).
    let approx = if frame.approx_mm.is_some() {
        " C_DIS"
    } else {
        ""
    };
    let mut apo_written = false;

    // Tracked exactly as the g-code renderer tracks them: `prog_pos` is the programmed point an
    // unstated axis inherits, `pos` is the last value actually written (a KRL aggregate omits what
    // has not changed, and the controller reads an omitted component as "keep the current one").
    let mut pos: [Option<f64>; 3] = [None, None, None];
    let mut prog_pos = [0.0f64; 3];
    let mut prev_abc: Option<[f64; 3]> = None;
    let mut prev_speed: Option<Feedrate> = None;
    let mut rotary_state = RotaryState::default();
    let letters = ['X', 'Y', 'Z'];

    for res in segments {
        let s = res?;
        if s.kind == SegmentKind::ManualGcode {
            // Not passed through. Every other flavor copies this field verbatim because every other
            // flavor *is* g-code, and the IR defines the field as verbatim g-code
            // (`spec/dry-ir-v0.schema.json`, `docs/10-dry-ir-v0-spec.md`). Copying g-code into a
            // DEF/END module produces a file that is neither, and nothing here can rewrite it —
            // which is precisely the "cannot faithfully represent" case ADR 0002 §4 says to refuse.
            return Err(CodecError::Other(
                "manualgcode segment cannot be emitted as KRL: the IR field is verbatim g-code, \
                 which is not a KRL statement, and copying it through would put g-code lines \
                 inside a DEF/END module. Remove the passthrough or emit a g-code flavor."
                    .to_string(),
            ));
        }

        if s.kind == SegmentKind::Dwell {
            if let Some(secs) = s.dwell_s {
                // Labelled "dwell", not "WAIT SEC": the refusal reads the same on every flavor,
                // which is what `tests/emit_rejects_unrepresentable.rs` checks across all of them.
                let secs_text = real(secs, "dwell")?;
                write_line(writer, &mut first_line, &format!("  WAIT SEC {secs_text}"))?;
            }
            continue;
        }

        // Every g-code flavor refuses a non-finite feedrate on a rapid as well as on a feed move,
        // because it writes an `F` word on both. KRL writes no speed at all ahead of a `PTP`
        // (`$VEL_AXIS[]` governs it), so without this the value would leave no trace and the gate
        // would be narrower here than everywhere else. `emit` is the last gate (ADR 0002 §4), so it
        // refuses the same IR the other flavors refuse.
        if !s.speed.value().is_finite() {
            return Err(CodecError::Other(format!(
                "cannot emit a motion segment with a feedrate of {} mm/min: the value is not finite",
                s.speed.value()
            )));
        }

        let mut start_prog = prog_pos;
        for (i, axis) in start_prog.iter_mut().enumerate() {
            if let Some(v) = s.start[i] {
                *axis = v.value();
            }
        }
        let mut end_prog = start_prog;
        for (i, axis) in end_prog.iter_mut().enumerate() {
            if let Some(v) = s.end[i] {
                *axis = v.value();
            }
        }
        prog_pos = end_prog;

        let is_arc = s.kind == SegmentKind::Arc && s.centre.is_some();
        if is_arc && (s.end[0].is_none() || s.end[1].is_none()) {
            return Err(CodecError::Other(
                "arc segment needs an explicit end X and Y: emitting one without them is a full \
                 360° circle, not a no-op"
                    .to_string(),
            ));
        }
        // Same predicate the g-code renderer uses to choose G0 over G1, so which moves are rapids
        // does not depend on the target.
        let has_e_word = !s.travel || s.filament != Length::ZERO;
        let is_ptp = !is_arc && s.travel && !p.travel_g1_e0 && !has_e_word;

        let abc = if p.five_axis {
            let joints = ORIENTATION_MODEL
                .resolve_joints(s.orientation, &mut rotary_state)
                .map_err(CodecError::Other)?;
            // `[0]` is the model's `C` word — the rotation about Z, which is KUKA's `A`; `[1]` is
            // its tilt from +Z, which is KUKA's `B`. See the module docs for why those are the same
            // two numbers.
            let words = ORIENTATION_MODEL.rotary_words(joints);
            Some([words[0].value, words[1].value, TOOL_ROLL_DEG])
        } else {
            None
        };

        let mut components: Vec<String> = Vec::new();
        for (i, &letter) in letters.iter().enumerate() {
            let explicit = s.end[i].is_some();
            let changed = pos[i].is_none_or(|v| v != end_prog[i]);
            let force = is_arc && i < 2;
            // Character for character the rule in `super::gcode`, so a component the g-code
            // renderer states is a component this one states. The five-axis branch is the one that
            // matters: `changed && !explicit` is an axis inheriting a *different* value from the
            // segment's own `start`, and dropping it walks the arm to the wrong place.
            let emit_axis = if p.five_axis {
                changed || explicit
            } else {
                explicit && (changed || force)
            };
            if emit_axis {
                components.push(format!("{letter} {}", real(end_prog[i], letter)?));
                pos[i] = Some(end_prog[i]);
            }
        }
        if let Some(abc) = abc {
            let prev = prev_abc.unwrap_or([f64::NAN; 3]);
            for (k, &letter) in ['A', 'B', 'C'].iter().enumerate() {
                if abc[k] != prev[k] {
                    components.push(format!("{letter} {}", real(abc[k], letter)?));
                }
            }
            prev_abc = Some(abc);
        }
        if components.is_empty() {
            // Nothing moved and nothing turned. G-code can say that with a bare `G1`, and an
            // aggregate restating the pose the robot is already at looked like the KRL equivalent —
            // it is not. It is a fabricated instruction: a zero-distance move, carrying `C_DIS`
            // when blending is on, that the IR never asked for. The two segments of
            // `conformance/vectors/retract_unretract` produced one each.
            //
            // The content such a segment does carry is filament or volume, and a KRL program has no
            // axis for either — a retract, an unretract and a stationary deposit are all
            // unrepresentable here, not merely unmoving. Refused rather than skipped: ADR 0002 §4
            // says do not silently emit nothing.
            return Err(CodecError::Other(
                "motion segment commands no pose a KRL instruction could state: every axis it \
                 names already holds the value it asks for, and filament and volume are not KRL \
                 quantities. A retract, an unretract or a stationary deposit has nothing left to \
                 emit on this target."
                    .to_string(),
            ));
        }

        let line = if is_arc {
            let aux = circ_auxiliary_point(&s, start_prog, end_prog)?;
            let aux_pose = pose_literal(&[
                format!("X {}", real(aux[0], 'X')?),
                format!("Y {}", real(aux[1], 'Y')?),
                format!("Z {}", real(aux[2], 'Z')?),
            ]);
            format!("  CIRC {aux_pose}, {}{approx}", pose_literal(&components))
        } else if is_ptp {
            // No approximation on a PTP: blending a joint move needs `C_PTP` with `$APO.CPTP` in
            // percent, which is a different quantity from the millimetres in `$APO.CDIS`.
            format!("  PTP {}", pose_literal(&components))
        } else {
            format!("  LIN {}{approx}", pose_literal(&components))
        };

        // Both modal lines go out only now, with the instruction that reads them in hand. `$VEL.CP`
        // governs CP motion only, so it never precedes a `PTP` whose speed it would not control;
        // `$APO.CDIS` precedes the first CP instruction and no other. Writing either earlier would
        // state machine state that the rest of the program might never reference — the vacuity
        // ADR 0002 §4 rules out, and the reason a segment that emits no instruction now leaves no
        // `$VEL.CP` behind either. The velocity comparison is against the last value *written*, so
        // a PTP between two equal-speed CP moves does not force a restatement.
        if !is_ptp {
            if let (Some(mm), false) = (frame.approx_mm, apo_written) {
                write_line(
                    writer,
                    &mut first_line,
                    &format!("  $APO.CDIS = {}", real(mm, "$APO.CDIS")?),
                )?;
                apo_written = true;
            }
            if prev_speed != Some(s.speed) {
                write_line(
                    writer,
                    &mut first_line,
                    &format!("  $VEL.CP = {}", vel_cp(s.speed.value())?),
                )?;
                prev_speed = Some(s.speed);
            }
        }
        write_line(writer, &mut first_line, &line)?;
    }

    write_line(writer, &mut first_line, "END")?;
    Ok(())
}

/// The auxiliary point of a `CIRC`: the point on the arc at half the swept angle.
///
/// Placing it at the midpoint is what carries the direction of travel — start → aux → end fixes a
/// unique arc, so a clockwise segment and its counter-clockwise twin no longer emit the same
/// instruction.
///
/// Refusals here are the arcs KRL's three-point form cannot express, not tolerances:
///
/// - a **helix** (`start.Z ≠ end.Z`), because a `CIRC` through three points is planar;
/// - a **full turn** (`end` = `start`), because the three points would be two distinct ones and the
///   circle underdetermined;
/// - a **zero radius**.
///
/// Not checked here: whether `|end − centre|` agrees with `|start − centre|`. That is a
/// tolerance-bearing question, and its epsilon is `verify::ARC_RADIUS_TOLERANCE_MM` — one
/// constant, applied by `resolve`'s L1 gate and by `verify`'s `arc-radius` rule, and published as
/// `FM1.F64.VERIFY.ARC_RADIUS` in `proofs/verify-numeric-boundaries-v0.toml`. Re-applying it here
/// would be a third application on a third input domain, which under ADR 0001 needs a boundary
/// entry of its own; this renderer defers instead.
///
/// **What the deferral does not buy.** `dry emit` never runs the verifier (ADR 0002 §1), so on the
/// emit path nothing checks this at all unless the caller also ran `dry verify`. An arc that fails
/// the rule emits an auxiliary point on the *start* radius, and the circle KUKA fits through the
/// three points is then not the circle the IR described. This is the target where that is
/// unrecoverable: RS-274 keeps the IR's own centre in `I`/`J`, so a downstream interpreter can still
/// see the contradiction, whereas a refitted three-point circle carries no trace of it. Recorded in
/// `docs/22-krl-emit.md` § "What this still does not do" rather than closed.
fn circ_auxiliary_point(
    s: &crate::ir::Segment,
    start_prog: [f64; 3],
    end_prog: [f64; 3],
) -> Result<[f64; 3], CodecError> {
    let [cx, cy] = s
        .centre
        .ok_or_else(|| CodecError::Other("arc segment is missing its centre".to_string()))?;
    let (cx, cy) = (cx.value(), cy.value());
    let [sx, sy, sz] = start_prog;
    let [ex, ey, ez] = end_prog;

    if ez != sz {
        return Err(CodecError::Other(format!(
            "arc segment rises from Z {sz} to Z {ez}: a KRL CIRC is the circle through three \
             points and cannot climb, so the helix cannot be emitted"
        )));
    }
    if ex == sx && ey == sy {
        return Err(CodecError::Other(
            "arc segment ends where it starts: a full turn has no distinct KRL auxiliary and end \
             point, so the circle it would describe is undetermined"
                .to_string(),
        ));
    }
    let radius = libm::hypot(sx - cx, sy - cy);
    if !(radius.is_finite() && radius > 0.0) {
        return Err(CodecError::Other(format!(
            "arc segment has a start radius of {radius} mm about ({cx}, {cy}): a KRL CIRC needs a \
             non-zero radius"
        )));
    }

    let theta_start = libm::atan2(sy - cy, sx - cx);
    let theta_end = libm::atan2(ey - cy, ex - cx);
    let theta_aux = theta_start + signed_sweep(theta_start, theta_end, s.clockwise) / 2.0;
    Ok([
        cx + radius * libm::cos(theta_aux),
        cy + radius * libm::sin(theta_aux),
        sz,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_names_that_kuka_would_reject_are_refused() {
        for bad in ["", "1st", "$tool", "with space", &"x".repeat(25)] {
            assert!(
                validate_program_name(bad).is_err(),
                "{bad:?} should be refused"
            );
        }
        for good in ["dry", "_a", "Part_1", &"x".repeat(24)] {
            assert!(
                validate_program_name(good).is_ok(),
                "{good:?} should be accepted"
            );
        }
    }

    #[test]
    fn vel_cp_converts_mm_per_min_to_metres_per_second() {
        assert_eq!(vel_cp(1200.0).unwrap(), "0.02");
        assert_eq!(vel_cp(60000.0).unwrap(), "1.0");
        // 100 mm/min is 1/600 m/s, which does not terminate: the published 12 decimals resolve it
        // to 1.6666666667e-3, i.e. to 6e-8 mm/min, below what the g-code side can print at all.
        assert_eq!(vel_cp(100.0).unwrap(), "0.001666666667");
        assert!(vel_cp(0.0).is_err());
        assert!(vel_cp(-1.0).is_err());
        assert!(vel_cp(f64::NAN).is_err());
    }

    /// A feedrate that survives the input guard but not the format is still refused, because
    /// `$VEL.CP = 0.0` is the line the guard exists to prevent and it is the *output* that reaches
    /// the controller.
    #[test]
    fn a_feedrate_that_rounds_to_zero_is_refused_not_printed_as_zero() {
        // Half an ulp of 1e-12 m/s is 5e-13 m/s, i.e. 3e-8 mm/min.
        for below in [1e-8, 2.9e-8, f64::MIN_POSITIVE] {
            let err = vel_cp(below).unwrap_err().to_string();
            assert!(err.contains("rounds to 0"), "{below}: {err}");
        }
        // Just above the boundary it still prints something non-zero.
        assert_eq!(vel_cp(6e-8).unwrap(), "0.000000000001");
    }

    #[test]
    fn reals_always_carry_a_decimal_point() {
        assert_eq!(real(10.0, 'X').unwrap(), "10.0");
        assert_eq!(real(-0.0, 'X').unwrap(), "0.0");
        assert_eq!(real(0.5, 'X').unwrap(), "0.5");
        assert!(real(f64::INFINITY, 'X').is_err());
    }

    /// The sweep is what puts the auxiliary point on the correct side of the chord, so the two
    /// directions must come out with opposite signs and equal magnitude.
    #[test]
    fn signed_sweep_separates_the_two_directions() {
        let quarter = std::f64::consts::FRAC_PI_2;
        assert!((signed_sweep(0.0, quarter, false) - quarter).abs() < 1e-15);
        assert!(
            (signed_sweep(0.0, quarter, true) - (quarter - std::f64::consts::TAU)).abs() < 1e-15
        );
        // A zero difference is a full turn in whichever direction was asked for.
        assert_eq!(signed_sweep(1.0, 1.0, false), std::f64::consts::TAU);
        assert_eq!(signed_sweep(1.0, 1.0, true), -std::f64::consts::TAU);
    }
}
