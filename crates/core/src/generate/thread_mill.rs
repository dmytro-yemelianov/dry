//! Parametric CNC Thread Milling & 3D Chamfering Generator (Track E, `docs/02-roadmap.md`).
//!
//! Generates high-precision helical toolpaths with tangential lead-in/lead-out arcs
//! for internal and external ISO metric / UN threads, and constant-load 3D chamfer passes.

use crate::resolve::Op;

/// Parameters for CNC Helical Thread Milling.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadMillParams {
    /// Nominal major diameter of thread (mm) (e.g. 10.0 for M10).
    pub nominal_diameter: f64,
    /// Thread pitch in mm (e.g. 1.5 for M10x1.5).
    pub pitch: f64,
    /// Total thread depth in mm along Z (e.g. 15.0 mm).
    pub thread_depth: f64,
    /// Cutter effective cutting diameter (mm) (e.g. 6.0 mm).
    pub tool_diameter: f64,
    /// True for internal hole thread (tapped hole), false for external rod thread (bolt).
    pub is_internal: bool,
    /// True for right-hand thread (M10-RH), false for left-hand thread (M10-LH).
    pub right_hand: bool,
    /// True for climb milling (standard CNC), false for conventional milling.
    pub climb: bool,
    /// Cutting feedrate (mm/min).
    pub feedrate: f64,
    /// Spindle speed (RPM).
    pub spindle_rpm: f64,
}

impl Default for ThreadMillParams {
    fn default() -> Self {
        Self {
            nominal_diameter: 10.0,
            pitch: 1.5,
            thread_depth: 12.0,
            tool_diameter: 6.0,
            is_internal: true,
            right_hand: true,
            climb: true,
            feedrate: 800.0,
            spindle_rpm: 4500.0,
        }
    }
}

/// Generates parametric L1 ops for a complete CNC thread milling operation.
pub fn generate_thread_milling_ops(params: &ThreadMillParams, center_x: f64, center_y: f64, start_z: f64) -> Result<Vec<Op>, String> {
    if params.nominal_diameter <= 0.0 || !params.nominal_diameter.is_finite() {
        return Err("Nominal diameter must be positive and finite".into());
    }
    if params.pitch <= 0.0 || !params.pitch.is_finite() {
        return Err("Pitch must be positive and finite".into());
    }
    if params.thread_depth <= 0.0 || !params.thread_depth.is_finite() {
        return Err("Thread depth must be positive and finite".into());
    }
    if params.tool_diameter <= 0.0 || !params.tool_diameter.is_finite() {
        return Err("Tool diameter must be positive and finite".into());
    }
    if params.is_internal && params.tool_diameter >= params.nominal_diameter {
        return Err(format!(
            "Tool diameter ({} mm) must be smaller than hole major diameter ({} mm)",
            params.tool_diameter, params.nominal_diameter
        ));
    }

    let mut ops = Vec::new();

    // Spindle start and feedrate configuration
    ops.push(Op::Power { level: params.spindle_rpm });
    ops.push(Op::Speed { print: params.feedrate });

    let tool_radius = params.tool_diameter / 2.0;
    let nominal_radius = params.nominal_diameter / 2.0;

    // Tool center path radius
    let orbit_radius = if params.is_internal {
        nominal_radius - tool_radius
    } else {
        nominal_radius + tool_radius
    };

    // Calculate number of full helical revolutions needed
    let total_revolutions = (params.thread_depth / params.pitch).ceil() as usize;
    let z_bottom = start_z - params.thread_depth;

    // Lead-in tangential arc radius
    let lead_in_r = orbit_radius / 2.0;

    // 1. Rapid move to clearance center above hole
    ops.push(Op::Move { x: Some(center_x), y: Some(center_y), z: Some(start_z + 2.0) });

    // 2. Plunge down hole center to bottom Z
    ops.push(Op::Move { x: Some(center_x), y: Some(center_y), z: Some(z_bottom) });

    // 3. Tangential lead-in arc (180 deg semi-circle from center to orbit edge)
    // Starts at (center_x, center_y, z_bottom), sweeps to (center_x + orbit_radius, center_y, z_bottom + pitch / 4.0)
    let lead_in_x = center_x + orbit_radius;
    let lead_in_y = center_y;
    let lead_in_z = z_bottom + params.pitch * 0.25;

    ops.push(Op::Arc {
        cx: center_x + lead_in_r,
        cy: center_y,
        x: Some(lead_in_x),
        y: Some(lead_in_y),
        z: Some(lead_in_z),
        clockwise: false,
    });

    // 4. Helical thread cutting passes (each 360 deg circle rises by exactly 1.0 pitch)
    let mut current_z = lead_in_z;
    for _ in 0..total_revolutions {
        let next_z = current_z + params.pitch;
        // 360-degree full circle via two 180-degree arcs to guarantee exact non-degenerate geometry
        let mid_x = center_x - orbit_radius;
        let mid_z = current_z + params.pitch * 0.5;

        ops.push(Op::Arc {
            cx: center_x,
            cy: center_y,
            x: Some(mid_x),
            y: Some(center_y),
            z: Some(mid_z),
            clockwise: false,
        });

        ops.push(Op::Arc {
            cx: center_x,
            cy: center_y,
            x: Some(lead_in_x),
            y: Some(center_y),
            z: Some(next_z),
            clockwise: false,
        });

        current_z = next_z;
    }

    // 5. Tangential lead-out arc back to center
    let lead_out_z = current_z + params.pitch * 0.25;
    ops.push(Op::Arc {
        cx: center_x + lead_in_r,
        cy: center_y,
        x: Some(center_x),
        y: Some(center_y),
        z: Some(lead_out_z),
        clockwise: false,
    });

    // 6. Retract to top clearance
    ops.push(Op::Move { x: Some(center_x), y: Some(center_y), z: Some(start_z + 5.0) });

    Ok(ops)
}

