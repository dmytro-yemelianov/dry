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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    let mut e_abs = Length::ZERO;
    let letters = ['X', 'Y', 'Z'];

    let mut prog_pos = [0.0; 3];

    let frame = match (p.flavor, &p.cnc_frame) {
        (FirmwareFlavor::Rs274, Some(f)) => Some(*f),
        _ => None,
    };
    if let Some(f) = frame {
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
            write_line(writer, &mut first_line, &format!("S{} M3", num(rpm)))?;
        }
        if f.coolant == Some(true) {
            write_line(writer, &mut first_line, "M8")?;
        }
    }

    for res in segments {
        let s = res?;
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
                let cmd = match p.flavor {
                    FirmwareFlavor::Klipper => {
                        let ms = (secs * 1000.0).round() as u64;
                        format!("G4 P{ms}")
                    }
                    FirmwareFlavor::Rs274 | FirmwareFlavor::Marlin | FirmwareFlavor::Duet => {
                        format!("G4 S{}", num(secs))
                    }
                    FirmwareFlavor::Grbl => format!("G4 P{}", num(secs)),
                    FirmwareFlavor::RobotKrl => format!("WAIT {}", num(secs)),
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
                toks.push(format!("V{}", num(s.speed.value())));
            } else {
                toks.push(format!("F{}", num(s.speed.value())));
            }
            prev_speed = Some(s.speed);
        }

        // Determine target linear axes (in machine joint coordinates if five_axis is true).
        let target_axes = if p.five_axis {
            p.kinematics.machine_position(end_prog, s.orientation)
        } else {
            end_prog
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
                toks.push(format!("{letter}{}", num(target_axes[i])));
                pos[i] = Some(Length::mm(target_axes[i]));
            }
        }

        // 5-axis: emit the two rotary words (degrees) from the toolframe orientation under the chosen
        // kinematics, each only when it changes. In 3-axis mode the orientation is dropped entirely.
        if p.five_axis {
            let rotaries = p.kinematics.rotary_words(s.orientation);
            let prev = prev_rotary.unwrap_or([f64::NAN, f64::NAN]);
            for (r, &pv) in rotaries.iter().zip(prev.iter()) {
                if r.value != pv {
                    toks.push(format!("{}{}", r.letter, num(r.value)));
                }
            }
            prev_rotary = Some([rotaries[0].value, rotaries[1].value]);
        }

        if is_arc {
            let [cx_prog, cy_prog] = s.centre.unwrap();
            let [sx_prog, sy_prog, sz_prog] = start_prog;

            let (i_val, j_val) = if p.five_axis {
                // I/J is an incremental start→centre offset, so both points must be transformed
                // under the orientation the arc itself is executed at.
                let start_mcs = p.kinematics.machine_position(start_prog, s.orientation);
                let centre_mcs = p
                    .kinematics
                    .machine_position([cx_prog.value(), cy_prog.value(), sz_prog], s.orientation);
                (centre_mcs[0] - start_mcs[0], centre_mcs[1] - start_mcs[1])
            } else {
                (
                    (cx_prog - Length::mm(sx_prog)).value(),
                    (cy_prog - Length::mm(sy_prog)).value(),
                )
            };
            if p.flavor == FirmwareFlavor::RobotKrl {
                toks.push(format!("C{}", num(i_val)));
                toks.push(format!("D{}", num(j_val)));
            } else {
                toks.push(format!("I{}", num(i_val)));
                toks.push(format!("J{}", num(j_val)));
            }
        }

        if !p.flavor.has_extruder() {
            // CNC, laser and robot targets emit motion-only commands and have no filament axis.
        } else if p.relative_e {
            if has_e_word {
                toks.push(format!("E{}", num(s.filament.value())));
            } else if p.travel_g1_e0 {
                toks.push("E0".to_string());
            }
        } else {
            e_abs = e_abs + s.filament;
            if has_e_word || p.travel_g1_e0 {
                toks.push(format!("E{}", num(e_abs.value())));
            }
        }

        write_line(writer, &mut first_line, &toks.join(" "))?;
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
pub fn emit(tp: &Toolpath, p: &EmitParams) -> Vec<String> {
    emit_stream(tp.segments.iter().cloned().map(Ok), p).unwrap()
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
