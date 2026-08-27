//! GRBL Laser mode emitter (D3.2, `docs/04-tasks.md` — unplanned series D2–D4).
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

/// Why a toolpath cannot be emitted as a laser program.
///
/// These are refusals rather than repairs. The previous behaviour substituted
/// `params.max_power_s` for a segment whose power was never commanded — and, because `f64::min`
/// returns its non-NaN operand, for a NaN power too — so the least-informed input produced the most
/// dangerous output a laser can be given. `crate::ir::Segment::power` documents `None` as "never
/// commanded", distinct from `Some(0.0)` meaning commanded off; inventing full beam power for it is
/// not a defensible reading of that.
#[derive(Debug, Clone, PartialEq)]
pub enum LaserError {
    /// A cutting move carried no commanded power.
    UncommandedPower { segment: usize },
    /// A commanded power that is NaN or infinite.
    NonFinitePower { segment: usize, value: f64 },
    /// A negative commanded power, which was previously emitted verbatim as `S-5`.
    NegativePower { segment: usize, value: f64 },
}

impl std::fmt::Display for LaserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UncommandedPower { segment } => write!(
                f,
                "segment {segment} cuts but commands no laser power; set it explicitly (0 means off)"
            ),
            Self::NonFinitePower { segment, value } => {
                write!(f, "segment {segment} commands a non-finite laser power ({value})")
            }
            Self::NegativePower { segment, value } => {
                write!(f, "segment {segment} commands a negative laser power ({value})")
            }
        }
    }
}

impl std::error::Error for LaserError {}

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
pub fn emit_grbl_laser(
    toolpath: &Toolpath,
    params: &LaserParams,
) -> Result<Vec<String>, LaserError> {
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

    for (index, seg) in toolpath.segments.iter().enumerate() {
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
            // Refuse rather than invent. Clamping the *upper* end is still right — a commanded
            // power above the machine's maximum is a request the hardware cannot honour — but the
            // lower and undefined ends are the caller's to state.
            let commanded = seg
                .power
                .ok_or(LaserError::UncommandedPower { segment: index })?;
            if !commanded.is_finite() {
                return Err(LaserError::NonFinitePower {
                    segment: index,
                    value: commanded,
                });
            }
            if commanded < 0.0 {
                return Err(LaserError::NegativePower {
                    segment: index,
                    value: commanded,
                });
            }
            let power_val = commanded.min(params.max_power_s);
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

    Ok(lines)
}
