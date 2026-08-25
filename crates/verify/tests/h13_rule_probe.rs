//! H1.3 §5 corpus probe — **report-only**, run before the new rules are wired into `verify_stream`.
//!
//! The design spec (`docs/superpowers/specs/2026-07-31-verify-strengthening-design.md` §5) fixes this
//! order deliberately: each candidate always-on rule is first run over every frozen corpus as a probe,
//! *before* it can fail CI, so that a hit is triaged rather than silently tuned away. §5's prediction is
//! that `continuity`, `arc-length` and `filament-consistency` fire **nowhere** in these corpora, because
//! `resolve`, `lift` and the codec all thread position exactly. A hit is the headline finding of the
//! slice and stops it until classified as (a) rule defect, (b) oracle divergence, or (c) synthetic
//! fixture defect.
//!
//! Run with:
//!   cargo test -p kmet-verify --test h13_rule_probe -- --ignored --nocapture
//!
//! The predicates here are deliberately duplicated from the spec rather than imported from `verify.rs`:
//! at probe time they do not exist there yet, and keeping them local means the probe measures the spec's
//! predicate, not whatever the implementation later becomes.

use kmet_kernel::ir::{Segment, SegmentKind, Toolpath};
use kmet_kernel::units::Length;
use std::collections::BTreeMap;
use std::f64::consts::TAU;
use std::fs;
use std::path::{Path, PathBuf};

/// A candidate rule, as a predicate over a toolpath's segments returning one message per hit.
type RulePredicate = fn(&[Segment]) -> Vec<String>;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// §3.2 hybrid tolerance: absolute below 1 mm, relative above. Third use of an idiom already at
/// `verify.rs:522` and `gcode/lift.rs:819`.
fn hybrid_exceeds(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() > rel * a.abs().max(b.abs()).max(1.0)
}

// ---------------------------------------------------------------------------------------------
// Candidate predicates (spec §3.1–§3.3)
// ---------------------------------------------------------------------------------------------

/// `continuity` (§3.2): per-axis, `None` inherits, `ManualGcode` resets tracked position.
fn probe_continuity(segments: &[Segment]) -> Vec<String> {
    let mut hits = Vec::new();
    let mut tracked: [Option<Length>; 3] = [None; 3];
    let axis = ['X', 'Y', 'Z'];

    for (i, s) in segments.iter().enumerate() {
        if s.kind == SegmentKind::ManualGcode {
            // Verbatim G-code may move the machine arbitrarily; claiming a continuity result across
            // it would be a stronger claim than we can support.
            tracked = [None; 3];
            continue;
        }
        for (k, (prev, start)) in tracked.iter().zip(s.start.iter()).enumerate() {
            if let (Some(p), Some(q)) = (prev, start) {
                if hybrid_exceeds(p.value(), q.value(), 1e-6) {
                    hits.push(format!(
                        "seg {i}: {} gap {:.9} mm (prev end {:.6} -> start {:.6})",
                        axis[k],
                        (p.value() - q.value()).abs(),
                        p.value(),
                        q.value()
                    ));
                }
            }
        }
        // "inherit": an unstated axis is the previous value.
        for (t, end) in tracked.iter_mut().zip(s.end.iter()) {
            *t = end.or(*t);
        }
    }
    hits
}

/// `negative-quantity` (§3.1): length/volume/speed/power < 0, width/height <= 0 when `Some`.
/// `filament` < 0 is a retraction and is excluded.
fn probe_negative_quantity(segments: &[Segment]) -> Vec<String> {
    let mut hits = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        if s.length.value() < 0.0 {
            hits.push(format!("seg {i}: length {} < 0", s.length.value()));
        }
        if let Some(p) = s.power {
            if p < 0.0 {
                hits.push(format!("seg {i}: power {p} < 0"));
            }
        }
        if s.volume.value() < 0.0 {
            hits.push(format!("seg {i}: volume {} < 0", s.volume.value()));
        }
        if s.speed.value() < 0.0 {
            hits.push(format!("seg {i}: speed {} < 0", s.speed.value()));
        }
        if let Some(w) = s.width {
            if w.value() <= 0.0 {
                hits.push(format!(
                    "seg {i}: width {} <= 0 (travel={}, kind={:?})",
                    w.value(),
                    s.travel,
                    s.kind
                ));
            }
        }
        if let Some(h) = s.height {
            if h.value() <= 0.0 {
                hits.push(format!(
                    "seg {i}: height {} <= 0 (travel={}, kind={:?})",
                    h.value(),
                    s.travel,
                    s.kind
                ));
            }
        }
    }
    hits
}

