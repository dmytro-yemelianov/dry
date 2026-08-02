use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Length, Volume};

fn point_dist(a: [Option<Length>; 3], b: [Option<Length>; 3]) -> f64 {
    let mut sq = 0.0;
    for i in 0..3 {
        if let (Some(p), Some(q)) = (a[i], b[i]) {
            let d = q.value() - p.value();
            sq += d * d;
        }
    }
    libm::sqrt(sq)
}

/// Apply Z-hop during travel moves to prevent the nozzle from scratching the print.
/// Splits travel moves longer than `zhop_min_dist` (default 2.0mm) into a vertical lift of
/// `zhop_height` (default 0.4mm), XY travel at the elevated Z, and a vertical drop.
pub fn z_hop(tp: &Toolpath) -> Toolpath {
    z_hop_with_params(tp, Length::mm(0.4), Length::mm(2.0))
}

pub fn z_hop_with_params(tp: &Toolpath, zhop_height: Length, zhop_min_dist: Length) -> Toolpath {
    if zhop_height.value() <= 0.0 {
        return tp.clone();
    }

    let mut out: Vec<Segment> = Vec::with_capacity(tp.segments.len() * 2);

    for s in &tp.segments {
        let is_eligible = s.travel
            && s.kind == SegmentKind::Line
            && s.length.value() >= zhop_min_dist.value()
            && s.start[2].is_some()
            && s.end[2].is_some();

        if is_eligible {
            let z_start = s.start[2].unwrap();
            let z_end = s.end[2].unwrap();
            let z_hop_start = z_start + zhop_height;
            let z_hop_end = z_end + zhop_height;

            let p_start_hopped = [s.start[0], s.start[1], Some(z_hop_start)];
            let p_end_hopped = [s.end[0], s.end[1], Some(z_hop_end)];

            // 1. Vertical lift
            let s_lift = Segment {
                start: s.start,
                end: p_start_hopped,
                travel: true,
                speed: s.speed,
                length: zhop_height,
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: s.width,
                height: s.height,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: s.temperature,
                fan: s.fan,
                flow: None,
                tool: s.tool,
                power: s.power,
                dwell_s: None,
                manual_gcode: None,
                orientation: s.orientation,
                control_points: None,
            };

            // 2. Main travel move at elevated Z
            let s_travel = Segment {
                start: p_start_hopped,
                end: p_end_hopped,
                travel: true,
                speed: s.speed,
                length: Length::mm(point_dist(p_start_hopped, p_end_hopped)),
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: s.width,
                height: s.height,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: s.temperature,
                fan: s.fan,
                flow: None,
                tool: s.tool,
                power: s.power,
                dwell_s: None,
                manual_gcode: None,
                orientation: s.orientation,
                control_points: None,
            };

            // 3. Vertical drop
            let lower_dist = z_hop_end.value() - z_end.value();
            let s_lower = Segment {
                start: p_end_hopped,
                end: s.end,
                travel: true,
                speed: s.speed,
                length: Length::mm(lower_dist),
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: s.width,
                height: s.height,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: s.temperature,
                fan: s.fan,
                flow: None,
                tool: s.tool,
                power: s.power,
                dwell_s: None,
                manual_gcode: None,
                orientation: s.orientation,
                control_points: None,
            };

            out.push(s_lift);
            out.push(s_travel);
            out.push(s_lower);
        } else {
            out.push(s.clone());
        }
    }

    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: out,
    }
}
