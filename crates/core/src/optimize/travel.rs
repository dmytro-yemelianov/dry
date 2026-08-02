use super::merge::{process_state, ProcessState};
use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};

/// A maximal run of consecutive extruding (non-travel) segments, together with the leading travel
/// segment that preceded it in the original toolpath (if any) — once the run moves, that travel is
/// replaced by one connecting it to whatever now comes before it.
struct Run {
    /// The extruding segments of this run, kept verbatim (their geometry/material is never touched).
    segs: Vec<Segment>,
    /// The travel segment that preceded this run in the original toolpath, if any. Kept verbatim when
    /// this run stays first; otherwise it survives only as a source of the *travel feedrate* (see
    /// [`travel_reorder`]) — never of process state, which a regenerated travel takes from the run it
    /// now follows.
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

/// The beam state of a travel *this pass invented*: dark.
///
/// `None` and `Some(0.0)` are not interchangeable, and the difference is the whole hazard. `None` is
/// "never commanded" — emit writes no word at all, so the controller keeps burning at the last `S` it
/// was given. `Some(0.0)` is "commanded off" — GRBL writes `M5` (`docs/10` §3.3). A travel that follows
/// a lit run must therefore *say* `Some(0.0)`: inheriting a level was what lit a 10 mm rapid at S600.
///
/// It must not invent the channel either. On a toolpath that never commands power, `Some(0.0)` would
/// make every non-GRBL flavor refuse the whole program — `emit` refuses a flavor that cannot render the
/// channel rather than dropping it (ADR 0002 §4) — so reordering an ordinary FFF print would stop it
/// emitting, and a GRBL one would gain an `M5` for a spindle the program never mentioned. `entering` is
/// the level the machine is at as the travel begins; the channel is sticky, so `None` there means
/// nothing before this point in the reordered program commanded one either.
fn beam_off(entering: Option<f64>) -> Option<f64> {
    entering.map(|_| 0.0)
}

/// The process state a *regenerated* connecting travel carries.
///
/// Not some other travel's. `travel_reorder` moves whole runs around, so the only state a synthesised
/// travel can honestly claim is the state the machine is in when it starts — whatever the run it now
/// follows (`after`) left behind. Copying it off an arbitrary original travel reinstates a machine state
/// from a point in the program that no longer exists.
///
/// Three fields are deliberately *not* continued:
/// * `travel` — it is a rapid, whatever the run before it was doing.
/// * `speed` — a travel moves at the travel feedrate (`travel_speed`, taken from a real travel of this
///   toolpath); continuing the extrusion feedrate would crawl every regenerated rapid.
/// * `flow` — it deposits nothing, so there is no deposition to scale.
///
/// and `power` is replaced by [`beam_off`], not continued.
fn connecting_state(after: &Segment, travel_speed: Feedrate) -> ProcessState {
    let mut st = process_state(after);
    st.travel = true;
    st.speed = travel_speed;
    st.flow = None;
    st.power = beam_off(st.power);
    st
}

/// Build a straight travel `Segment` from `from` to `to` carrying `state`. The move deposits nothing
/// (volume/filament zero) and is a plain line.
///
/// `state` is destructured rather than field-accessed for the same reason [`ProcessState`] destructures
/// [`Segment`]: a channel added there fails to compile here too, so no future channel can reach a
/// synthesised travel by whatever the nearest donor happened to hold.
fn make_travel(from: [Option<Length>; 3], to: [Option<Length>; 3], state: ProcessState) -> Segment {
    let ProcessState {
        travel,
        speed,
        width,
        height,
        temperature,
        fan,
        flow,
        tool,
        power,
        orientation,
    } = state;
    Segment {
        start: from,
        end: to,
        travel,
        speed,
        length: Length::mm(point_dist(from, to)),
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width,
        height,
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature,
        fan,
        flow,
        tool,
        power,
        dwell_s: None,
        manual_gcode: None,
        orientation,
        control_points: None,
    }
}

/// Reorder independent extrusion runs to reduce total travel, without changing any run's internal
/// geometry or deposited material. Pure: returns a new toolpath.
///
/// A run is a maximal sequence of consecutive non-travel (extruding) segments; runs are separated by
/// travel moves. The first run is kept fixed (it begins the print); the rest are greedily ordered by
/// nearest-neighbour from the current end position. Connecting travels are regenerated as single
/// straight moves that continue the machine state of the run they follow, with the beam commanded off
/// (see [`connecting_state`]). Conservative: if the toolpath contains dwells/arcs in travel position, or
/// has 0/1 runs, or any run boundary is ambiguous, it is returned unchanged.
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

    // A representative travel feedrate: any run's leading travel (runs after the first are separated by
    // one, so this always finds a real travel of this toolpath), else the first run's own feedrate.
    // Feedrate is *all* a regenerated travel takes from elsewhere; its machine state comes from the run
    // it follows (see `connecting_state`).
    let travel_speed = runs
        .iter()
        .find_map(|r| r.lead_travel.as_ref().map(|t| t.speed))
        .unwrap_or(runs[0].segs[0].speed);

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
    for (idx, run) in ordered.iter().enumerate() {
        if idx == 0 {
            // keep the first run's original leading travel verbatim (preserves the print's opening).
            if let Some(t) = &run.lead_travel {
                out.push(t.clone());
            }
        } else {
            // the segment the travel departs from — the previous run's last move — is both the position
            // and the machine state it must continue from.
            let prev = ordered[idx - 1]
                .segs
                .last()
                .expect("a run holds at least one segment");
            let state = connecting_state(prev, travel_speed);
            out.push(make_travel(prev.end, run.start(), state));
        }
        out.extend(run.segs.iter().cloned());
    }

    Toolpath {
        version: tp.version,
        meta: tp.meta.clone(),
        segments: out,
    }
}