/// `segment-length`: for straight or stationary primitives, the declared `length` must equal the
/// distance between the segment's own endpoints. Added after the first probe run exposed the gap —
/// `arc-length` covers arcs only, so `vectors/retract_unretract` seg 0 (`length: 0.0` with endpoints
/// 10 mm apart) was visible only as a downstream `continuity` symptom on the *following* segment.
///
/// `Arc` is excluded (`arc-length` owns it), `Spline` is excluded (its `length` is the sampled curve,
/// not the chord), and `ManualGcode` is excluded as unmodeled.
fn probe_segment_length(segments: &[Segment]) -> Vec<String> {
    let mut hits = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        if matches!(
            s.kind,
            SegmentKind::Arc | SegmentKind::Spline | SegmentKind::ManualGcode
        ) {
            continue;
        }
        let (Some(sx), Some(sy), Some(sz), Some(ex), Some(ey), Some(ez)) = (
            s.start[0], s.start[1], s.start[2], s.end[0], s.end[1], s.end[2],
        ) else {
            continue; // an undefined axis inherits; no displacement is asserted
        };
        let dx = ex.value() - sx.value();
        let dy = ey.value() - sy.value();
        let dz = ez.value() - sz.value();
        let expected = (dx * dx + dy * dy + dz * dz).sqrt();
        if hybrid_exceeds(s.length.value(), expected, 1e-6) {
            hits.push(format!(
                "seg {i} ({:?}): length {:.9} vs |end-start| {:.9}",
                s.kind,
                s.length.value(),
                expected
            ));
        }
    }
    hits
}

fn normalised_angle(v: f64) -> f64 {
    let mut out = v % TAU;
    if out < 0.0 {
        out += TAU;
    }
    out
}

fn swept_delta(start: f64, end: f64, clockwise: bool) -> f64 {
    let delta = normalised_angle(if clockwise { start - end } else { end - start });
    if delta <= 1e-12 {
        TAU
    } else {
        delta
    }
}

/// `arc-length` (§2): one formula everywhere — `length ≈ hypot(r·sweep, Δz)`.
fn probe_arc_length(segments: &[Segment]) -> Vec<String> {
    let mut hits = Vec::new();
    for (i, s) in segments.iter().enumerate() {
        if s.kind != SegmentKind::Arc {
            continue;
        }
        let (Some([cx, cy]), Some(sx), Some(sy), Some(ex), Some(ey)) =
            (s.centre, s.start[0], s.start[1], s.end[0], s.end[1])
        else {
            continue; // malformed arcs are `arc-radius`'s business, not this rule's
        };
        let radius = (sx - cx).hypot(sy - cy).value();
        if !radius.is_finite() || radius <= 0.0 {
            continue;
        }
        let start_a = (sy - cy).atan2(sx - cx).value();
        let end_a = (ey - cy).atan2(ex - cx).value();
        let sweep = swept_delta(start_a, end_a, s.clockwise);
        let dz = match (s.start[2], s.end[2]) {
            (Some(z0), Some(z1)) => z1.value() - z0.value(),
            _ => 0.0,
        };
        let expected = (radius * sweep).hypot(dz);
        if hybrid_exceeds(s.length.value(), expected, 1e-6) {
            hits.push(format!(
                "seg {i}: length {:.9} vs hypot(r*sweep, dz) {:.9} (r={:.6}, sweep={:.6}, dz={:.6})",
                s.length.value(),
                expected,
                radius,
                sweep,
                dz
            ));
        }
    }
    hits
}

/// `filament-consistency` (§3.3): `volume/filament` constant per `tool`, relative 1e-6.
fn probe_filament_consistency(segments: &[Segment]) -> Vec<String> {
    let mut hits = Vec::new();
    let mut first_ratio: BTreeMap<Option<u32>, f64> = BTreeMap::new();
    for (i, s) in segments.iter().enumerate() {
        if s.travel || s.volume.value() <= 0.0 || s.filament.value() <= 0.0 {
            continue;
        }
        let ratio = s.volume.value() / s.filament.value();
        match first_ratio.get(&s.tool) {
            None => {
                first_ratio.insert(s.tool, ratio);
            }
            Some(&base) => {
                if (ratio - base).abs() > 1e-6 * base.abs().max(ratio.abs()) {
                    hits.push(format!(
                        "seg {i}: volume/filament {ratio:.9} vs tool {:?} baseline {base:.9}",
                        s.tool
                    ));
                }
            }
        }
    }
    hits
}

