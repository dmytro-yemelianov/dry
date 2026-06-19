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

/// How to emit (Marlin flavour for now). Unknown fields (e.g. `flavor`) are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct EmitParams {
    #[serde(default = "default_true")]
    pub relative_e: bool,
    #[serde(default)]
    pub travel_g1_e0: bool,
    /// Emit rotary `A`/`B` words from the toolframe orientation (5-axis). Default off ⇒ 3-axis, the
    /// orientation is dropped and the motion g-code is byte-identical to the conformance oracle.
    #[serde(default)]
    pub five_axis: bool,
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
        }
    }
}

/// Map a toolframe orientation (tool-direction unit vector) to rotary `(A, B)` angles in **degrees**
/// for an AB-head: `B = atan2(i, k)` (lead in the X-Z plane), `A = atan2(j, hypot(i, k))` (tilt toward
/// Y). `None` ⇒ identity (+Z) ⇒ `(0, 0)`.
fn tool_angles(orientation: Option<[f64; 3]>) -> (f64, f64) {
    let [i, j, k] = orientation.unwrap_or([0.0, 0.0, 1.0]);
    let b = i.atan2(k).to_degrees();
    let a = j.atan2((i * i + k * k).sqrt()).to_degrees();
    (a, b)
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

/// Emit motion g-code lines for a toolpath.
pub fn emit(tp: &Toolpath, p: &EmitParams) -> Vec<String> {
    let mut out = Vec::with_capacity(tp.segments.len());
    let mut pos: [Option<Length>; 3] = [None, None, None];
    let mut prev_speed: Option<Feedrate> = None;
    let mut prev_ab: Option<(f64, f64)> = None;
    let mut e_abs = Length::ZERO;
    let letters = ['X', 'Y', 'Z'];

    for s in &tp.segments {
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

        // 5-axis: emit the rotary A/B (degrees) from the toolframe orientation, each only when it
        // changes. In 3-axis mode the orientation is dropped entirely.
        if p.five_axis {
            let (a, b) = tool_angles(s.orientation);
            let (pa, pb) = prev_ab.unwrap_or((f64::NAN, f64::NAN));
            if a != pa {
                toks.push(format!("A{}", num(a)));
            }
            if b != pb {
                toks.push(format!("B{}", num(b)));
            }
            prev_ab = Some((a, b));
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
}
