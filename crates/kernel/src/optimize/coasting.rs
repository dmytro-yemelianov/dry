use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Length, Volume};

/// Interpolate a 3D point between `start` and `end` at parameter `t` in `[0, 1]`.
fn interpolate_point(
    start: [Option<Length>; 3],
    end: [Option<Length>; 3],
    t: f64,
) -> [Option<Length>; 3] {
    let mut out = [None, None, None];
    for i in 0..3 {
        if let (Some(s), Some(e)) = (start[i], end[i]) {
            out[i] = Some(Length::mm(s.value() + t * (e.value() - s.value())));
        } else if let Some(s) = start[i] {
            out[i] = Some(s);
        } else if let Some(e) = end[i] {
            out[i] = Some(e);
        }
    }
    out
}

/// Split a segment at a given distance `d` from its start.
/// Returns (left_segment, right_segment).
fn split_segment(s: &Segment, d: Length) -> (Segment, Segment) {
    let len_val = s.length.value();
    if len_val <= 1e-9 {
        return (s.clone(), s.clone());
    }
    let t = (d.value() / len_val).clamp(0.0, 1.0);

    // Interpolate Z axis linearly
    let z_start = s.start[2].map(|v| v.value());
    let z_end = s.end[2].map(|v| v.value());
    let z_split = match (z_start, z_end) {
        (Some(zs), Some(ze)) => Some(Length::mm(zs + t * (ze - zs))),
        (Some(zs), None) => Some(Length::mm(zs)),
        (None, Some(ze)) => Some(Length::mm(ze)),
        (None, None) => None,
    };

    let split_point: [Option<Length>; 3];
    let mut left_centre = None;
    let mut right_centre = None;

    match s.kind {
        SegmentKind::Arc => {
            if let Some(centre) = s.centre {
                let cx = centre[0].value();
                let cy = centre[1].value();
                let sx = s.start[0].map(|v| v.value()).unwrap_or(0.0);
                let sy = s.start[1].map(|v| v.value()).unwrap_or(0.0);

                let r = libm::hypot(sx - cx, sy - cy);
                let start_angle = libm::atan2(sy - cy, sx - cx);

                let swept_angle = d.value() / r;
                let split_angle = if s.clockwise {
                    start_angle - swept_angle
                } else {
                    start_angle + swept_angle
                };

                let split_x = cx + r * libm::cos(split_angle);
                let split_y = cy + r * libm::sin(split_angle);

                split_point = [
                    Some(Length::mm(split_x)),
                    Some(Length::mm(split_y)),
                    z_split,
                ];
                left_centre = Some(centre);
                right_centre = Some(centre);
            } else {
                let sp = interpolate_point(s.start, s.end, t);
                split_point = [sp[0], sp[1], z_split];
            }
        }
        _ => {
            let sp = interpolate_point(s.start, s.end, t);
            split_point = [sp[0], sp[1], z_split];
        }
    }

    let mut left = s.clone();
    left.end = split_point;
    left.length = d;
    left.volume = s.volume * t;
    left.filament = s.filament * t;
    left.centre = left_centre;

    let mut right = s.clone();
    right.start = split_point;
    right.length = Length::mm(len_val - d.value());
    right.volume = s.volume * (1.0 - t);
    right.filament = s.filament * (1.0 - t);
    right.centre = right_centre;

    (left, right)
}

/// Apply coasting to the end of each extrusion run.
/// Replaces the last portion of an extrusion path before a travel move (or the end of the toolpath)
/// with a non-extruding move (volume and filament set to zero), reducing oozing.
/// The default coasting distance is 0.3mm, capped at 50% of the extrusion run's length.
pub fn coasting(tp: &Toolpath) -> Toolpath {
    coasting_with_dist(tp, Length::mm(0.3))
}

pub fn coasting_with_dist(tp: &Toolpath, coasting_dist: Length) -> Toolpath {
    if coasting_dist.value() <= 0.0 {
        return tp.clone();
    }

    let mut out: Vec<Segment> = Vec::with_capacity(tp.segments.len());
    let mut run: Vec<Segment> = Vec::new();

    for seg in &tp.segments {
        let can_split_for_coasting =
            !seg.travel && matches!(seg.kind, SegmentKind::Line | SegmentKind::Arc);
        if !can_split_for_coasting {
            if !run.is_empty() {
                out.extend(process_run(&run, coasting_dist));
                run.clear();
            }
            out.push(seg.clone());
        } else {
            run.push(seg.clone());
        }
    }

    if !run.is_empty() {
        out.extend(process_run(&run, coasting_dist));
    }

    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: out,
    }
}

fn process_run(run: &[Segment], coasting_dist: Length) -> Vec<Segment> {
    let run_len: f64 = run.iter().map(|s| s.length.value()).sum();
    if run_len <= 1e-6 {
        return run.to_vec();
    }

    let c_dist = coasting_dist.value().min(run_len * 0.5);
    if c_dist <= 1e-6 {
        return run.to_vec();
    }

    let mut remaining_coast = c_dist;
    let mut processed: Vec<Segment> = Vec::with_capacity(run.len() + 1);

    for s in run.iter().rev() {
        if remaining_coast <= 1e-9 {
            processed.push(s.clone());
        } else if s.length.value() <= remaining_coast + 1e-9 {
            let mut coast_seg = s.clone();
            coast_seg.volume = Volume::ZERO;
            coast_seg.filament = Length::ZERO;
            remaining_coast -= s.length.value();
            processed.push(coast_seg);
        } else {
            let split_len = s.length.value() - remaining_coast;
            let (left, mut right) = split_segment(s, Length::mm(split_len));
            right.volume = Volume::ZERO;
            right.filament = Length::ZERO;
            processed.push(right);
            processed.push(left);
            remaining_coast = 0.0;
        }
    }

    processed.reverse();
    processed
}
