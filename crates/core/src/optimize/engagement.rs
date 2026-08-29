//! Radial tool engagement & corner feedrate optimization (D4.2, `docs/04-tasks.md` — unplanned series D2–D4).
//!
//! Prevents tool chatter, vibration, and cutter deflection by dynamically scaling entry feedrates
//! around sharp internal corners where the radial width of cut spikes.

use crate::ir::Toolpath;
use crate::units::{Feedrate, Length};

/// Computes the radial engagement angle $\theta_e$ of a cylindrical milling tool cutter (in radians).
///
/// In straight cuts with a given stepover ratio $s = \text{stepover} / D \in (0, 1]$, the engagement angle is:
/// $\theta_e = \arccos(1 - 2s)$
/// In a turn of interior angle $\alpha$, the engagement angle spikes by $(\pi - \alpha)$.
pub fn calculate_radial_engagement_angle(stepover_ratio: f64, turn_angle_rad: f64) -> f64 {
    let s = stepover_ratio.clamp(0.01, 1.0);
    let straight_theta = (1.0 - 2.0 * s).clamp(-1.0, 1.0).acos();
    let turn_spike = (std::f64::consts::PI - turn_angle_rad).max(0.0);
    (straight_theta + turn_spike).min(std::f64::consts::PI)
}

/// Optimize corner feedrates on a toolpath based on angular transitions.
///
/// `min_feed_ratio` sets the lower bound for feedrate reduction (e.g. 0.4 for 40% of original feedrate).
pub fn optimize_corner_feedrate(toolpath: &mut Toolpath, min_feed_ratio: f64) {
    if toolpath.segments.len() < 2 {
        return;
    }

    let min_ratio = min_feed_ratio.clamp(0.1, 1.0);

    for i in 0..toolpath.segments.len() - 1 {
        // Only shape cutting moves (not rapid travels)
        if toolpath.segments[i].travel || toolpath.segments[i + 1].travel {
            continue;
        }

        let (Some(s1_x), Some(s1_y)) =
            (toolpath.segments[i].start[0], toolpath.segments[i].start[1])
        else {
            continue;
        };
        let (Some(e1_x), Some(e1_y)) = (toolpath.segments[i].end[0], toolpath.segments[i].end[1])
        else {
            continue;
        };
        let (Some(e2_x), Some(e2_y)) = (
            toolpath.segments[i + 1].end[0],
            toolpath.segments[i + 1].end[1],
        ) else {
            continue;
        };

        let dx1 = e1_x.value() - s1_x.value();
        let dy1 = e1_y.value() - s1_y.value();
        let len1 = libm::hypot(dx1, dy1);

        let dx2 = e2_x.value() - e1_x.value();
        let dy2 = e2_y.value() - e1_y.value();
        let len2 = libm::hypot(dx2, dy2);

        if len1 < 1e-4 || len2 < 1e-4 {
            continue;
        }

        let dot = (dx1 * dx2 + dy1 * dy2) / (len1 * len2);
        let cos_angle = dot.clamp(-1.0, 1.0);

        // If turn is sharper than ~30 degrees (cos_angle < 0.866)
        if cos_angle < 0.866 {
            // Factor is proportional to (1 + cos_angle) / 2
            let scale = ((1.0 + cos_angle) * 0.5).sqrt().clamp(min_ratio, 1.0);
            let current_speed = toolpath.segments[i].speed.value();
            toolpath.segments[i].speed = Feedrate(current_speed * scale);
        }
    }
}

/// Dynamically scales feedrates across all toolpath segments according to instantaneous radial tool engagement.
pub fn optimize_radial_engagement(
    toolpath: &mut Toolpath,
    stepover_ratio: f64,
    min_feed_ratio: f64,
) {
    if toolpath.segments.len() < 2 {
        return;
    }

    let s = stepover_ratio.clamp(0.01, 1.0);
    let nominal_theta = (1.0 - 2.0 * s).clamp(-1.0, 1.0).acos();
    let min_ratio = min_feed_ratio.clamp(0.1, 1.0);

    for i in 0..toolpath.segments.len() - 1 {
        if toolpath.segments[i].travel || toolpath.segments[i + 1].travel {
            continue;
        }

        let (Some(s1_x), Some(s1_y)) =
            (toolpath.segments[i].start[0], toolpath.segments[i].start[1])
        else {
            continue;
        };
        let (Some(e1_x), Some(e1_y)) = (toolpath.segments[i].end[0], toolpath.segments[i].end[1])
        else {
            continue;
        };
        let (Some(e2_x), Some(e2_y)) = (
            toolpath.segments[i + 1].end[0],
            toolpath.segments[i + 1].end[1],
        ) else {
            continue;
        };

        let dx1 = e1_x.value() - s1_x.value();
        let dy1 = e1_y.value() - s1_y.value();
        let len1 = libm::hypot(dx1, dy1);

        let dx2 = e2_x.value() - e1_x.value();
        let dy2 = e2_y.value() - e1_y.value();
        let len2 = libm::hypot(dx2, dy2);

        if len1 < 1e-4 || len2 < 1e-4 {
            continue;
        }

        let dot = (dx1 * dx2 + dy1 * dy2) / (len1 * len2);
        let cos_angle = dot.clamp(-1.0, 1.0);
        let turn_angle_rad = cos_angle.acos();

        let theta_e = calculate_radial_engagement_angle(s, turn_angle_rad);
        if theta_e > nominal_theta + 0.1 {
            // Scale feedrate inversely proportional to engagement angle ratio
            let scale = (nominal_theta / theta_e).clamp(min_ratio, 1.0);
            let current_speed = toolpath.segments[i].speed.value();
            toolpath.segments[i].speed = Feedrate(current_speed * scale);
        }
    }
}

