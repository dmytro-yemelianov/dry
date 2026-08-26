//! GRBL Laser mode emitter (D3.2, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Emits industry-standard GRBL laser G-code:
//! - `M3`: Constant laser power mode (e.g. for cutting).
//! - `M4`: Dynamic laser power mode (auto-scales power with acceleration for engraving).
//! - `M5`: Laser off during rapid positioning moves (`G0`).
//! - `S<power>`: Commanded laser power (scaled to configured max PWM `S` value, e.g. 1000).

use crate::ir::Toolpath;
use serde::{Deserialize, Serialize};

/// Commanded laser power modulation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LaserMode {
    #[default]
    Constant, // M3
    Dynamic, // M4
}

/// Configuration parameters for GRBL laser emission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaserParams {
    pub mode: LaserMode,
    pub max_power_s: f64,
    pub default_feedrate: f64,
}

impl Default for LaserParams {
    fn default() -> Self {
        Self {
            mode: LaserMode::Dynamic,
            max_power_s: 1000.0,
            default_feedrate: 1200.0,
        }
    }
}

/// Emit GRBL laser motion G-code from an L2 [`Toolpath`].
pub fn emit_grbl_laser(toolpath: &Toolpath, params: &LaserParams) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("; Dry GRBL Laser Program".into());
    lines.push("G21 ; Millimetres".into());
    lines.push("G90 ; Absolute coordinates".into());

    let mode_cmd = match params.mode {
        LaserMode::Constant => "M3",
        LaserMode::Dynamic => "M4",
    };

    let mut laser_active = false;
    let mut last_speed = 0.0;
    let mut last_power = -1.0;

    for seg in &toolpath.segments {
        let is_rapid = seg.travel || seg.speed.value() == 0.0;

        let [Some(ex), Some(ey), _] = [seg.end[0], seg.end[1], seg.end[2]] else {
            continue;
        };

        let x = ex.value();
        let y = ey.value();

        if is_rapid {
            if laser_active {
                lines.push("M5 ; Laser off".into());
                laser_active = false;
            }
            lines.push(format!("G0 X{x:.3} Y{y:.3}"));
        } else {
            let power_val = seg
                .power
                .unwrap_or(params.max_power_s)
                .min(params.max_power_s);
            let speed = if seg.speed.value() > 0.0 {
                seg.speed.value()
            } else {
                params.default_feedrate
            };

            if !laser_active || (power_val != last_power) {
                lines.push(format!("{mode_cmd} S{power_val:.0}"));
                laser_active = true;
                last_power = power_val;
            }

            if (speed - last_speed).abs() > 1e-3 {
                lines.push(format!("G1 X{x:.3} Y{y:.3} F{speed:.1}"));
                last_speed = speed;
            } else {
                lines.push(format!("G1 X{x:.3} Y{y:.3}"));
            }
        }
    }

    if laser_active {
        lines.push("M5 ; Laser off".into());
    }
    lines.push("M2 ; End of program".into());

    lines
}
