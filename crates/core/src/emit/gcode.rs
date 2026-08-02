use super::kinematics::RotaryState;
use super::{Kinematics, SplineFlatteningIterator};
use crate::ir::{SegmentKind, Toolpath};
use crate::units::{Feedrate, Length};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FirmwareFlavor {
    #[default]
    Marlin,
    Klipper,
    Duet,
    /// CNC/RS-274 family (`ISO-6983`).
    Rs274,
    /// GRBL laser controller dialect.
    Grbl,
    /// Kuka Robot Language (KRL) style robot program output.
    RobotKrl,
}

impl FirmwareFlavor {
    /// Whether the target has a filament (E) axis. CNC, laser and robot controllers reject `E`.
    pub fn has_extruder(self) -> bool {
        matches!(
            self,
            FirmwareFlavor::Marlin | FirmwareFlavor::Klipper | FirmwareFlavor::Duet
        )
    }
}

/// How to emit.
#[derive(Debug, Clone, Deserialize)]
pub struct EmitParams {
    #[serde(default = "default_true")]
    pub relative_e: bool,
    #[serde(default)]
    pub travel_g1_e0: bool,
    /// Emit rotary words from the toolframe orientation (5-axis). Default off ⇒ 3-axis, the
    /// orientation is dropped and the motion g-code is byte-identical to the conformance oracle.
    #[serde(default)]
    pub five_axis: bool,
    /// Which rotary kinematics map the orientation onto words (default [`Kinematics::Ab`]).
    #[serde(default)]
    pub kinematics: Kinematics,
    /// Firmware/dialect flavor: marlin, klipper, duet, rs274, grbl, robot_krl.
    #[serde(default)]
    pub flavor: FirmwareFlavor,
    /// CNC work-coordinate/tool/spindle/coolant frame emitted ahead of motion by the RS-274 renderer
    /// (Task 5). Additive and optional: absent leaves existing g-code output byte-identical.
    ///
    /// **Invariant: the frame is emitted once per program, never per span.** Any path that emits a
    /// toolpath piecewise — the span-preserving g-code rewrite in [`crate::gcode`] — must clear this
    /// field, or every spliced span gets its own preamble and the per-span line accounting desyncs.
    #[serde(default)]
    pub cnc_frame: Option<CncFrame>,
}

/// CNC work-coordinate/tool/spindle/coolant preamble, sourced from `MachineProfile::cnc`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CncFrame {
    /// Work coordinate system, `54..=59` → `G54..G59`. `None` ⇒ default to G54.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wcs: Option<u8>,
    /// Tool number for `T<n> M6`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<u32>,
    /// Spindle speed in RPM for `S<rpm> M3`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spindle_rpm: Option<f64>,
    /// Flood coolant on/off (`M8`/`M9`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coolant: Option<bool>,
}

impl CncFrame {
    /// Validate the frame the way [`crate::profile::Profile::validate`] validates the `machine.cnc`
    /// fields it is built from.
    ///
    /// `EmitParams`/`CncFrame` are `pub` with `pub` fields and derive `Deserialize`, so a frame can
    /// reach the emitter without ever passing through profile validation. An out-of-range `wcs`
    /// renders as a bare `G0` where `G54` belongs — silently leaving the previous work offset
    /// active, i.e. cutting at the wrong origin — and a zero `spindle_rpm` renders `S0 M3`
    /// immediately before a cutting move.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(wcs) = self.wcs {
            if !(54..=59).contains(&wcs) {
                return Err(format!(
                    "cnc_frame.wcs must be 54..=59 (G54..G59), got {wcs}"
                ));
            }
        }
        if let Some(rpm) = self.spindle_rpm {
            if !(rpm.is_finite() && rpm > 0.0) {
                return Err(format!(
                    "cnc_frame.spindle_rpm must be finite and > 0, got {rpm}"
                ));
            }
        }
        Ok(())
    }
}

fn default_true() -> bool {
    true
}

impl Default for EmitParams {
    fn default() -> Self {
        EmitParams {
            relative_e: true,
            travel_g1_e0: false,
            five_axis: false,
            kinematics: Kinematics::default(),
            flavor: FirmwareFlavor::default(),
            cnc_frame: None,
        }
    }
}

