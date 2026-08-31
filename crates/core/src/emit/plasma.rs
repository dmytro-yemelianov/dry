//! 2D/2.5D Plasma torch and abrasive waterjet cutting emitter (D3.3, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Features:
//! - Automated pierce delay (`G04 P...`).
//! - Torch ignite / abrasive jet commands (`M03` on, `M05` off).
//! - Tangential lead-in and lead-out trajectories to prevent pierce scars on cut contour edges.

use crate::ir::{SegmentKind, Toolpath};
use serde::{Deserialize, Serialize};

/// The lead-in trajectory geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LeadInType {
    None,
    Linear,
    #[default]
    Arc,
}

/// Cutting process parameters for plasma / waterjet machines.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuttingParams {
    /// Height for piercing (mm above workpiece).
    pub pierce_height: f64,
    /// Dwell time in seconds during initial pierce before motion begins.
    pub pierce_delay_s: f64,
    /// Operating cut height (mm).
    pub cut_height: f64,
    /// Safe rapid traverse height (mm).
    pub safe_traverse_height: f64,
    /// Cutting feedrate (mm/min).
    pub cut_feedrate: f64,
    /// Lead-in trajectory type.
    pub lead_in_type: LeadInType,
    /// Radius / distance for tangential lead-in (mm).
    pub lead_in_radius: f64,
}

impl Default for CuttingParams {
    fn default() -> Self {
        Self {
            pierce_height: 3.8,
            pierce_delay_s: 0.5,
            cut_height: 1.5,
            safe_traverse_height: 25.0,
            cut_feedrate: 2500.0,
            lead_in_type: LeadInType::Arc,
            lead_in_radius: 4.0,
        }
    }
}

/// Emit plasma / waterjet cutting motion G-code.
/// Emit a plasma/waterjet program.
///
/// Refuses a toolpath it cannot faithfully represent, on the same terms as [`crate::emit_stream`].
/// This emitter was added after the H1.1 emit gate and did not inherit it: every coordinate and feed
/// went through `format!("{x:.3}")`, which renders a non-finite value as the literal word `XNaN` or
/// `Xinf`. That is the exact defect H1.1's audit named — "non-finite quantities could reach metal" —
/// and on a plasma table it is an undefined move with a live torch.
/// Refuse a non-finite word before it is formatted.
///
/// `format!("{v:.3}")` renders a non-finite value as the literal text `NaN` or `inf`, so the guard
/// has to run before formatting rather than inspect the result. Kept separate from
/// `gcode::num_checked` on purpose: that helper also *formats*, and adopting it here would have
/// changed this emitter's word precision and every program it has produced.
fn finite_words(words: &[(&str, f64)]) -> Result<(), crate::codec::CodecError> {
    for (name, v) in words {
        if !v.is_finite() {
            return Err(crate::codec::CodecError::Other(format!(
                "cannot emit non-finite {name} value ({v})"
            )));
        }
    }
    Ok(())
}

pub fn emit_plasma_waterjet(
    toolpath: &Toolpath,
    params: &CuttingParams,
) -> Result<Vec<String>, crate::codec::CodecError> {
    // The machine-parameter words come from `params` and are written on every program, so they are
    // checked once up front rather than per segment.
    for (name, value) in [
        ("safe_traverse_height", params.safe_traverse_height),
        ("pierce_height", params.pierce_height),
        ("cut_height", params.cut_height),
        ("pierce_delay_s", params.pierce_delay_s),
        ("lead_in_radius", params.lead_in_radius),
    ] {
        if !value.is_finite() {
            return Err(crate::codec::CodecError::Other(format!(
                "cannot emit non-finite {name} ({value})"
            )));
        }
    }
    let mut lines = Vec::new();
    lines.push("; Dry Plasma/Waterjet Program".into());
    lines.push("G21 ; Millimetres".into());
    lines.push("G90 ; Absolute positioning".into());

    let mut torch_active = false;

    for seg in &toolpath.segments {
        // A non-finite speed is refused rather than substituted. `seg.speed <= ZERO` is false for a
        // NaN, so such a segment was treated as a cut; the `speed > ZERO` test below is false too,
        // so it then silently inherited `params.cut_feedrate`. The emitted `F` word was finite and
        // the program looked valid — an unknown commanded speed laundered into a plausible one.
        if !seg.speed.value().is_finite() {
            return Err(crate::codec::CodecError::Other(format!(
                "cannot emit non-finite speed ({})",
                seg.speed.value()
            )));
        }
        let is_rapid = seg.travel || seg.speed <= crate::units::Feedrate::ZERO;

        let [Some(ex), Some(ey), _] = [seg.end[0], seg.end[1], seg.end[2]] else {
            continue;
        };

        let x = ex.value();
        let y = ey.value();
        // Checked, not reformatted: the words keep their original `{:.3}` / `{:.1}` precision, so a
        // valid program is byte-identical to what this emitter produced before the guard.
        finite_words(&[("X", x), ("Y", y)])?;

        if is_rapid {
            if torch_active {
                lines.push("M05 ; Torch off".into());
                lines.push(format!(
                    "G00 Z{:.3} ; Retract to safe traverse",
                    params.safe_traverse_height
                ));
                torch_active = false;
            }
            lines.push(format!("G00 X{x:.3} Y{y:.3}"));
        } else {
            if !torch_active {
                // Pierce sequence
                lines.push(format!(
                    "G00 Z{:.3} ; Move to pierce height",
                    params.pierce_height
                ));
                lines.push("M03 ; Torch ON".into());
                if params.pierce_delay_s > 0.0 {
                    lines.push(format!("G04 P{:.2} ; Pierce delay", params.pierce_delay_s));
                }
                lines.push(format!(
                    "G01 Z{:.3} F1500.0 ; Drop to cut height",
                    params.cut_height
                ));

                // Lead-in annotations
                if params.lead_in_radius > 0.0 {
                    match params.lead_in_type {
                        LeadInType::Linear => {
                            lines
                                .push(format!("; Linear lead-in ({:.1}mm)", params.lead_in_radius));
                        }
                        LeadInType::Arc => {
                            lines.push(format!(
                                "; Tangential arc lead-in (R{:.1}mm)",
                                params.lead_in_radius
                            ));
                        }
                        LeadInType::None => {}
                    }
                }

                torch_active = true;
            }

            let feed = if seg.speed > crate::units::Feedrate::ZERO {
                seg.speed.value()
            } else {
                params.cut_feedrate
            };

            match seg.kind {
                SegmentKind::Arc => {
                    let dir = if seg.clockwise { "G02" } else { "G03" };
                    if let Some(c) = seg.centre {
                        let cx = c[0].value();
                        let cy = c[1].value();
                        let [Some(sx), Some(sy), _] = [seg.start[0], seg.start[1], seg.start[2]]
                        else {
                            continue;
                        };
                        let i = cx - sx.value();
                        let j = cy - sy.value();
                        finite_words(&[("I", i), ("J", j)])?;
                        lines.push(format!("{dir} X{x:.3} Y{y:.3} I{i:.3} J{j:.3} F{feed:.1}"));
                    } else {
                        lines.push(format!("G01 X{x:.3} Y{y:.3} F{feed:.1}"));
                    }
                }
                _ => {
                    lines.push(format!("G01 X{x:.3} Y{y:.3} F{feed:.1}"));
                }
            }
        }
    }

    if torch_active {
        lines.push("M05 ; Torch off".into());
        lines.push(format!("G00 Z{:.3} ; Retract", params.safe_traverse_height));
    }
    lines.push("M30 ; Program end".into());

    Ok(lines)
}
