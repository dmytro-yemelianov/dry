//! Parametric CNC Lathe & Turning Generator (Track E, `docs/04-tasks.md`).
//!
//! Generates multi-pass turning toolpaths for 2-axis CNC lathes (XZ plane):
//! - Facing operations: surface the workpiece end face to Z=0.
//! - Outer Diameter (OD) roughing: progressive linear passes down to target radius with finish allowance.
//! - Profiling & Finishing: single contour pass at final dimensions.

use crate::resolve::Op;
use serde::{Deserialize, Serialize};

/// Parameters for CNC Lathe Facing operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatheFacingParams {
    /// Stock outer diameter in mm (e.g. 50.0).
    pub stock_diameter: f64,
    /// Facing cut target Z coordinate (mm, e.g. 0.0).
    pub target_z: f64,
    /// Safe retract clearance X beyond stock radius (mm).
    pub clearance_x: f64,
    /// Safe retract clearance Z ahead of workpiece face (mm).
    pub clearance_z: f64,
    /// Cutting feedrate (mm/min).
    pub feedrate: f64,
    /// Spindle speed (RPM) or surface speed.
    pub spindle_rpm: f64,
    /// Number of facing depth passes.
    pub passes: usize,
    /// Depth of cut per pass along Z (mm).
    pub depth_per_pass: f64,
}

impl Default for LatheFacingParams {
    fn default() -> Self {
        Self {
            stock_diameter: 40.0,
            target_z: 0.0,
            clearance_x: 2.0,
            clearance_z: 2.0,
            feedrate: 250.0,
            spindle_rpm: 1200.0,
            passes: 1,
            depth_per_pass: 1.0,
        }
    }
}

/// Parameters for CNC Lathe Outer Diameter (OD) Roughing & Finishing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatheTurningParams {
    /// Starting raw stock diameter (mm).
    pub raw_diameter: f64,
    /// Finished target outer diameter (mm).
    pub target_diameter: f64,
    /// Total length of cut along Z axis (positive value, cutting from Z=0 to Z=-cut_length).
    pub cut_length: f64,
    /// Maximum radial depth of cut per pass (mm).
    pub depth_of_cut: f64,
    /// Radial finish allowance left for final pass (mm).
    pub finish_allowance: f64,
    /// Safe retract clearance X (mm).
    pub clearance_x: f64,
    /// Safe retract clearance Z (mm).
    pub clearance_z: f64,
    /// Roughing feedrate (mm/min).
    pub rough_feedrate: f64,
    /// Finishing feedrate (mm/min).
    pub finish_feedrate: f64,
    /// Spindle speed (RPM).
    pub spindle_rpm: f64,
}

impl Default for LatheTurningParams {
    fn default() -> Self {
        Self {
            raw_diameter: 50.0,
            target_diameter: 30.0,
            cut_length: 45.0,
            depth_of_cut: 2.0,
            finish_allowance: 0.5,
            clearance_x: 2.0,
            clearance_z: 2.0,
            rough_feedrate: 300.0,
            finish_feedrate: 150.0,
            spindle_rpm: 1500.0,
        }
    }
}

/// Generates parametric L1 ops for CNC lathe facing.
/// In standard lathe programming, X represents radial position (or diameter/2) and Z represents axial position.
pub fn generate_lathe_facing_ops(params: &LatheFacingParams) -> Result<Vec<Op>, String> {
    if params.stock_diameter <= 0.0 || !params.stock_diameter.is_finite() {
        return Err("Stock diameter must be positive and finite".into());
    }
    if params.feedrate <= 0.0 || !params.feedrate.is_finite() {
        return Err("Feedrate must be positive and finite".into());
    }
    if params.passes == 0 {
        return Err("Number of passes must be at least 1".into());
    }

    let mut ops = Vec::new();
    let r_stock = params.stock_diameter / 2.0;
    let start_x = r_stock + params.clearance_x;

    ops.push(Op::Power { level: params.spindle_rpm });
    ops.push(Op::Speed { print: params.feedrate });
    ops.push(Op::Extruder { on: false });

    // Initial positioning in safe clearance
    ops.push(Op::Move {
        x: Some(start_x),
        y: Some(0.0),
        z: Some(params.target_z + params.clearance_z),
    });

    for pass in 0..params.passes {
        let current_z = params.target_z + ((params.passes - 1 - pass) as f64 * params.depth_per_pass);

        // Move to start X at clearance
        ops.push(Op::Move {
            x: Some(start_x),
            y: Some(0.0),
            z: Some(current_z + 1.0),
        });

        // Plunge to pass Z
        ops.push(Op::Move {
            x: Some(start_x),
            y: Some(0.0),
            z: Some(current_z),
        });

        // Cut across face from outer radius past center (to X=-0.5 mm to ensure clean tip cutoff)
        ops.push(Op::Move {
            x: Some(-0.5),
            y: Some(0.0),
            z: Some(current_z),
        });

        // Retract 1mm in Z
        ops.push(Op::Move {
            x: Some(-0.5),
            y: Some(0.0),
            z: Some(current_z + 1.0),
        });

        // Rapid back to start X
        ops.push(Op::Move {
            x: Some(start_x),
            y: Some(0.0),
            z: Some(current_z + 1.0),
        });
    }

    // Final retract to full clearance
    ops.push(Op::Move {
        x: Some(start_x),
        y: Some(0.0),
        z: Some(params.target_z + params.clearance_z),
    });

    Ok(ops)
}