/// Format a number as FullControl does: 6 decimals, trailing zeros + trailing `.` stripped, no `-0`.
pub(crate) fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Format a g-code word value, refusing anything the format cannot faithfully carry.
///
/// [`num`] is `format!("{v:.6}")` plus trimming, and Rust renders NaN as `NaN` and the infinities
/// as `inf`/`-inf` — so a non-finite quantity leaves here as a syntactically well-formed word with
/// a nonsense value (`G1 FNaN Xinf`). `emit` is the last gate before a machine (`dry emit` never
/// runs the verifier), so the fallible emit path refuses the program rather than writing it.
pub(crate) fn num_checked(
    v: f64,
    word: impl std::fmt::Display,
) -> Result<String, crate::codec::CodecError> {
    if !v.is_finite() {
        return Err(crate::codec::CodecError::Other(format!(
            "cannot emit non-finite {word} value ({v})"
        )));
    }
    Ok(num(v))
}

fn write_line<W: std::io::Write>(
    writer: &mut W,
    first: &mut bool,
    line: &str,
) -> Result<(), crate::codec::CodecError> {
    if !*first {
        writer
            .write_all(b"\n")
            .map_err(|e| crate::codec::CodecError::Other(e.to_string()))?;
    }
    writer
        .write_all(line.as_bytes())
        .map_err(|e| crate::codec::CodecError::Other(e.to_string()))?;
    *first = false;
    Ok(())
}

