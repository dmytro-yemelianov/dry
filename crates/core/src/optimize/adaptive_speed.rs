use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length};

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

fn is_printing(s: &Segment) -> bool {
    !s.travel && s.kind != SegmentKind::Dwell
}

fn is_contiguous(a: &Segment, b: &Segment) -> bool {
    if !is_printing(a) || !is_printing(b) {
        return false;
    }
    point_dist(a.end, b.start) < 1e-4
}

/// Compute the unit tangent vector at the start and end of a segment.
/// Unit entry and exit tangents of a segment — arc-aware (tangent ⟂ radius, winding-signed) and
/// Δz-aware. `None` when the segment is too short or too degenerate to have a direction.
///
/// Shared with `verify`'s `junction-velocity` rule on purpose: that contract names one machine limit
/// (square-corner velocity), so the cornering quantity must be computed in exactly one place. Before
/// H1.3 `verify` measured a scalar feedrate delta under the same name, which missed the constant-speed
/// 90° corner the rule is named for.
pub(crate) fn get_tangents(s: &Segment) -> Option<([f64; 3], [f64; 3])> {
    let sx = s.start[0]?.value();
    let sy = s.start[1]?.value();
    let sz = s.start[2].unwrap_or(Length::ZERO).value();

    let ex = s.end[0]?.value();
    let ey = s.end[1]?.value();
    let ez = s.end[2].unwrap_or(Length::ZERO).value();

    let length_val = s.length.value();
    if length_val < 1e-6 {
        return None;
    }

    let dz = (ez - sz) / length_val;
    let xy_scale = libm::sqrt((1.0 - dz * dz).max(0.0));

    match s.kind {
        SegmentKind::Arc => {
            let centre = s.centre?;
            let cx = centre[0].value();
            let cy = centre[1].value();

            let rs = libm::hypot(sx - cx, sy - cy);
            let re = libm::hypot(ex - cx, ey - cy);
            if rs < 1e-6 || re < 1e-6 {
                return None;
            }

            // Normal vectors pointing from centre to points
            let (txs, tys) = if s.clockwise {
                ((sy - cy) / rs, -(sx - cx) / rs)
            } else {
                (-(sy - cy) / rs, (sx - cx) / rs)
            };

            let (txe, tye) = if s.clockwise {
                ((ey - cy) / re, -(ex - cx) / re)
            } else {
                (-(ey - cy) / re, (ex - cx) / re)
            };

            Some((
                [txs * xy_scale, tys * xy_scale, dz],
                [txe * xy_scale, tye * xy_scale, dz],
            ))
        }
        _ => {
            let dx = ex - sx;
            let dy = ey - sy;
            let d_xy = libm::hypot(dx, dy);
            if d_xy < 1e-6 {
                let t = [0.0, 0.0, dz.signum()];
                Some((t, t))
            } else {
                let tx = dx / d_xy * xy_scale;
                let ty = dy / d_xy * xy_scale;
                let t = [tx, ty, dz];
                Some((t, t))
            }
        }
    }
}

/// Dynamically scale printing segment speed based on features.
/// Uses a default acceleration limit of 500 mm/s².
pub fn adaptive_speed(tp: &Toolpath) -> Toolpath {
    adaptive_speed_with_params(tp, 500.0)
}

pub fn adaptive_speed_with_params(tp: &Toolpath, a_limit: f64) -> Toolpath {
    adaptive_speed_with_kinematics(tp, a_limit, None)
}

