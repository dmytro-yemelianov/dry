//! `emit` — lower an L2 [`Toolpath`] to motion g-code (Marlin), reproducing FullControl's bytes
//! (`docs/03-conformance.md`, the strictest gate). Clean-room: the formatting rules below are Dry's
//! independent reimplementation of FullControl's *observed* output, not its code.
//!
//! Rules (per move): `G1` when extruding, `G0` when travelling; `F<speed>` only when the feedrate
//! changes; an axis `X`/`Y`/`Z` only when it changes; in relative-E mode the extruding move carries
//! `E<filament>` (a travel carries none, unless `travel_g1_e0`). Numbers are `{:.6}` with trailing
//! zeros and a trailing `.` stripped (so `1000.000000`→`1000`, `0.200000`→`0.2`, `0`→`0`).

use crate::ir::Toolpath;
use crate::units::{Feedrate, Length};
use serde::Deserialize;

/// The rotary kinematics of the 5-axis machine: which two rotary axes carry the toolframe orientation,
/// and how the tool-direction unit vector maps onto them. Supports mechanical TCP (Tool Center Point)
/// translation offsets and rotary joint rotation offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kinematics {
    /// Tilting head: `A` about X then `B` about Y. Words `A`,`B`.
    Ab {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
    /// `A` about X, `C` about Z (e.g. table/trunnion). Words `A`,`C`.
    Ac {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
    /// `B` about Y, `C` about Z. Words `B`,`C`.
    Bc {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
}

impl Default for Kinematics {
    fn default() -> Self {
        Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        }
    }
}

impl<'de> serde::Deserialize<'de> for Kinematics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawKinematics {
            String(String),
            Struct(RawKinematicsStruct),
        }

        #[derive(Deserialize)]
        struct RawKinematicsStruct {
            #[serde(rename = "type")]
            kind: String,
            #[serde(default)]
            pivot_offset: [f64; 3],
            #[serde(default)]
            rotary_offset: [f64; 2],
        }

        let raw = RawKinematics::deserialize(deserializer)?;
        match raw {
            RawKinematics::String(s) => match s.as_str() {
                "ab" => Ok(Kinematics::Ab {
                    pivot_offset: [0.0, 0.0, 0.0],
                    rotary_offset: [0.0, 0.0],
                }),
                "ac" => Ok(Kinematics::Ac {
                    pivot_offset: [0.0, 0.0, 0.0],
                    rotary_offset: [0.0, 0.0],
                }),
                "bc" => Ok(Kinematics::Bc {
                    pivot_offset: [0.0, 0.0, 0.0],
                    rotary_offset: [0.0, 0.0],
                }),
                other => Err(D::Error::custom(format!("unknown kinematics: {}", other))),
            },
            RawKinematics::Struct(s) => match s.kind.as_str() {
                "ab" => Ok(Kinematics::Ab {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                "ac" => Ok(Kinematics::Ac {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                "bc" => Ok(Kinematics::Bc {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                other => Err(D::Error::custom(format!("unknown kinematics type: {}", other))),
            },
        }
    }
}

/// How to emit (Marlin flavour for now). Unknown fields (e.g. `flavor`) are ignored.
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
        }
    }
}

/// One emitted rotary word: its letter and its value in **degrees**.
struct Rotary {
    letter: char,
    value: f64,
}

/// Map a toolframe orientation (tool-direction unit vector) to the two rotary words for `kinematics`,
/// in source order. `None` ⇒ identity (+Z) ⇒ all-zero angles. Conventions (each documented on
/// [`Kinematics`]):
///
/// - **AB**: `B = atan2(i, k)` (lead in X-Z), `A = atan2(j, hypot(i, k))` (tilt toward Y).
/// - **AC**: `C = atan2(j, i)` (azimuth about Z), `A = acos(k)` (polar tilt from +Z).
/// - **BC**: `C = atan2(j, i)`, `B = acos(k)`.
///
/// `+Z` gives `atan2(0, 0) = 0` and `acos(1) = 0`, so every convention yields zeros there.
fn tool_rotaries(orientation: Option<[f64; 3]>, kinematics: Kinematics) -> [Rotary; 2] {
    let [i, j, k] = orientation.unwrap_or([0.0, 0.0, 1.0]);
    match kinematics {
        Kinematics::Ab { pivot_offset: _, rotary_offset } => {
            let a = j.atan2((i * i + k * k).sqrt()).to_degrees() + rotary_offset[0];
            let b = i.atan2(k).to_degrees() + rotary_offset[1];
            [
                Rotary {
                    letter: 'A',
                    value: a,
                },
                Rotary {
                    letter: 'B',
                    value: b,
                },
            ]
        }
        Kinematics::Ac { pivot_offset: _, rotary_offset } => {
            let c = j.atan2(i).to_degrees() + rotary_offset[1];
            let a = k.clamp(-1.0, 1.0).acos().to_degrees() + rotary_offset[0];
            [
                Rotary {
                    letter: 'C',
                    value: c,
                },
                Rotary {
                    letter: 'A',
                    value: a,
                },
            ]
        }
        Kinematics::Bc { pivot_offset: _, rotary_offset } => {
            let c = j.atan2(i).to_degrees() + rotary_offset[1];
            let b = k.clamp(-1.0, 1.0).acos().to_degrees() + rotary_offset[0];
            [
                Rotary {
                    letter: 'C',
                    value: c,
                },
                Rotary {
                    letter: 'B',
                    value: b,
                },
            ]
        }
    }
}

/// Format a number as FullControl does: 6 decimals, trailing zeros + trailing `.` stripped, no `-0`.
fn num(v: f64) -> String {
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
fn to_mcs(
    p: [f64; 3],
    orientation: Option<[f64; 3]>,
    kinematics: Kinematics,
) -> [f64; 3] {
    let [i, j, k] = orientation.unwrap_or([0.0, 0.0, 1.0]);
    match kinematics {
        Kinematics::Ab { pivot_offset, rotary_offset } => {
            let a_nom = j.atan2((i * i + k * k).sqrt());
            let b_nom = i.atan2(k);
            let a = a_nom + rotary_offset[0].to_radians();
            let b = b_nom + rotary_offset[1].to_radians();

            let (sa, ca) = a.sin_cos();
            let (sb, cb) = b.sin_cos();

            // R = R_y(b) * R_x(a)
            let lx = pivot_offset[0];
            let ly = pivot_offset[1];
            let lz = pivot_offset[2];

            let rx = cb * lx - sb * sa * ly + sb * ca * lz;
            let ry = ca * ly + sa * lz;
            let rz = -sb * lx - cb * sa * ly + cb * ca * lz;

            [
                p[0] - rx,
                p[1] - ry,
                p[2] - rz,
            ]
        }
        Kinematics::Ac { pivot_offset, rotary_offset } => {
            let c_nom = j.atan2(i);
            let a_nom = k.clamp(-1.0, 1.0).acos();
            let a = a_nom + rotary_offset[0].to_radians();
            let c = c_nom + rotary_offset[1].to_radians();

            let (sa, ca) = a.sin_cos();
            let (sc, cc) = c.sin_cos();

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

            [
                rx - lx,
                ry - ly,
                rz - lz,
            ]
        }
        Kinematics::Bc { pivot_offset, rotary_offset } => {
            let c_nom = j.atan2(i);
            let b_nom = k.clamp(-1.0, 1.0).acos();
            let b = b_nom + rotary_offset[0].to_radians();
            let c = c_nom + rotary_offset[1].to_radians();

            let (sb, cb) = b.sin_cos();
            let (sc, cc) = c.sin_cos();

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

            [
                rx - lx,
                ry - ly,
                rz - lz,
            ]
        }
    }
}

/// Emit motion g-code lines for a toolpath.
pub fn emit(tp: &Toolpath, p: &EmitParams) -> Vec<String> {
    let mut out = Vec::with_capacity(tp.segments.len());
    let mut pos: [Option<Length>; 3] = [None, None, None];
    let mut prev_speed: Option<Feedrate> = None;
    let mut prev_rotary: Option<[f64; 2]> = None;
    let mut e_abs = Length::ZERO;
    let letters = ['X', 'Y', 'Z'];

    let mut prog_pos = [0.0; 3];
    let mut prev_orientation: Option<[f64; 3]> = None;

    for s in &tp.segments {
        // a dwell is a pause in the motion stream, not a move: emit `G4 S<seconds>` and carry on (it
        // does not touch the running position or feedrate).
        if s.kind == "dwell" {
            if let Some(secs) = s.dwell_s {
                out.push(format!("G4 S{}", num(secs)));
            }
            continue;
        }

        // Track programmed coordinates.
        let mut start_prog = prog_pos;
        for i in 0..3 {
            if let Some(v) = s.start[i] {
                start_prog[i] = v.value();
            }
        }
        let mut end_prog = start_prog;
        for i in 0..3 {
            if let Some(v) = s.end[i] {
                end_prog[i] = v.value();
            }
        }
        prog_pos = end_prog;

        let is_arc = s.kind == "arc" && s.centre.is_some();
        let cmd = if s.travel {
            "G0"
        } else if is_arc {
            if s.clockwise {
                "G2"
            } else {
                "G3"
            }
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
            let changed = pos[i].map_or(true, |v| (v.value() - target_axes[i]).abs() > 1e-9);
            let force = is_arc && i < 2;

            if (p.five_axis && (changed || explicit)) || (!p.five_axis && explicit && (changed || force)) {
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
                let centre_mcs = to_mcs([cx_prog.value(), cy_prog.value(), sz_prog], s.orientation, p.kinematics);
                (centre_mcs[0] - start_mcs[0], centre_mcs[1] - start_mcs[1])
            } else {
                ((cx_prog - Length::mm(sx_prog)).value(), (cy_prog - Length::mm(sy_prog)).value())
            };
            toks.push(format!("I{}", num(i_val)));
            toks.push(format!("J{}", num(j_val)));
        }

        if p.relative_e {
            if !s.travel {
                toks.push(format!("E{}", num(s.filament.value())));
            } else if p.travel_g1_e0 {
                toks.push("E0".to_string());
            }
        } else {
            e_abs = e_abs + s.filament;
            if !s.travel || p.travel_g1_e0 {
                toks.push(format!("E{}", num(e_abs.value())));
            }
        }

        prev_orientation = s.orientation;
        out.push(toks.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::num;

    #[test]
    fn number_format_matches_fullcontrol() {
        assert_eq!(num(1000.0), "1000");
        assert_eq!(num(0.2), "0.2");
        assert_eq!(num(0.0), "0");
        assert_eq!(num(0.498902), "0.498902");
        assert_eq!(num(10.0), "10");
        assert_eq!(num(-1.5), "-1.5");
    }

    #[test]
    fn test_travel_g1_e0() {
        use crate::ir::{Segment, Toolpath};
        use crate::units::{Feedrate, Length, Volume};
        use super::{emit, EmitParams};

        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![
                Segment {
                    start: [None, None, None],
                    end: [Some(Length::mm(10.0)), None, None],
                    travel: true,
                    speed: Feedrate(1000.0),
                    length: Length::mm(10.0),
                    volume: Volume::ZERO,
                    filament: Length::ZERO,
                    width: None,
                    height: None,
                    kind: "line".to_string(),
                    centre: None,
                    clockwise: false,
                    temperature: None,
                    fan: None,
                    flow: None,
                    tool: None,
                    dwell_s: None,
                    orientation: None,
                }
            ],
        };

        let gcode_default = emit(&tp, &EmitParams::default());
        assert_eq!(gcode_default[0], "G0 F1000 X10");

        let gcode_e0 = emit(&tp, &EmitParams {
            travel_g1_e0: true,
            ..EmitParams::default()
        });
        assert_eq!(gcode_e0[0], "G0 F1000 X10 E0");

        let gcode_abs_e0 = emit(&tp, &EmitParams {
            relative_e: false,
            travel_g1_e0: true,
            ..EmitParams::default()
        });
        assert_eq!(gcode_abs_e0[0], "G0 F1000 X10 E0");
    }
}