/// Computes the Dynamic Chip Thinning Compensation (DCTC) multiplier for small radial stepover $s = a_e / D \in (0, 0.5]$.
///
/// When $s < 0.5$, chip thickness $h_{\text{max}} = f_z \cdot 2 \sqrt{s(1 - s)}$. To maintain constant chip load:
/// $\text{multiplier} = \frac{1}{2 \sqrt{s(1 - s)}}$.
/// For $s \ge 0.5$, the multiplier is 1.0 (no chip thinning).
pub fn calculate_chip_thinning_multiplier(stepover_ratio: f64) -> f64 {
    let s = stepover_ratio.clamp(0.001, 1.0);
    if s >= 0.5 {
        1.0
    } else {
        let denom = 2.0 * (s * (1.0 - s)).sqrt();
        if denom > 1e-4 {
            (1.0 / denom).min(3.5) // Cap maximum feedrate scaling to 350% for machine safety
        } else {
            1.0
        }
    }
}

/// Applies Dynamic Chip Thinning Compensation across all cutting segments of a toolpath given a nominal radial stepover ratio.
pub fn apply_chip_thinning_compensation(toolpath: &mut Toolpath, stepover_ratio: f64) {
    let multiplier = calculate_chip_thinning_multiplier(stepover_ratio);
    if (multiplier - 1.0).abs() < 1e-4 {
        return;
    }
    for seg in &mut toolpath.segments {
        if !seg.travel && seg.length > Length::ZERO {
            let current = seg.speed.value();
            seg.speed = Feedrate(current * multiplier);
        }
    }
}

/// Generates progressive trochoidal peeling arc moves along a straight slot channel.
pub fn generate_trochoidal_slot(
    start: [f64; 2],
    end: [f64; 2],
    z_cut: f64,
    slot_width: f64,
    tool_diameter: f64,
    step_forward: f64,
    feedrate: f64,
) -> Result<Vec<crate::resolve::Op>, String> {
    if tool_diameter <= 0.0 || slot_width <= tool_diameter {
        return Err("Slot width must be greater than tool diameter".into());
    }
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let total_len = libm::hypot(dx, dy);
    if total_len < 1e-4 {
        return Err("Slot length must be positive".into());
    }

    let ux = dx / total_len;
    let uy = dy / total_len;
    let nx = -uy;
    let ny = ux;

    let orbit_radius = (slot_width - tool_diameter) / 2.0;
    let step = step_forward.clamp(0.1, tool_diameter * 0.5);
    let num_steps = (total_len / step).ceil() as usize;

    let mut ops = vec![
        crate::resolve::Op::Speed { print: feedrate },
        crate::resolve::Op::Extruder { on: true },
        crate::resolve::Op::Move {
            x: Some(start[0]),
            y: Some(start[1]),
            z: Some(z_cut),
        },
    ];

    for i in 0..=num_steps {
        let progress = (i as f64 * step).min(total_len);
        let cx = start[0] + ux * progress;
        let cy = start[1] + uy * progress;

        // Circular trochoidal cutting loop around (cx, cy)
        ops.push(crate::resolve::Op::Arc {
            cx,
            cy,
            x: Some(cx + nx * orbit_radius),
            y: Some(cy + ny * orbit_radius),
            z: Some(z_cut),
            clockwise: false,
        });
        ops.push(crate::resolve::Op::Arc {
            cx,
            cy,
            x: Some(cx - nx * orbit_radius),
            y: Some(cy - ny * orbit_radius),
            z: Some(z_cut),
            clockwise: false,
        });
    }

    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{resolve, Design, Op, ResolveParams};

    #[test]
    fn test_radial_engagement_calculation() {
        // 50% stepover on straight cut -> theta_e = acos(0) = pi/2 = 90 deg
        let straight_90 = calculate_radial_engagement_angle(0.5, std::f64::consts::PI);
        assert!((straight_90 - std::f64::consts::FRAC_PI_2).abs() < 1e-5);

        // 90 deg inside corner turn -> theta_e increases
        let corner = calculate_radial_engagement_angle(0.5, std::f64::consts::FRAC_PI_2);
        assert!(corner > straight_90);
    }

    #[test]
    fn test_chip_thinning_calculation() {
        // At 50% stepover, chip thinning factor is 1.0 (nominal)
        assert_eq!(calculate_chip_thinning_multiplier(0.5), 1.0);

        // At 10% stepover, chip thinning factor is > 1.6x
        let factor_10 = calculate_chip_thinning_multiplier(0.1);
        assert!(factor_10 > 1.6);
    }

    #[test]
    fn test_trochoidal_slot_generation() {
        let ops = generate_trochoidal_slot([0.0, 0.0], [50.0, 0.0], -2.0, 12.0, 6.0, 1.0, 1500.0);
        assert!(ops.is_ok());
        let op_list = ops.unwrap();
        assert!(op_list.len() > 20, "Should generate progressive trochoidal peeling arcs");
    }

    #[test]
    fn test_optimize_corner_feedrate_decelerates_sharp_turns() {
        let design = Design {
            ops: vec![
                Op::Speed { print: 1000.0 },
                Op::Extruder { on: true },
                Op::Move {
                    x: Some(0.0),
                    y: Some(0.0),
                    z: Some(0.0),
                },
                Op::Move {
                    x: Some(10.0),
                    y: Some(0.0),
                    z: Some(0.0),
                },
                Op::Move {
                    x: Some(10.0),
                    y: Some(10.0),
                    z: Some(0.0),
                },
            ],
        };
        let mut tp = resolve(&design, &ResolveParams::default());
        optimize_corner_feedrate(&mut tp, 0.4);
        assert!(tp.segments[1].speed.value() < 1000.0);
        assert!(tp.segments[1].speed.value() >= 400.0);
    }
}



