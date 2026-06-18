//! `emit` — lower an L2 [`Toolpath`] to motion g-code (Marlin), reproducing FullControl's bytes
//! (`docs/03-conformance.md`, the strictest gate). Clean-room: the formatting rules below are Dry's
//! independent reimplementation of FullControl's *observed* output, not its code.
//!
//! Rules (per move): `G1` when extruding, `G0` when travelling; `F<speed>` only when the feedrate
//! changes; an axis `X`/`Y`/`Z` only when it changes; in relative-E mode the extruding move carries
//! `E<filament>` (a travel carries none, unless `travel_g1_e0`). Numbers are `{:.6}` with trailing
//! zeros and a trailing `.` stripped (so `1000.000000`→`1000`, `0.200000`→`0.2`, `0`→`0`).

use crate::ir::Toolpath;
use serde::Deserialize;

/// How to emit (Marlin flavour for now). Unknown fields (e.g. `flavor`) are ignored.
#[derive(Debug, Clone, Deserialize)]
pub struct EmitParams {
    #[serde(default = "default_true")]
    pub relative_e: bool,
    #[serde(default)]
    pub travel_g1_e0: bool,
}

fn default_true() -> bool {
    true
}

impl Default for EmitParams {
    fn default() -> Self {
        EmitParams {
            relative_e: true,
            travel_g1_e0: false,
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

/// Emit motion g-code lines for a toolpath.
pub fn emit(tp: &Toolpath, p: &EmitParams) -> Vec<String> {
    let mut out = Vec::with_capacity(tp.segments.len());
    let mut pos: [Option<f64>; 3] = [None, None, None];
    let mut prev_speed: Option<f64> = None;
    let mut e_abs = 0.0_f64;
    let letters = ['X', 'Y', 'Z'];

    for s in &tp.segments {
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
            toks.push(format!("F{}", num(s.speed)));
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
                    toks.push(format!("{letter}{}", num(v)));
                }
                pos[i] = Some(v);
            }
        }

        if is_arc {
            let [cx, cy] = s.centre.unwrap();
            let sx = arc_start[0].unwrap_or(0.0);
            let sy = arc_start[1].unwrap_or(0.0);
            toks.push(format!("I{}", num(cx - sx)));
            toks.push(format!("J{}", num(cy - sy)));
        }

        if p.relative_e {
            if !s.travel {
                toks.push(format!("E{}", num(s.filament)));
            } else if p.travel_g1_e0 {
                toks.push("E0".to_string());
            }
        } else {
            e_abs += s.filament;
            if !s.travel || p.travel_g1_e0 {
                toks.push(format!("E{}", num(e_abs)));
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
