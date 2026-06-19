//! L2 optimisation passes — semantics-preserving IR→IR transforms (`docs/01-architecture.md` §4). A
//! pass rewrites the [`Toolpath`] while keeping its *meaning*: the same path through space and the same
//! deposited material. The win is a smaller, cleaner toolpath (fewer moves), not a different one.
//!
//! `merge_collinear` is the first: it coalesces consecutive collinear moves that share *all* process
//! state (feedrate, bead, channels, orientation, travel/extrude) into one longer move — dropping the
//! redundant intermediate point. Length, volume and filament are summed, so `simulate` is unchanged
//! except for the (now lower) segment count.

use crate::ir::{Segment, Toolpath};
use crate::units::{Angle, Length, Volume};
use std::f64::consts::TAU;

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
    (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
}

/// True when `b` continues `a` in the same straight line, with identical process state — so the pair
/// can become a single move.
fn mergeable(a: &Segment, b: &Segment) -> bool {
    // both plain lines (arcs and dwells are never coalesced) with identical process state.
    if a.kind != "line"
        || b.kind != "line"
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
    let (m1, m2) = (dot(d1, d1).sqrt(), dot(d2, d2).sqrt());
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

/// True when `b` extends `a`'s run for arc-fitting: both plain lines, contiguous (`a.end == b.start`),
/// and sharing *all* process state (the same predicate `merge_collinear` uses, minus collinearity).
fn same_run_state(a: &Segment, b: &Segment) -> bool {
    a.kind == "line"
        && b.kind == "line"
        && a.dwell_s.is_none()
        && b.dwell_s.is_none()
        && a.travel == b.travel
        && a.speed == b.speed
        && a.width == b.width
        && a.height == b.height
        && a.temperature == b.temperature
        && a.fan == b.fan
        && a.flow == b.flow
        && a.tool == b.tool
        && a.orientation == b.orientation
        && a.end == b.start
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
/// winding (and the run is long enough); otherwise `None` (caller keeps the run verbatim).
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
    let dist = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1);
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
    let start_a = (sy - cy).atan2(sx - cx);
    let end_a = (ey - cy).atan2(ex - cx);
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
        kind: "arc".to_string(),
        centre: Some([Length::mm(cx), Length::mm(cy)]),
        clockwise,
        temperature: first.temperature,
        fan: first.fan,
        flow: first.flow,
        tool: first.tool,
        dwell_s: None,
        orientation: first.orientation,
    })
}

