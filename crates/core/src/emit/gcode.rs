use super::kinematics::tool_rotaries;
use super::{Kinematics, SplineFlatteningIterator};
use crate::ir::{SegmentKind, Toolpath};
use crate::units::{Feedrate, Length};
use serde::Deserialize;

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
    /// Firmware/dialect flavor: marlin, klipper, duet, rs274, grbl.
    #[serde(default)]
    pub flavor: FirmwareFlavor,
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

/// Helper to translate a 3D point from Workpiece Coordinate System (WCS) to Machine
/// Coordinate System (MCS) using the configured 5-axis kinematics and offsets.
fn to_mcs(p: [f64; 3], orientation: Option<[f64; 3]>, kinematics: Kinematics) -> [f64; 3] {
    let [i, j, k] = orientation.unwrap_or([0.0, 0.0, 1.0]);
    match kinematics {
        Kinematics::Ab {
            pivot_offset,
            rotary_offset,
        } => {
            let a_nom = libm::atan2(j, libm::hypot(i, k));
            let b_nom = libm::atan2(i, k);
            let a = a_nom + rotary_offset[0].to_radians();
            let b = b_nom + rotary_offset[1].to_radians();

            let sa = libm::sin(a);
            let ca = libm::cos(a);
            let sb = libm::sin(b);
            let cb = libm::cos(b);

            // R = R_y(b) * R_x(a)
            let lx = pivot_offset[0];
            let ly = pivot_offset[1];
            let lz = pivot_offset[2];

            let rx = cb * lx - sb * sa * ly + sb * ca * lz;
            let ry = ca * ly + sa * lz;
            let rz = -sb * lx - cb * sa * ly + cb * ca * lz;

            [p[0] - rx, p[1] - ry, p[2] - rz]
        }
        Kinematics::Ac {
            pivot_offset,
            rotary_offset,
        } => {
            let c_nom = libm::atan2(j, i);
            let a_nom = libm::acos(k.clamp(-1.0, 1.0));
            let a = a_nom + rotary_offset[0].to_radians();
            let c = c_nom + rotary_offset[1].to_radians();

            let sa = libm::sin(a);
            let ca = libm::cos(a);
            let sc = libm::sin(c);
            let cc = libm::cos(c);

            // R_table = R_x(a) * R_z(c)
            let lx = pivot_offset[0];
            let ly = pivot_offset[1];
            let lz = pivot_offset[2];

            let px = p[0] + lx;
            let py = p[1] + ly;
            let pz = p[2] + lz;

            let rx = cc * px - sc * py;
            let ry = ca * sc * px + ca * cc * py - sa * pz;
            let rz = sa * sc * px + sa * cc * py + ca * pz;

            [rx - lx, ry - ly, rz - lz]
        }
        Kinematics::Bc {
            pivot_offset,
            rotary_offset,
        } => {
            let c_nom = libm::atan2(j, i);
            let b_nom = libm::acos(k.clamp(-1.0, 1.0));
            let b = b_nom + rotary_offset[0].to_radians();
            let c = c_nom + rotary_offset[1].to_radians();

            let sb = libm::sin(b);
            let cb = libm::cos(b);
            let sc = libm::sin(c);
            let cc = libm::cos(c);

            // R_table = R_y(b) * R_z(c)
            let lx = pivot_offset[0];
            let ly = pivot_offset[1];
            let lz = pivot_offset[2];

            let px = p[0] + lx;
            let py = p[1] + ly;
            let pz = p[2] + lz;

            let rx = cb * cc * px - cb * sc * py + sb * pz;
            let ry = sc * px + cc * py;
            let rz = -sb * cc * px + sb * sc * py + cb * pz;

            [rx - lx, ry - ly, rz - lz]
        }
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
    let mut prev_orientation: Option<[f64; 3]> = None;

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
        let cmd = if is_arc {
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
            toks.push(format!("F{}", num(s.speed.value())));
            prev_speed = Some(s.speed);
        }

        // Determine target linear axes (in machine joint coordinates if five_axis is true).
        let target_axes = if p.five_axis {
            to_mcs(end_prog, s.orientation, p.kinematics)
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
            let rotaries = tool_rotaries(s.orientation, p.kinematics);
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
                let start_mcs = to_mcs(start_prog, prev_orientation, p.kinematics);
                let centre_mcs = to_mcs(
                    [cx_prog.value(), cy_prog.value(), sz_prog],
                    s.orientation,
                    p.kinematics,
                );
                (centre_mcs[0] - start_mcs[0], centre_mcs[1] - start_mcs[1])
            } else {
                (
                    (cx_prog - Length::mm(sx_prog)).value(),
                    (cy_prog - Length::mm(sy_prog)).value(),
                )
            };
            toks.push(format!("I{}", num(i_val)));
            toks.push(format!("J{}", num(j_val)));
        }

        if p.relative_e {
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

        prev_orientation = s.orientation;
        write_line(writer, &mut first_line, &toks.join(" "))?;
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
