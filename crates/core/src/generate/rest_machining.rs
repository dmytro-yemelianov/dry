//! Multi-Pass 2D/3D Rest Machining (Track E / Advanced CNC CAM).
//!
//! Automatically calculates uncut residual material left by larger roughing tools ($D_{\text{rough}}$)
//! in sharp internal corners and pockets, generating localized finishing toolpaths for smaller tools ($D_{\text{rest}}$)
//! without re-machining open cleared stock.

use crate::resolve::Op;
use serde::{Deserialize, Serialize};

/// Configuration parameters for rest machining cleanup passes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RestMachiningParams {
    /// Diameter of previous roughing tool (mm).
    pub rough_tool_diameter: f64,
    /// Diameter of small rest-machining cleanup tool (mm).
    pub rest_tool_diameter: f64,
    /// Corner vertex coordinate `[x, y]` in WCS (mm).
    pub corner_vertex: [f64; 2],
    /// Angle of the inside corner (degrees, e.g. 90.0).
    pub corner_angle_deg: f64,
    /// Depth of the cut Z (negative for milling into stock, mm).
    pub z_cut: f64,
    /// Safe retract clearance Z (mm).
    pub z_clearance: f64,
    /// Cutting feedrate (mm/min).
    pub feedrate: f64,
    /// Number of radial finishing cleanup passes (e.g. 2 or 3).
    pub radial_passes: usize,
}

impl Default for RestMachiningParams {
    fn default() -> Self {
        Self {
            rough_tool_diameter: 12.0,
            rest_tool_diameter: 4.0,
            corner_vertex: [0.0, 0.0],
            corner_angle_deg: 90.0,
            z_cut: -5.0,
            z_clearance: 5.0,
            feedrate: 800.0,
            radial_passes: 3,
        }
    }
}

/// Generates localized rest machining cleanup toolpath operations for an inside corner.
pub fn generate_corner_rest_machining_ops(params: &RestMachiningParams) -> Result<Vec<Op>, String> {
    if params.rest_tool_diameter <= 0.0 {
        return Err("Rest tool diameter must be positive".into());
    }
    if params.rough_tool_diameter <= params.rest_tool_diameter {
        return Err("Rough tool diameter must be strictly greater than rest tool diameter".into());
    }
    if params.corner_angle_deg <= 0.0 || params.corner_angle_deg >= 180.0 {
        return Err("Inside corner angle must be between 0 and 180 degrees".into());
    }

    let r_rough = params.rough_tool_diameter / 2.0;
    let r_rest = params.rest_tool_diameter / 2.0;

    let half_angle_rad = (params.corner_angle_deg / 2.0).to_radians();
    let sin_half = half_angle_rad.sin();

    // Distance from corner vertex to tool center at tangency
    let d_rough = r_rough / sin_half;
    let d_rest = r_rest / sin_half;

    let total_delta_d = d_rough - d_rest;
    if total_delta_d < 1e-4 {
        return Ok(Vec::new());
    }

    let num_passes = params.radial_passes.max(1);
    let step_r = (r_rough - r_rest) / (num_passes as f64);

    let vx = params.corner_vertex[0];
    let vy = params.corner_vertex[1];

    let mut ops = Vec::new();

    // Initial positioning at roughing boundary
    let start_x = vx + r_rough;
    let start_y = vy + (r_rough - step_r);

    ops.push(Op::Speed { print: params.feedrate });
    ops.push(Op::Extruder { on: false });
    ops.push(Op::Move {
        x: Some(start_x),
        y: Some(start_y),
        z: Some(params.z_clearance),
    });

    // Plunge to cutting depth
    ops.push(Op::Move {
        x: Some(start_x),
        y: Some(start_y),
        z: Some(params.z_cut),
    });
    ops.push(Op::Extruder { on: true });

    // Progressive radial cleanup passes from rough boundary inward to final rest radius
    for pass in 1..=num_passes {
        let r_k = r_rough - (pass as f64 * step_r);
        let arc_r = r_rough - r_k;

        let pass_start_x = vx + r_rough;
        let pass_start_y = vy + r_k;
        let pass_end_x = vx + r_k;
        let pass_end_y = vy + r_rough;

        // Move to start of this pass
        ops.push(Op::Move {
            x: Some(pass_start_x),
            y: Some(pass_start_y),
            z: Some(params.z_cut),
        });

        if arc_r > 1e-4 {
            // Circular arc centered at (vx + r_k, vy + r_k) with exact radius (r_rough - r_k)
            ops.push(Op::Arc {
                cx: vx + r_k,
                cy: vy + r_k,
                x: Some(pass_end_x),
                y: Some(pass_end_y),
                z: Some(params.z_cut),
                clockwise: false,
            });
        } else {
            ops.push(Op::Move {
                x: Some(pass_end_x),
                y: Some(pass_end_y),
                z: Some(params.z_cut),
            });
        }
    }

    // Retract to safe Z
    ops.push(Op::Extruder { on: false });
    ops.push(Op::Move {
        x: None,
        y: None,
        z: Some(params.z_clearance),
    });

    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rest_machining_validation() {
        let params = RestMachiningParams {
            rough_tool_diameter: 4.0,
            rest_tool_diameter: 6.0, // Invalid: rest > rough
            ..RestMachiningParams::default()
        };

        assert!(generate_corner_rest_machining_ops(&params).is_err());
    }

    #[test]
    fn test_rest_machining_pass_generation() {
        let params = RestMachiningParams {
            rough_tool_diameter: 12.0,
            rest_tool_diameter: 4.0,
            corner_vertex: [50.0, 50.0],
            corner_angle_deg: 90.0,
            z_cut: -3.0,
            z_clearance: 5.0,
            feedrate: 1200.0,
            radial_passes: 3,
        };

        let ops = generate_corner_rest_machining_ops(&params).expect("Should generate rest ops");
        assert!(ops.len() >= 8, "Must contain clearance, plunge, passes, and retract");
    }
}