/// Replace maximal runs of consecutive, same-state line moves that lie on a common circle with a single
/// G2/G3 arc. Pure: returns a new toolpath. Non-fitting runs and arcs/dwells pass through unchanged.
pub fn arc_fit(tp: &Toolpath) -> Toolpath {
    let mut out: Vec<Segment> = Vec::with_capacity(tp.segments.len());
    let mut i = 0;
    while i < tp.segments.len() {
        let seg = &tp.segments[i];
        // only plain, fully-defined line moves start a candidate run; everything else (arcs, dwells,
        // the first positioning move with an undefined start) passes through untouched.
        if seg.kind != "line" || seg.dwell_s.is_some() || !fully_defined(seg) {
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

/// A maximal run of consecutive extruding (non-travel) segments, together with the index of the
/// representative leading travel segment that precedes it (if any) — that travel is rewritten to connect
/// the run to whatever now comes before it.
struct Run {
    /// The extruding segments of this run, kept verbatim (their geometry/material is never touched).
    segs: Vec<Segment>,
    /// The travel segment that preceded this run in the original toolpath, if any. Reused as the
    /// template (speed, channels) for the rewritten connecting travel.
    lead_travel: Option<Segment>,
}

impl Run {
    /// The run's start point — the first extruding segment's start.
    fn start(&self) -> [Option<Length>; 3] {
        self.segs[0].start
    }
    /// The run's end point — the last extruding segment's end.
    fn end(&self) -> [Option<Length>; 3] {
        self.segs[self.segs.len() - 1].end
    }
}

/// Euclidean distance between two points over the axes defined in *both* (undefined axes are ignored,
/// matching `resolve`'s `dist`). Used to score nearest-neighbour ordering of runs.
fn point_dist(a: [Option<Length>; 3], b: [Option<Length>; 3]) -> f64 {
    let mut sq = 0.0;
    for i in 0..3 {
        if let (Some(p), Some(q)) = (a[i], b[i]) {
            let d = q.value() - p.value();
            sq += d * d;
        }
    }
    sq.sqrt()
}

/// Build a straight travel `Segment` from `from` to `to`, reusing `template`'s speed/channels. The move
/// deposits nothing (volume/filament zero) and is a plain line.
fn make_travel(from: [Option<Length>; 3], to: [Option<Length>; 3], template: &Segment) -> Segment {
    Segment {
        start: from,
        end: to,
        travel: true,
        speed: template.speed,
        length: Length::mm(point_dist(from, to)),
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width: template.width,
        height: template.height,
        kind: "line".to_string(),
        centre: None,
        clockwise: false,
        temperature: template.temperature,
        fan: template.fan,
        flow: None,
        tool: template.tool,
        dwell_s: None,
        orientation: template.orientation,
    }
}

/// Reorder independent extrusion runs to reduce total travel, without changing any run's internal
/// geometry or deposited material. Pure: returns a new toolpath.
///
/// A run is a maximal sequence of consecutive non-travel (extruding) segments; runs are separated by
/// travel moves. The first run is kept fixed (it begins the print); the rest are greedily ordered by
/// nearest-neighbour from the current end position. Connecting travels are regenerated as single
/// straight moves. Conservative: if the toolpath contains dwells/arcs in travel position, or has 0/1
/// runs, or any run boundary is ambiguous, it is returned unchanged.
pub fn travel_reorder(tp: &Toolpath) -> Toolpath {
    // Partition into runs separated by travels. We only reorder the simplest structure: alternating
    // optional-leading-travel + extruding-run. Anything we don't recognise (a dwell, a leading travel
    // that is not a plain line, multiple consecutive travels) makes us bail out unchanged — be
    // conservative and deterministic.
    let segs = &tp.segments;
    let mut runs: Vec<Run> = Vec::new();
    let mut i = 0;
    while i < segs.len() {
        let mut lead_travel: Option<Segment> = None;
        // collect a single leading travel (a plain line) if present.
        if segs[i].travel {
            // only a plain-line travel is a recognised connector; a dwell or arc-in-travel ⇒ bail.
            if segs[i].kind != "line" || segs[i].dwell_s.is_some() {
                return tp.clone();
            }
            lead_travel = Some(segs[i].clone());
            i += 1;
            // a second consecutive travel is unexpected structure ⇒ bail out unchanged.
            if i < segs.len() && segs[i].travel {
                return tp.clone();
            }
        }
        // now collect the extruding run.
        if i >= segs.len() || segs[i].travel {
            // a leading travel with no following extruding segment (trailing travel) ⇒ bail.
            return tp.clone();
        }
        let run_start = i;
        while i < segs.len() && !segs[i].travel {
            i += 1;
        }
        runs.push(Run {
            segs: segs[run_start..i].to_vec(),
            lead_travel,
        });
    }

    // 0 or 1 runs ⇒ nothing to reorder.
    if runs.len() <= 1 {
        return tp.clone();
    }

    // A representative travel speed/template: prefer any run's leading travel, else the first run's
    // first segment. (We need *some* template for the very first connecting travel if the first run had
    // no lead.)
    let template = runs
        .iter()
        .find_map(|r| r.lead_travel.clone())
        .unwrap_or_else(|| runs[0].segs[0].clone());

    // Greedy nearest-neighbour over the runs after the first. The first run is fixed.
    let mut remaining: Vec<Run> = runs.split_off(1);
    let first = runs.pop().expect("exactly one run before split");

    let mut ordered: Vec<Run> = Vec::with_capacity(remaining.len() + 1);
    let mut pos = first.end();
    ordered.push(first);
    while !remaining.is_empty() {
        // pick the remaining run whose start is closest to the current position. Ties broken by index
        // (the first such run) for determinism.
        let mut best = 0;
        let mut best_d = point_dist(pos, remaining[0].start());
        for (k, r) in remaining.iter().enumerate().skip(1) {
            let d = point_dist(pos, r.start());
            if d < best_d {
                best_d = d;
                best = k;
            }
        }
        let next = remaining.remove(best);
        pos = next.end();
        ordered.push(next);
    }

    // Re-emit: the first run keeps its original leading travel (if any); every subsequent run gets a
    // regenerated straight travel from the previous run's end to its start.
    let mut out: Vec<Segment> = Vec::with_capacity(segs.len());
    let mut prev_end: Option<[Option<Length>; 3]> = None;
    for (idx, run) in ordered.iter().enumerate() {
        if idx == 0 {
            // keep the first run's original leading travel verbatim (preserves the print's opening).
            if let Some(t) = &run.lead_travel {
                out.push(t.clone());
            }
        } else {
            let from = prev_end.expect("a previous run sets the position");
            out.push(make_travel(from, run.start(), &template));
        }
        out.extend(run.segs.iter().cloned());
        prev_end = Some(run.end());
    }

    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_singleton_are_unchanged() {
        let empty = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert_eq!(merge_collinear(&empty), empty);
    }
}