/// Adaptive-speed shaping with explicit kinematic limits.
///
/// `a_limit` (mm/s²) drives the arc centripetal speed limit `v_max = sqrt(a·r)`, exactly as
/// [`adaptive_speed_with_params`]. When `junction_velocity` is `Some(scv)` (a max junction /
/// square-corner velocity in mm/s), each contiguous-printing junction additionally gets an **absolute**
/// feedrate cap `scv · sqrt((1 + dot) / 2) · 60` (mm/min) on top of the existing relative cosine factor,
/// so a sharp corner is never taken faster than the machine can change direction. With `None` the result
/// is identical to [`adaptive_speed_with_params`].
pub fn adaptive_speed_with_kinematics(
    tp: &Toolpath,
    a_limit: f64,
    junction_velocity: Option<f64>,
) -> Toolpath {
    if a_limit <= 0.0 {
        return tp.clone();
    }

    let n = tp.segments.len();
    if n == 0 {
        return tp.clone();
    }

    // Precalculate tangents for all segments
    let tangents: Vec<Option<([f64; 3], [f64; 3])>> = tp
        .segments
        .iter()
        .map(|s| {
            if is_printing(s) {
                get_tangents(s)
            } else {
                None
            }
        })
        .collect();

    let mut new_segments = tp.segments.clone();

    for i in 0..n {
        let s = &tp.segments[i];
        if !is_printing(s) {
            continue;
        }

        let mut speed_scale = 1.0_f64;
        // Absolute per-junction feedrate ceiling (mm/min) from the optional junction velocity. Stays
        // `INFINITY` (no cap) when no junction velocity is supplied or this segment has no junction.
        let mut v_cap_mm_min = f64::INFINITY;

        // 1. Curvature (arc radius) limit
        if s.kind == SegmentKind::Arc {
            if let Some(centre) = s.centre {
                let cx = centre[0].value();
                let cy = centre[1].value();
                let sx = s.start[0].map(|v| v.value()).unwrap_or(0.0);
                let sy = s.start[1].map(|v| v.value()).unwrap_or(0.0);
                let r = libm::hypot(sx - cx, sy - cy);
                if r > 1e-6 {
                    let v_max = libm::sqrt(a_limit * r); // mm/s
                    let f_max = v_max * 60.0; // mm/min
                    let current_feedrate = s.speed.value();
                    if current_feedrate > f_max {
                        speed_scale = speed_scale.min(f_max / current_feedrate);
                    }
                }
            }
        }

        // 2. Corner/junction sharpness scaling with contiguous printing segments
        let s_tangents = tangents[i];

        if let Some((s_start_tang, s_end_tang)) = s_tangents {
            // Check start junction (with segment i-1)
            if i > 0 {
                let prev = &tp.segments[i - 1];
                if is_contiguous(prev, s) {
                    if let Some((_, prev_end_tang)) = tangents[i - 1] {
                        let dot = prev_end_tang[0] * s_start_tang[0]
                            + prev_end_tang[1] * s_start_tang[1]
                            + prev_end_tang[2] * s_start_tang[2];
                        // Cosine-of-half-angle junction factor
                        let factor = libm::sqrt(((1.0 + dot) / 2.0).max(0.0));
                        let clamped_factor = factor.clamp(0.2, 1.0);
                        speed_scale = speed_scale.min(clamped_factor);
                        if let Some(scv) = junction_velocity {
                            v_cap_mm_min = v_cap_mm_min.min(scv * factor * 60.0);
                        }
                    }
                }
            }

            // Check end junction (with segment i+1)
            if i + 1 < n {
                let next = &tp.segments[i + 1];
                if is_contiguous(s, next) {
                    if let Some((next_start_tang, _)) = tangents[i + 1] {
                        let dot = s_end_tang[0] * next_start_tang[0]
                            + s_end_tang[1] * next_start_tang[1]
                            + s_end_tang[2] * next_start_tang[2];
                        let factor = libm::sqrt(((1.0 + dot) / 2.0).max(0.0));
                        let clamped_factor = factor.clamp(0.2, 1.0);
                        speed_scale = speed_scale.min(clamped_factor);
                        if let Some(scv) = junction_velocity {
                            v_cap_mm_min = v_cap_mm_min.min(scv * factor * 60.0);
                        }
                    }
                }
            }
        }

        // Apply the relative cosine scale first, then the absolute junction-velocity ceiling.
        let mut new_speed = s.speed.value();
        if speed_scale < 1.0 {
            new_speed *= speed_scale;
        }
        if new_speed > v_cap_mm_min {
            new_speed = v_cap_mm_min;
        }
        if new_speed < s.speed.value() {
            new_segments[i].speed = Feedrate(new_speed);
        }
    }

    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: new_segments,
    }
}
