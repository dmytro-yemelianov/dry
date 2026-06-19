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
/// and how the tool-direction unit vector maps onto them. Default [`Kinematics::Ab`] (a tilting head)
/// reproduces the historical AB mapping byte-for-byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kinematics {
    /// Tilting head: `A` about X then `B` about Y. Words `A`,`B`.
    #[default]
    Ab,
    /// `A` about X, `C` about Z (e.g. table/trunnion). Words `A`,`C`.
    Ac,
    /// `B` about Y, `C` about Z. Words `B`,`C`.
    Bc,
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
            kinematics: Kinematics::Ab,
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
        Kinematics::Ab => {
            let a = j.atan2((i * i + k * k).sqrt()).to_degrees();
            let b = i.atan2(k).to_degrees();
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
        Kinematics::Ac => {
            let c = j.atan2(i).to_degrees();
            let a = k.clamp(-1.0, 1.0).acos().to_degrees();
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
        Kinematics::Bc => {
            let c = j.atan2(i).to_degrees();
            let b = k.clamp(-1.0, 1.0).acos().to_degrees();
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

/// Emit motion g-code lines for a stream of segments.
pub fn emit_stream<I>(segments: I, p: &EmitParams) -> Result<Vec<String>, crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<crate::ir::Segment, crate::codec::CodecError>>,
{
    let mut out = Vec::new();
    let mut pos: [Option<Length>; 3] = [None, None, None];
    let mut prev_speed: Option<Feedrate> = None;
    let mut prev_rotary: Option<[f64; 2]> = None;
    let mut e_abs = Length::ZERO;
    let letters = ['X', 'Y', 'Z'];

    for res in segments {
        let s = res?;
        // a dwell is a pause in the motion stream, not a move: emit `G4 S<seconds>` and carry on (it
        // does not touch the running position or feedrate).
        if s.kind == "dwell" {
            if let Some(secs) = s.dwell_s {
                out.push(format!("G4 S{}", num(secs)));
            }
            continue;
        }
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

        // arc I/J is the centre offset from the move's START (the position before this move).
        let arc_start = [pos[0], pos[1]];
        for (i, &letter) in letters.iter().enumerate() {
            if let Some(v) = s.end[i] {
                // arcs always state the end X and Y (the plane end point); Z, and all line axes, are
                // emitted only when they change.
                let force = is_arc && i < 2;
                if force || pos[i] != Some(v) {
                    toks.push(format!("{letter}{}", num(v.value())));
                }
                pos[i] = Some(v);
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
            let [cx, cy] = s.centre.unwrap();
            let sx = arc_start[0].unwrap_or(Length::ZERO);
            let sy = arc_start[1].unwrap_or(Length::ZERO);
            toks.push(format!("I{}", num((cx - sx).value())));
            toks.push(format!("J{}", num((cy - sy).value())));
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

        out.push(toks.join(" "));
    }
    Ok(out)
}

/// Emit motion g-code lines for a toolpath.
pub fn emit(tp: &Toolpath, p: &EmitParams) -> Vec<String> {
    emit_stream(tp.segments.iter().cloned().map(Ok), p).unwrap()
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