/// Emit motion g-code for a stream of segments to a writer without collecting every line first.
///
/// # Errors
///
/// Returns the first refusal or IO failure. **This is the one emit entry point that can write a
/// partial program**: unlike [`emit`] and [`emit_stream`], which buffer and hand back nothing on
/// error, lines here reach `writer` as they are produced, so a refusal raised mid-program (a
/// non-finite word, an endpointless arc) leaves everything before it already written. Under RS-274
/// that prefix is missing its `M9`/`M5`/`M30` postamble while still parsing as a valid program.
/// A caller streaming to a file must not leave the partial output where the program belongs —
/// write to a temporary path and rename only on `Ok`, or unlink on `Err`.
pub fn emit_stream_to_writer<I, W>(
    segments: I,
    p: &EmitParams,
    writer: &mut W,
) -> Result<(), crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<crate::ir::Segment, crate::codec::CodecError>>,
    W: std::io::Write,
{
    let segments = SplineFlatteningIterator::new(segments.into_iter());
    let mut first_line = true;
    let mut pos: [Option<Length>; 3] = [None, None, None];
    let mut prev_speed: Option<Feedrate> = None;
    let mut prev_rotary: Option<[f64; 2]> = None;
    // The power channel, modal exactly like the feedrate: `prev_power` is the last commanded `S`,
    // `spindle_on` whether an `M3` is outstanding. Only GRBL *renders* them — RS-274 already commands
    // the spindle once per program through [`CncFrame`], and interleaving a per-segment `S`/`M5` with
    // that preamble's own `S… M3`/`M5` is a collision this slice does not open; the printer flavors
    // have no spindle at all. Every other flavor therefore refuses a toolpath that carries the
    // channel instead of dropping it silently (see the segment loop below).
    let mut prev_power: Option<f64> = None;
    let mut spindle_on = false;
    let mut e_abs = Length::ZERO;
    let letters = ['X', 'Y', 'Z'];

    let mut prog_pos = [0.0; 3];
    // The C axis is history-dependent inside the singular cone; see `RotaryState`. Threaded exactly
    // like `prog_pos` above: advanced once per motion segment, untouched by dwells and manual g-code.
    let mut rotary_state = RotaryState::default();

    let frame = match (p.flavor, &p.cnc_frame) {
        (FirmwareFlavor::Rs274, Some(f)) => Some(*f),
        _ => None,
    };
    if p.five_axis {
        p.kinematics
            .validate()
            .map_err(crate::codec::CodecError::Other)?;
    }
    if let Some(f) = frame {
        f.validate().map_err(crate::codec::CodecError::Other)?;
        write_line(writer, &mut first_line, "G21 G17 G90")?;
        write_line(
            writer,
            &mut first_line,
            &format!("G{}", f.wcs.unwrap_or(54)),
        )?;
        if let Some(tool) = f.tool {
            write_line(writer, &mut first_line, &format!("T{tool} M6"))?;
        }
        if let Some(rpm) = f.spindle_rpm {
            write_line(
                writer,
                &mut first_line,
                &format!("S{} M3", num_checked(rpm, 'S')?),
            )?;
        }
        if f.coolant == Some(true) {
            write_line(writer, &mut first_line, "M8")?;
        }
    }

    for res in segments {
        let s = res?;

        // Power transitions are written ahead of whatever the segment is — a move, a dwell or a
        // verbatim block — because `resolve` attaches the running channel to every segment alike,
        // and a laser that changes state one segment late has already burnt the difference.
        //
        // `M3`, never `M4`. The same channel drives a laser and a CNC spindle, and `M4` means two
        // incompatible things to them: dynamic (feedrate-scaled) laser power to a GRBL controller in
        // laser mode, and *counter-clockwise rotation* to a spindle. `M3` is the one spelling that is
        // correct under both readings. Selecting dynamic power is a machine capability (GRBL `$32`),
        // so it belongs with the profile field, not here.
        //
        // `S0` is spelt `M5`, not `S0`: under `M3` a zero `S` leaves the laser *enabled* at zero
        // power, and "enabled" is the state that burns when the controller's next command misses.
        if let Some(level) = s.power {
            // The domain (finite, `>= 0` — `docs/10` §3.3 and `spec/dry-ir-v0.schema.json`) is
            // checked on *every* flavor, before the question of who renders it. Checking it inside
            // the GRBL arm would make the refusal flavor-conditional: a negative `S` reaching emit
            // through an IR file would then be silently dropped by every other target.
            if !level.is_finite() || level < 0.0 {
                return Err(crate::codec::CodecError::Other(format!(
                    "cannot emit spindle/laser power {level}: the channel must be finite and >= 0"
                )));
            }
            // Only GRBL has a rendering for the channel. Every other flavor refuses rather than
            // dropping a commanded machine state on the floor (ADR 0002 §4 — refuse, never emit
            // vacuously): a program that says "cut at S600" and emits g-code that says nothing at
            // all about the spindle is exactly the vacuous emission that rule forbids.
            if p.flavor != FirmwareFlavor::Grbl {
                return Err(crate::codec::CodecError::Other(format!(
                    "flavor {:?} cannot render the spindle/laser power channel (segment commands \
                     S{level}); {}",
                    p.flavor,
                    match p.flavor {
                        // RS-274 does drive a spindle, but through the *program* frame: the
                        // profile's `machine.cnc.spindle_rpm` writes one `S… M3` preamble and one
                        // `M5` postamble. A per-segment channel would interleave with those, so the
                        // two ways of commanding one spindle are kept mutually exclusive rather
                        // than merged by guesswork.
                        FirmwareFlavor::Rs274 =>
                            "RS-274 commands the spindle once per program through the profile's \
                             `machine.cnc` frame — set `spindle_rpm` there, or emit with `grbl`",
                        _ => "emit with `grbl`, or resolve a design without a `power` op",
                    }
                )));
            }
            if prev_power != Some(level) {
                let level_text = num_checked(level, 'S')?;
                if level > 0.0 {
                    let line = if spindle_on {
                        format!("S{level_text}")
                    } else {
                        spindle_on = true;
                        format!("S{level_text} M3")
                    };
                    write_line(writer, &mut first_line, &line)?;
                } else {
                    // A commanded zero is written even when no `M3` of ours is outstanding. The IR
                    // distinguishes "commanded off" (`Some(0.0)`) from "never commanded" (`None`),
                    // and that distinction only survives into g-code if the off is spelt out: the
                    // controller may well be live from a preceding program or a manual jog, and
                    // `M5` on an already-stopped spindle costs nothing.
                    spindle_on = false;
                    write_line(writer, &mut first_line, "M5")?;
                }
                prev_power = Some(level);
            }
        }

        if s.kind == SegmentKind::ManualGcode {
            if let Some(text) = &s.manual_gcode {
                for line in text.lines() {
                    write_line(writer, &mut first_line, line)?;
                }
            }
            continue;
        }

        // a dwell is a pause in the motion stream, not a move: emit dialect-specific dwell command and carry on (it
        // does not touch the running position or feedrate).
        if s.kind == SegmentKind::Dwell {
            if let Some(secs) = s.dwell_s {
                // reject up front: the Klipper branch casts, and `NaN as u64` saturates to 0 rather
                // than carrying the non-finite value into the word check below.
                let secs_text = num_checked(secs, "dwell")?;
                let cmd = match p.flavor {
                    FirmwareFlavor::Klipper => {
                        let ms = (secs * 1000.0).round() as u64;
                        format!("G4 P{ms}")
                    }
                    FirmwareFlavor::Rs274 | FirmwareFlavor::Marlin | FirmwareFlavor::Duet => {
                        format!("G4 S{secs_text}")
                    }
                    FirmwareFlavor::Grbl => format!("G4 P{secs_text}"),
                    FirmwareFlavor::RobotKrl => format!("WAIT {secs_text}"),
                };
                write_line(writer, &mut first_line, &cmd)?;
            }
            continue;
        }

        // Track programmed coordinates.
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
        // An arc word list without an endpoint is not a no-op: RS-274 reads `G3 I-10 J0` as a full
        // 360° circle. The X/Y words below are only forced when the endpoint is explicit, so refuse
        // the segment here — the importer refuses the same construct (`gcode::lift::arc_geometry`),
        // which is why round-trip coverage never sees it.
        if is_arc && (s.end[0].is_none() || s.end[1].is_none()) {
            return Err(crate::codec::CodecError::Other(
                "arc segment needs an explicit end X and Y: emitting one without them is a full \
                 360° circle, not a no-op"
                    .to_string(),
            ));
        }
        let has_e_word = !s.travel || s.filament != Length::ZERO;
        let is_robot = p.flavor == FirmwareFlavor::RobotKrl;
        let cmd = if is_robot {
            if is_arc {
                "CIRC"
            } else if s.travel && !p.travel_g1_e0 && !has_e_word {
                "PTP"
            } else {
                "LIN"
            }
        } else if is_arc {
            if s.clockwise {
                "G2"
            } else {
                "G3"
            }
        } else if s.travel && !p.travel_g1_e0 && !has_e_word {
            "G0"
        } else {
            "G1"
        };
        let mut toks = vec![cmd.to_string()];

        if prev_speed != Some(s.speed) {
            if is_robot {
                toks.push(format!("V{}", num_checked(s.speed.value(), 'V')?));
            } else {
                toks.push(format!("F{}", num_checked(s.speed.value(), 'F')?));
            }
            prev_speed = Some(s.speed);
        }

        // 5-axis: resolve this segment's rotary joints once, advancing the C-axis state. The linear
        // words below, the rotary words further down and the arc offsets all read these same angles
        // — a held C that reached only one of them would describe two different machine states on
        // one line.
        let joints = if p.five_axis {
            Some(
                p.kinematics
                    .resolve_joints(s.orientation, &mut rotary_state)
                    .map_err(crate::codec::CodecError::Other)?,
            )
        } else {
            None
        };

        // Determine target linear axes (in machine joint coordinates if five_axis is true).
        let target_axes = match joints {
            Some(joints) => p.kinematics.machine_position(end_prog, joints),
            None => end_prog,
        };

        for (i, &letter) in letters.iter().enumerate() {
            let explicit = s.end[i].is_some();
            let changed = pos[i].is_none_or(|v| v.value() != target_axes[i]);
            let force = is_arc && i < 2;
            let emit_axis = if p.five_axis {
                changed || explicit
            } else {
                explicit && (changed || force)
            };

            if emit_axis {
                toks.push(format!("{letter}{}", num_checked(target_axes[i], letter)?));
                pos[i] = Some(Length::mm(target_axes[i]));
            }
        }

        // 5-axis: emit the two rotary words (degrees) from the toolframe orientation under the chosen
        // kinematics, each only when it changes. In 3-axis mode the orientation is dropped entirely.
        if let Some(joints) = joints {
            let rotaries = p.kinematics.rotary_words(joints);
            let prev = prev_rotary.unwrap_or([f64::NAN, f64::NAN]);
            for (r, &pv) in rotaries.iter().zip(prev.iter()) {
                if r.value != pv {
                    toks.push(format!("{}{}", r.letter, num_checked(r.value, r.letter)?));
                }
            }
            prev_rotary = Some([rotaries[0].value, rotaries[1].value]);
        }

        if is_arc {
            let [cx_prog, cy_prog] = s.centre.unwrap();
            let [sx_prog, sy_prog, sz_prog] = start_prog;

            let (i_val, j_val) = if let Some(joints) = joints {
                // I/J is an incremental start→centre offset, so both points must be transformed
                // under the orientation the arc itself is executed at.
                let start_mcs = p.kinematics.machine_position(start_prog, joints);
                let centre_mcs = p
                    .kinematics
                    .machine_position([cx_prog.value(), cy_prog.value(), sz_prog], joints);
                (centre_mcs[0] - start_mcs[0], centre_mcs[1] - start_mcs[1])
            } else {
                (
                    (cx_prog - Length::mm(sx_prog)).value(),
                    (cy_prog - Length::mm(sy_prog)).value(),
                )
            };
            if p.flavor == FirmwareFlavor::RobotKrl {
                toks.push(format!("C{}", num_checked(i_val, 'C')?));
                toks.push(format!("D{}", num_checked(j_val, 'D')?));
            } else {
                toks.push(format!("I{}", num_checked(i_val, 'I')?));
                toks.push(format!("J{}", num_checked(j_val, 'J')?));
            }
        }

        if !p.flavor.has_extruder() {
            // CNC, laser and robot targets emit motion-only commands and have no filament axis.
        } else if p.relative_e {
            if has_e_word {
                toks.push(format!("E{}", num_checked(s.filament.value(), 'E')?));
            } else if p.travel_g1_e0 {
                toks.push("E0".to_string());
            }
        } else {
            e_abs = e_abs + s.filament;
            if has_e_word || p.travel_g1_e0 {
                toks.push(format!("E{}", num_checked(e_abs.value(), 'E')?));
            }
        }

        write_line(writer, &mut first_line, &toks.join(" "))?;
    }

    // A program must not end with the beam or the spindle still live. `spindle_on` is only ever set
    // by the GRBL branch above, so this fires exactly when this emitter turned it on.
    if spindle_on {
        write_line(writer, &mut first_line, "M5")?;
    }

    if let Some(f) = frame {
        if f.coolant == Some(true) {
            write_line(writer, &mut first_line, "M9")?;
        }
        if f.spindle_rpm.is_some() {
            write_line(writer, &mut first_line, "M5")?;
        }
        write_line(writer, &mut first_line, "M30")?;
    }

    Ok(())
}