/// Generates parametric L1 ops for CNC lathe outer diameter (OD) roughing & finish pass.
pub fn generate_lathe_od_turning_ops(params: &LatheTurningParams) -> Result<Vec<Op>, String> {
    if params.raw_diameter <= 0.0 || !params.raw_diameter.is_finite() {
        return Err("Raw diameter must be positive and finite".into());
    }
    if params.target_diameter <= 0.0 || !params.target_diameter.is_finite() {
        return Err("Target diameter must be positive and finite".into());
    }
    if params.target_diameter >= params.raw_diameter {
        return Err("Target diameter must be smaller than raw diameter for OD turning".into());
    }
    if params.cut_length <= 0.0 || !params.cut_length.is_finite() {
        return Err("Cut length must be positive and finite".into());
    }
    if params.depth_of_cut <= 0.0 || !params.depth_of_cut.is_finite() {
        return Err("Depth of cut must be positive and finite".into());
    }

    let mut ops = Vec::new();
    let r_raw = params.raw_diameter / 2.0;
    let r_target = params.target_diameter / 2.0;
    let r_rough_final = r_target + params.finish_allowance;
    let total_radial_removal = r_raw - r_rough_final;

    let num_rough_passes = (total_radial_removal / params.depth_of_cut).ceil() as usize;
    let actual_doc = if num_rough_passes > 0 {
        total_radial_removal / (num_rough_passes as f64)
    } else {
        0.0
    };

    ops.push(Op::Power { level: params.spindle_rpm });
    ops.push(Op::Speed { print: params.rough_feedrate });
    ops.push(Op::Extruder { on: false });

    let safe_x = r_raw + params.clearance_x;
    let safe_z = params.clearance_z;
    let z_end = -params.cut_length;

    // Safe approach
    ops.push(Op::Move {
        x: Some(safe_x),
        y: Some(0.0),
        z: Some(safe_z),
    });

    // Roughing passes
    for pass in 1..=num_rough_passes {
        let current_r = r_raw - (pass as f64 * actual_doc);

        // Position at pass radius in clearance Z
        ops.push(Op::Move {
            x: Some(current_r),
            y: Some(0.0),
            z: Some(safe_z),
        });

        // Longitudinal cut along Z
        ops.push(Op::Move {
            x: Some(current_r),
            y: Some(0.0),
            z: Some(z_end),
        });

        // Retract diagonally 45 degrees
        ops.push(Op::Move {
            x: Some(current_r + 1.0),
            y: Some(0.0),
            z: Some(z_end + 1.0),
        });

        // Rapid back to start Z
        ops.push(Op::Move {
            x: Some(current_r + 1.0),
            y: Some(0.0),
            z: Some(safe_z),
        });
    }

    // Finishing pass (if finish allowance > 0)
    if params.finish_allowance > 0.0 {
        ops.push(Op::Speed { print: params.finish_feedrate });

        // Position at target radius
        ops.push(Op::Move {
            x: Some(r_target),
            y: Some(0.0),
            z: Some(safe_z),
        });

        // Precision finishing cut
        ops.push(Op::Move {
            x: Some(r_target),
            y: Some(0.0),
            z: Some(z_end),
        });

        // Retract 45 degrees
        ops.push(Op::Move {
            x: Some(r_target + params.clearance_x),
            y: Some(0.0),
            z: Some(z_end + 1.0),
        });

        // Return to safe home
        ops.push(Op::Move {
            x: Some(safe_x),
            y: Some(0.0),
            z: Some(safe_z),
        });
    }

    Ok(ops)
}
