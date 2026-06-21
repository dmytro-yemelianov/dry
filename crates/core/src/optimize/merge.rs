use crate::ir::{Segment, SegmentKind, Toolpath};

/// The 3-D `start → end` direction of a line segment, if every axis is defined.
fn direction(s: &Segment) -> Option<[f64; 3]> {
    let mut d = [0.0; 3];
    for (di, (a, b)) in d.iter_mut().zip(s.start.iter().zip(s.end.iter())) {
        *di = (*b)?.value() - (*a)?.value();
    }
    Some(d)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross_mag(a: [f64; 3], b: [f64; 3]) -> f64 {
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    libm::sqrt(c[0] * c[0] + c[1] * c[1] + c[2] * c[2])
}

/// True when `b` continues `a` in the same straight line, with identical process state — so the pair
/// can become a single move.
fn mergeable(a: &Segment, b: &Segment) -> bool {
    // both plain lines (arcs and dwells are never coalesced) with identical process state.
    if a.kind != SegmentKind::Line
        || b.kind != SegmentKind::Line
        || a.dwell_s.is_some()
        || b.dwell_s.is_some()
        || a.travel != b.travel
        || a.speed != b.speed
        || a.width != b.width
        || a.height != b.height
        || a.temperature != b.temperature
        || a.fan != b.fan
        || a.flow != b.flow
        || a.tool != b.tool
        || a.orientation != b.orientation
        || a.end != b.start
    {
        return false;
    }
    let (Some(d1), Some(d2)) = (direction(a), direction(b)) else {
        return false;
    };
    let (m1, m2) = (libm::sqrt(dot(d1, d1)), libm::sqrt(dot(d2, d2)));
    if m1 == 0.0 || m2 == 0.0 {
        return false; // a zero-length move has no direction to match.
    }
    // collinear (cross ≈ 0, relative to the magnitudes) and same direction (positive dot product).
    cross_mag(d1, d2) <= 1e-9 * m1 * m2 && dot(d1, d2) > 0.0
}

/// Coalesce consecutive collinear, same-state moves. Pure: returns a new toolpath.
pub fn merge_collinear(tp: &Toolpath) -> Toolpath {
    let mut out: Vec<Segment> = Vec::with_capacity(tp.segments.len());
    for seg in &tp.segments {
        if let Some(last) = out.last_mut() {
            if mergeable(last, seg) {
                last.end = seg.end;
                last.length = last.length + seg.length;
                last.volume = last.volume + seg.volume;
                last.filament = last.filament + seg.filament;
                continue;
            }
        }
        out.push(seg.clone());
    }
    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: out,
    }
}
