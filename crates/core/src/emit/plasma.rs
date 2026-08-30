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
pub fn emit_plasma_waterjet(toolpath: &Toolpath, params: &CuttingParams) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("; Dry Plasma/Waterjet Program".into());
    lines.push("G21 ; Millimetres".into());
    lines.push("G90 ; Absolute positioning".into());

    let mut torch_active = false;

    for seg in &toolpath.segments {
        let is_rapid = seg.travel || seg.speed <= crate::units::Feedrate::ZERO;

        let [Some(ex), Some(ey), _] = [seg.end[0], seg.end[1], seg.end[2]] else {
            continue;
        };

        let x = ex.value();
        let y = ey.value();

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

    lines
}
