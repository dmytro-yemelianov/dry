use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length};

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

/// Everything a controller acts on that is *not* the run's own geometry: feedrate, bead,
/// travel/extrude, orientation and every process channel. Two segments may only be folded into one
/// move (or one arc) when these are equal — a difference here is a command the machine would
/// otherwise never receive.
///
/// Built by an **exhaustive** destructure of [`Segment`] on purpose: a new field then fails to
/// compile here, and whoever adds it has to decide whether it is process state. Nothing else caught
/// the `power` channel, and the consequence was physical — a resolved `[600, 600, 0]` coalesced to
/// `[600, 600]`, leaving the beam lit across a move the program authored dark.
///
/// It is also what a pass that *synthesises* a segment must fill in (`travel::make_travel`), so the
/// question "what state does this new move carry?" is asked once, in these terms, rather than by
/// each pass copying fields off whatever donor segment is in scope.
#[derive(PartialEq)]
pub(super) struct ProcessState {
    pub(super) travel: bool,
    pub(super) speed: Feedrate,
    pub(super) width: Option<Length>,
    pub(super) height: Option<Length>,
    pub(super) temperature: Option<f64>,
    pub(super) fan: Option<f64>,
    pub(super) flow: Option<f64>,
    pub(super) tool: Option<u32>,
    pub(super) power: Option<f64>,
    pub(super) orientation: Option<[f64; 3]>,
}

pub(super) fn process_state(s: &Segment) -> ProcessState {
    let Segment {
        // Geometry: what the two segments' endpoints must satisfy is the caller's business
        // (contiguity here and in `arc_fit`, collinearity in `mergeable`).
        start: _,
        end: _,
        travel,
        speed,
        // Additive/derived quantities: summed by the merge, recomputed by the arc fit.
        length: _,
        volume: _,
        filament: _,
        width,
        height,
        // `kind` is checked separately (only plain lines are ever folded), and the fields that exist
        // only for the other kinds are therefore already excluded by it.
        kind: _,
        centre: _,
        clockwise: _,
        temperature,
        fan,
        flow,
        tool,
        power,
        dwell_s: _,
        manual_gcode: _,
        orientation,
        control_points: _,
    } = s;
    ProcessState {
        travel: *travel,
        speed: *speed,
        width: *width,
        height: *height,
        temperature: *temperature,
        fan: *fan,
        flow: *flow,
        tool: *tool,
        power: *power,
        orientation: *orientation,
    }
}

/// True when `b` continues `a`'s run: both plain lines (arcs and dwells are never folded),
/// contiguous, and sharing all [`ProcessState`]. `mergeable` adds collinearity on top of it; `arc_fit`
/// uses it as-is, so the two passes cannot drift apart about what "same state" means.
pub(super) fn same_run_state(a: &Segment, b: &Segment) -> bool {
    a.kind == SegmentKind::Line
        && b.kind == SegmentKind::Line
        && a.dwell_s.is_none()
        && b.dwell_s.is_none()
        && a.end == b.start
        && process_state(a) == process_state(b)
}

/// True when `b` continues `a` in the same straight line, with identical process state — so the pair
/// can become a single move.
fn mergeable(a: &Segment, b: &Segment) -> bool {
    if !same_run_state(a, b) {
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
