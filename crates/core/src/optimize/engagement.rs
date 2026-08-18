//! Radial tool engagement & corner feedrate optimization (D4.2, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Prevents tool chatter, vibration, and cutter deflection by dynamically scaling entry feedrates
//! around sharp internal corners where the radial width of cut spikes.

use crate::ir::Toolpath;
use crate::units::Feedrate;

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

        let (Some(s1_x), Some(s1_y)) = (toolpath.segments[i].start[0], toolpath.segments[i].start[1]) else { continue };
        let (Some(e1_x), Some(e1_y)) = (toolpath.segments[i].end[0], toolpath.segments[i].end[1]) else { continue };
        let (Some(e2_x), Some(e2_y)) = (toolpath.segments[i + 1].end[0], toolpath.segments[i + 1].end[1]) else { continue };

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
