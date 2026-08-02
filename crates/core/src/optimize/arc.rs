use super::merge::same_run_state;
use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Angle, Length, Volume};
use std::f64::consts::TAU;

/// Absolute tolerance (mm) for the circle fit: every point of a candidate run must lie within this
/// distance of the fitted circle for the run to become an arc. Small enough that only a genuinely
/// circular run fits, large enough to absorb the f64 rounding of the circumcircle division.
const ARC_FIT_TOL: f64 = 1e-6;

/// The XY of a segment endpoint, if both axes are defined.
fn xy(p: &[Option<Length>; 3]) -> Option<(f64, f64)> {
    Some((p[0]?.value(), p[1]?.value()))
}

/// The Z of a segment endpoint, if defined.
fn z(p: &[Option<Length>; 3]) -> Option<f64> {
    p[2].map(|v| v.value())
}

/// True when both endpoints of `s` have all three axes defined — a prerequisite for arc-fitting (the
/// very first positioning move, with an undefined start, can never be part of an arc).
fn fully_defined(s: &Segment) -> bool {
    s.start.iter().all(Option::is_some) && s.end.iter().all(Option::is_some)
}

/// The circumcircle centre `(cx, cy)` of three points, or `None` if they are (near-)collinear (the
/// linear system is singular). Solves the two perpendicular-bisector equations for the centre.
fn circumcentre(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64)) -> Option<(f64, f64)> {
    let (ax, ay) = p0;
    let (bx, by) = p1;
    let (cx, cy) = p2;
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-12 {
        return None; // collinear ⇒ no finite circumcircle.
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d;
    Some((ux, uy))
}

/// Signed area of the triangle `(o, a, b)` — its sign is the turn direction at `a` about `o`. Used to
/// check the run winds consistently one way (a real arc, not an S-curve).
fn turn_sign(o: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// Try to fold the maximal same-state line run `segs` (already collected) into a single arc. Returns
/// the arc `Segment` if the points lie on a common circle within [`ARC_FIT_TOL`] with consistent
/// winding (and the run is long enough); otherwise `None` (caller keeps the run verbatim). The fitted
/// segment carries the native arc's analytic length; deposited material is kept from the source run.
fn fit_arc(segs: &[Segment]) -> Option<Segment> {
    // need ≥3 line segments (≥4 points) to justify replacing a polyline with an arc.
    if segs.len() < 3 {
        return None;
    }
    // the run is planar: every endpoint shares one Z. Collect it and reject if any Z is missing/varies.
    let first = &segs[0];
    let z0 = z(&first.start)?;
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(segs.len() + 1);
    pts.push(xy(&first.start)?);
    if (z(&first.start)? - z0).abs() > ARC_FIT_TOL {
        return None;
    }
    for s in segs {
        if (z(&s.start)? - z0).abs() > ARC_FIT_TOL || (z(&s.end)? - z0).abs() > ARC_FIT_TOL {
            return None;
        }
        pts.push(xy(&s.end)?);
    }

    // fit the circle to the first three *distinct* points.
    let p0 = pts[0];
    let dist = |a: (f64, f64), b: (f64, f64)| libm::hypot(a.0 - b.0, a.1 - b.1);
    let p1 = *pts.iter().find(|&&p| dist(p, p0) > ARC_FIT_TOL)?;
    let p2 = *pts
        .iter()
        .find(|&&p| dist(p, p0) > ARC_FIT_TOL && dist(p, p1) > ARC_FIT_TOL)?;
    let (cx, cy) = circumcentre(p0, p1, p2)?;
    let radius = dist(p0, (cx, cy));
    if radius <= ARC_FIT_TOL {
        return None;
    }

    // every point must lie on that circle, and every turn about the centre share one sign.
    let centre = (cx, cy);
    let mut sign = 0.0_f64;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (dist(b, centre) - radius).abs() > ARC_FIT_TOL {
            return None;
        }
        let t = turn_sign(centre, a, b);
        if t.abs() <= 1e-12 {
            return None; // a zero-length / radial step has no winding ⇒ not a clean arc.
        }
        if sign == 0.0 {
            sign = t.signum();
        } else if t.signum() != sign {
            return None; // winding flips ⇒ an S-curve, not a single arc.
        }
    }

    // winding: a counter-clockwise sweep (sign > 0) is G3 (clockwise = false); CW is G2.
    let clockwise = sign < 0.0;
    let (sx, sy) = p0;
    let (ex, ey) = *pts.last()?;
    let start_a = libm::atan2(sy - cy, sx - cx);
    let end_a = libm::atan2(ey - cy, ex - cx);
    // swept angle in the winding direction, normalised into `(0, TAU]` exactly as `resolve` does.
    let mut swept = Angle(if clockwise {
        start_a - end_a
    } else {
        end_a - start_a
    }) % TAU;
    if swept <= Angle::ZERO {
        swept = swept + Angle(TAU);
    }
    let length = Length::mm(radius) * swept; // planar run ⇒ Δz = 0.

    let volume = segs.iter().fold(Volume::ZERO, |acc, s| acc + s.volume);
    let filament = segs.iter().fold(Length::ZERO, |acc, s| acc + s.filament);
    let last = segs.last()?;
    Some(Segment {
        start: first.start,
        end: last.end,
        travel: first.travel,
        speed: first.speed,
        length,
        volume,
        filament,
        width: first.width,
        height: first.height,
        kind: SegmentKind::Arc,
        centre: Some([Length::mm(cx), Length::mm(cy)]),
        clockwise,
        temperature: first.temperature,
        fan: first.fan,
        flow: first.flow,
        tool: first.tool,
        power: first.power,
        dwell_s: None,
        manual_gcode: None,
        orientation: first.orientation,
        control_points: None,
    })
}

/// Replace maximal runs of consecutive, same-state line moves that lie on a common circle with a single
/// G2/G3 arc. Pure: returns a new toolpath. Non-fitting runs and arcs/dwells pass through unchanged.
/// Because the emitted target motion becomes a native arc, simulated length/time can change.
pub fn arc_fit(tp: &Toolpath) -> Toolpath {
    let mut out: Vec<Segment> = Vec::with_capacity(tp.segments.len());
    let mut i = 0;
    while i < tp.segments.len() {
        let seg = &tp.segments[i];
        // only plain, fully-defined line moves start a candidate run; everything else (arcs, dwells,
        // the first positioning move with an undefined start) passes through untouched.
        if seg.kind != SegmentKind::Line || seg.dwell_s.is_some() || !fully_defined(seg) {
            out.push(seg.clone());
            i += 1;
            continue;
        }
        // extend the run while the next move continues the same process state, contiguously and with
        // every axis defined.
        let mut j = i + 1;
        while j < tp.segments.len()
            && fully_defined(&tp.segments[j])
            && same_run_state(&tp.segments[j - 1], &tp.segments[j])
        {
            j += 1;
        }
        let run = &tp.segments[i..j];
        if let Some(arc) = fit_arc(run) {
            out.push(arc);
        } else {
            out.extend(run.iter().cloned());
        }
        i = j;
    }
    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: out,
    }
}