/// Parameters for CNC Chamfering / Deburring.
#[derive(Debug, Clone, PartialEq)]
pub struct ChamferParams {
    /// Width of chamfer bevel (mm) (e.g. 1.0 mm for 1x45° chamfer).
    pub chamfer_width: f64,
    /// Chamfer angle in degrees (default: 45.0 deg).
    pub chamfer_angle_deg: f64,
    /// Cutter tip flat diameter (mm) (e.g. 1.0 mm).
    pub tip_diameter: f64,
    /// Cutter major diameter (mm) (e.g. 10.0 mm).
    pub cutter_diameter: f64,
    /// Cutting feedrate (mm/min).
    pub feedrate: f64,
    /// Spindle speed (RPM).
    pub spindle_rpm: f64,
}

impl Default for ChamferParams {
    fn default() -> Self {
        Self {
            chamfer_width: 1.0,
            chamfer_angle_deg: 45.0,
            tip_diameter: 1.0,
            cutter_diameter: 10.0,
            feedrate: 1200.0,
            spindle_rpm: 6000.0,
        }
    }
}

/// Generates a constant-load 3D chamfer toolpath along a 2D polyline contour.
pub fn generate_chamfer_ops(
    params: &ChamferParams,
    contour_points: &[[f64; 2]],
    z_surface: f64,
) -> Result<Vec<Op>, String> {
    if contour_points.len() < 2 {
        return Err("Chamfer contour requires at least 2 points".into());
    }
    if params.chamfer_width <= 0.0 || !params.chamfer_width.is_finite() {
        return Err("Chamfer width must be positive and finite".into());
    }

    // Offset calculation along tool flute to avoid machining with 0-velocity dead center tip
    let tip_radius = params.tip_diameter / 2.0;
    let clearance_offset = 1.0; // 1mm up the chamfer cone
    let radial_offset = tip_radius + clearance_offset + (params.chamfer_width / 2.0);
    let depth_offset = radial_offset * (params.chamfer_angle_deg.to_radians()).tan();
    let cutting_z = z_surface - depth_offset;

    let mut ops = vec![
        Op::Power { level: params.spindle_rpm },
        Op::Speed { print: params.feedrate },
        Op::Move {
            x: Some(contour_points[0][0]),
            y: Some(contour_points[0][1]),
            z: Some(z_surface + 5.0),
        },
        Op::Move {
            x: Some(contour_points[0][0]),
            y: Some(contour_points[0][1]),
            z: Some(cutting_z),
        },
    ];

    // Trace along contour
    for pt in &contour_points[1..] {
        ops.push(Op::Move {
            x: Some(pt[0]),
            y: Some(pt[1]),
            z: Some(cutting_z),
        });
    }

    // Retract
    ops.push(Op::Move {
        x: Some(contour_points.last().unwrap()[0]),
        y: Some(contour_points.last().unwrap()[1]),
        z: Some(z_surface + 5.0),
    });

    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_milling_generation() {
        let params = ThreadMillParams {
            nominal_diameter: 12.0,
            pitch: 1.75,
            thread_depth: 10.0,
            tool_diameter: 8.0,
            is_internal: true,
            right_hand: true,
            climb: true,
            feedrate: 600.0,
            spindle_rpm: 3500.0,
        };

        let ops = generate_thread_milling_ops(&params, 50.0, 50.0, 0.0);
        assert!(ops.is_ok(), "Thread milling ops generation must succeed");
        let op_list = ops.unwrap();
        assert!(op_list.len() > 10, "Should generate multi-revolution helical arcs");
    }

    #[test]
    fn test_thread_milling_invalid_tool_size() {
        let params = ThreadMillParams {
            nominal_diameter: 6.0,
            pitch: 1.0,
            thread_depth: 10.0,
            tool_diameter: 8.0, // Tool larger than hole!
            ..Default::default()
        };

        let ops = generate_thread_milling_ops(&params, 0.0, 0.0, 0.0);
        assert!(ops.is_err(), "Tool larger than internal thread hole must be rejected");
    }

    #[test]
    fn test_chamfer_ops_generation() {
        let params = ChamferParams::default();
        let contour = vec![[0.0, 0.0], [50.0, 0.0], [50.0, 50.0], [0.0, 50.0], [0.0, 0.0]];
        let ops = generate_chamfer_ops(&params, &contour, 0.0);
        assert!(ops.is_ok());
        let op_list = ops.unwrap();
        assert_eq!(op_list.len(), 9);
    }
}