// ---------------------------------------------------------------------------------------------
// Corpus loading
// ---------------------------------------------------------------------------------------------

/// Every frozen toolpath we can reach, as `(corpus/name, toolpath)`.
fn load_corpora() -> Vec<(String, Toolpath)> {
    let root = repo_root();
    let mut out = Vec::new();

    // conformance/vectors/<name>/input.json — the IR is the whole document.
    let vectors = root.join("conformance/vectors");
    if let Ok(entries) = fs::read_dir(&vectors) {
        for e in entries.flatten() {
            let input = e.path().join("input.json");
            if !input.is_file() {
                continue;
            }
            let name = e.file_name().to_string_lossy().to_string();
            match serde_json::from_str::<Toolpath>(&fs::read_to_string(&input).unwrap()) {
                Ok(tp) => out.push((format!("vectors/{name}"), tp)),
                Err(err) => panic!("vectors/{name}/input.json did not parse as Toolpath: {err}"),
            }
        }
    }

    // conformance/gcode/*.json and conformance/gallery/*.json — the IR is under `.ir`.
    for (corpus, dir) in [
        ("gcode", root.join("conformance/gcode")),
        ("gallery", root.join("conformance/gallery")),
    ] {
        collect_ir_field(&dir, corpus, &mut out);
    }
    out
}

fn collect_ir_field(dir: &Path, corpus: &str, out: &mut Vec<(String, Toolpath)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
        let Some(ir) = doc.get("ir") else { continue };
        match serde_json::from_value::<Toolpath>(ir.clone()) {
            Ok(tp) => out.push((format!("{corpus}/{name}"), tp)),
            Err(err) => panic!("{corpus}/{name} `.ir` did not parse as Toolpath: {err}"),
        }
    }
}

#[test]
#[ignore = "H1.3 §5 report-only probe; run explicitly with --ignored --nocapture"]
fn probe_new_rules_against_the_frozen_corpora() {
    let corpora = load_corpora();
    assert!(
        corpora.len() >= 20,
        "probe loaded only {} toolpaths — corpus discovery is broken, and a probe that reads \
         nothing would report a false all-clear",
        corpora.len()
    );

    let rules: [(&str, RulePredicate); 5] = [
        ("continuity", probe_continuity),
        ("negative-quantity", probe_negative_quantity),
        ("segment-length", probe_segment_length),
        ("arc-length", probe_arc_length),
        ("filament-consistency", probe_filament_consistency),
    ];

    let mut total_segments = 0usize;
    let mut hit_count: BTreeMap<&str, usize> = BTreeMap::new();
    let mut fixtures_hit: BTreeMap<&str, usize> = BTreeMap::new();

    println!(
        "\n=== H1.3 §5 corpus probe: {} toolpaths ===\n",
        corpora.len()
    );
    for (name, tp) in &corpora {
        total_segments += tp.segments.len();
        for (rule, predicate) in rules {
            let hits = predicate(&tp.segments);
            if hits.is_empty() {
                continue;
            }
            *fixtures_hit.entry(rule).or_default() += 1;
            *hit_count.entry(rule).or_default() += hits.len();
            println!("[{rule}] {name} ({} segments)", tp.segments.len());
            for h in &hits {
                println!("    {h}");
            }
        }
    }

    println!("\n--- summary ---");
    println!(
        "{} toolpaths, {} segments probed",
        corpora.len(),
        total_segments
    );
    for (rule, _) in rules {
        println!(
            "  {rule:22} {} hits across {} fixtures",
            hit_count.get(rule).copied().unwrap_or(0),
            fixtures_hit.get(rule).copied().unwrap_or(0)
        );
    }
    println!(
        "\n§5 prediction: continuity, arc-length and filament-consistency fire nowhere here.\n\
         Any hit above must be triaged as (a) rule defect, (b) oracle divergence, or\n\
         (c) synthetic-fixture defect BEFORE the rule is wired into verify_stream.\n"
    );
}