/// Emit motion g-code lines for a stream of segments.
pub fn emit_stream<I>(segments: I, p: &EmitParams) -> Result<Vec<String>, crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<crate::ir::Segment, crate::codec::CodecError>>,
{
    let mut buf = Vec::new();
    emit_stream_to_writer(segments, p, &mut buf)?;
    let text =
        String::from_utf8(buf).map_err(|e| crate::codec::CodecError::Other(e.to_string()))?;
    Ok(if text.is_empty() {
        Vec::new()
    } else {
        text.split('\n').map(str::to_owned).collect()
    })
}

/// Emit motion g-code lines for a toolpath.
///
/// **Deprecated in favour of [`emit_stream`]**, which reports what this cannot. It survives only as
/// a transitional guard for the in-tree call sites that build their own IR and so cannot violate
/// the precondition below; every caller handling IR it did not construct must use [`emit_stream`].
///
/// **Precondition: `tp` carries only finite quantities and `p` is a valid emit configuration**
/// (unit toolframe orientations under a five-axis model, an in-range [`CncFrame`], an explicit
/// endpoint on every arc). This entry point is infallible, so it has no way to report the rejection
/// that [`emit_stream`] and [`emit_stream_to_writer`] perform.
///
/// On a violated precondition it **refuses the program**: debug builds panic on the
/// `debug_assert`, release builds return no lines at all. It never emits a partial or
/// nonsense-valued program, because a syntactically well-formed word carrying `NaN`/`inf` or a
/// wrong-origin `G0` is the one output that reaches metal unchallenged.
///
/// Callers handling untrusted IR must use [`emit_stream`] and surface the error.
#[deprecated(note = "use emit_stream; emit() cannot report a refused program")]
pub fn emit(tp: &Toolpath, p: &EmitParams) -> Vec<String> {
    match emit_stream(tp.segments.iter().cloned().map(Ok), p) {
        Ok(lines) => lines,
        Err(e) => {
            debug_assert!(false, "emit() precondition violated: {e}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_params_json_without_cnc_frame_deserializes() {
        let p: EmitParams = serde_json::from_str(r#"{"relative_e":true}"#).unwrap();
        assert!(p.cnc_frame.is_none());
        assert!(EmitParams::default().cnc_frame.is_none());
    }
}
