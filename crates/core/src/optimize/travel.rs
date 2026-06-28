use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Length, Volume};

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
    libm::sqrt(sq)
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
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: template.temperature,
        fan: template.fan,
        flow: None,
        tool: template.tool,
        dwell_s: None,
        manual_gcode: None,
        orientation: template.orientation,
        control_points: None,
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
            if segs[i].kind != SegmentKind::Line || segs[i].dwell_s.is_some() {
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
