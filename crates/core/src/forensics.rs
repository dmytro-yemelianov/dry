//! G-code forensics: infer slicer behavior from imported g-code and produce an **explainable** report.
//!
//! Every derived fact carries a [`Confidence`] tag so the report never presents a guess as a measurement:
//! - `from-comment` — taken directly from a slicer comment (e.g. a `;TYPE:` feature marker);
//! - `measured` — a deterministic count/sum over the motion (travel distance, segment count);
//! - `inferred` — an estimate derived from geometry (layer height, line width).
//!
//! The probabilistic layer (issue #29 rounds 2–3): declared-settings extraction from config comments,
//! infill-angle and -spacing inference from geometry, an extrusion-multiplier estimate, and a
//! seam-strategy hint. Still out of scope: resonance modelling and Cura's base64 config block.

use crate::engine::segment_motion_time;
use crate::gcode::ImportedGcode;
use crate::ir::Segment;
use serde::{Deserialize, Serialize};

/// How a reported fact was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Confidence {
    /// A deterministic count/sum over the motion.
    Measured,
    /// Taken directly from a slicer comment.
    FromComment,
    /// An estimate derived from geometry.
    Inferred,
}

/// A numeric estimate with its provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Estimate {
    /// `None` when it could not be determined.
    pub value: Option<f64>,
    pub confidence: Confidence,
    pub note: String,
}

/// Aggregated motion statistics for one feature class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStat {
    pub feature: String,
    pub source: Confidence,
    pub segments: usize,
    pub extruding_distance_mm: f64,
    pub time_s: f64,
    pub min_speed_mm_min: f64,
    pub max_speed_mm_min: f64,
    pub peak_flow_mm3_s: f64,
}

/// The layer model inferred from extruding-move Z levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerModel {
    pub layer_count: usize,
    pub layer_height_mm: Estimate,
}

/// Travel / retraction statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelStat {
    pub travel_moves: usize,
    pub travel_distance_mm: f64,
    pub retractions: usize,
}

/// A flagged pattern worth attention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    pub kind: String,
    pub count: usize,
    pub note: String,
}

/// Slicer settings extracted verbatim from `; key = value` config comments (PrusaSlicer family).
/// Every present field is `from-comment`; `None` means the setting was not declared in the file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeclaredSettings {
    pub layer_height_mm: Option<f64>,
    pub extrusion_width_mm: Option<f64>,
    pub infill_angle_deg: Option<f64>,
    pub infill_density: Option<String>,
}

/// A seam-placement hint inferred from where outer-wall loops start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeamHint {
    /// `aligned` (< 1 mm spread), `clustered` (< 5 mm), `scattered`, or `unknown` (< 2 loops).
    pub strategy: String,
    pub loops: usize,
    pub source: Confidence,
}

/// The full forensics report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicsReport {
    pub slicer: String,
    pub source_lines: usize,
    pub segment_count: usize,
    pub layers: LayerModel,
    pub line_width_mm: Estimate,
    /// Slicer settings read from config comments (all `from-comment`).
    pub declared: DeclaredSettings,
    pub features: Vec<FeatureStat>,
    /// Dominant infill directions in degrees (mod 180), inferred from geometry; empty if no infill.
    pub infill_angles_deg: Vec<f64>,
    /// Perpendicular spacing between parallel infill lines (mm), inferred.
    pub infill_spacing_mm: Estimate,
    /// Effective extrusion multiplier, inferred from deposited vs. nominal-bead volume.
    pub extrusion_multiplier: Estimate,
    /// Seam-placement hint inferred from outer-wall loop starts.
    pub seam: SeamHint,
    pub travel: TravelStat,
    pub hotspots: Vec<Hotspot>,
}

const TINY_SEGMENT_MM: f64 = 0.5;

/// Detect the slicer from header comments (best-effort; `"unknown"` when no signature matches).
fn detect_slicer(lines: &[String]) -> String {
    let hay = lines
        .iter()
        .take(60)
        .map(|l| l.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    // Most specific first.
    for (needle, name) in [
        ("superslicer", "SuperSlicer"),
        ("prusaslicer", "PrusaSlicer"),
        ("orcaslicer", "OrcaSlicer"),
        ("bambustudio", "BambuStudio"),
        ("simplify3d", "Simplify3D"),
        ("ideamaker", "ideaMaker"),
        ("cura", "Cura"),
        ("kisslicer", "KISSlicer"),
        ("slic3r", "Slic3r"),
    ] {
        if hay.contains(needle) {
            return name.to_string();
        }
    }
    "unknown".to_string()
}

/// If `line` is a feature marker (`;TYPE:…`, `; FEATURE: …`, `; feature …`), return its payload.
/// Handles Cura/PrusaSlicer (`;TYPE:`), Orca/Bambu (`; FEATURE:`) and Simplify3D (`; feature `),
/// case-insensitively.
fn feature_marker_payload(line: &str) -> Option<&str> {
    let body = line.trim().strip_prefix(';')?.trim_start();
    let upper = body.to_ascii_uppercase();
    for key in ["TYPE:", "FEATURE:", "FEATURE "] {
        if upper.starts_with(key) {
            let payload = body[key.len()..].trim(); // key is ASCII, so byte length matches
            if !payload.is_empty() {
                return Some(payload);
            }
        }
    }
    None
}

/// Map a slicer feature-marker payload onto a canonical class (order matters — most specific first).
fn canonical_feature(payload: &str) -> &'static str {
    let p = payload.to_ascii_lowercase();
    let has = |s: &str| p.contains(s);
    if has("outer") || has("external perimeter") {
        "outer-wall"
    } else if has("wall-inner") || has("inner") || has("perimeter") {
        "inner-wall"
    } else if has("bridge") {
        "bridge"
    } else if has("solid") {
        "solid-infill"
    } else if has("skin") || has("top surface") || has("top solid") || has("bottom") {
        "top-bottom"
    } else if has("support") {
        "support"
    } else if has("skirt") || has("brim") {
        "skirt-brim"
    } else if has("fill") || has("infill") || has("sparse") {
        "infill"
    } else if has("travel") {
        "travel"
    } else {
        "other"
    }
}

fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = values.len();
    Some(if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    })
}

/// Extract slicer settings from `; key = value` config comments (PrusaSlicer / SuperSlicer / Orca).
fn parse_declared_settings(lines: &[String]) -> DeclaredSettings {
    let mut d = DeclaredSettings::default();
    let num = |v: &str| {
        v.trim()
            .trim_end_matches('%')
            .split(',')
            .next()
            .and_then(|x| x.trim().parse::<f64>().ok())
    };
    for line in lines {
        let Some(body) = line.trim().strip_prefix(';') else {
            continue;
        };
        let Some((k, v)) = body.split_once('=') else {
            continue;
        };
        let (key, val) = (k.trim().to_ascii_lowercase(), v.trim());
        match key.as_str() {
            "layer_height" => d.layer_height_mm = num(val).or(d.layer_height_mm),
            "extrusion_width" => d.extrusion_width_mm = num(val).or(d.extrusion_width_mm),
            "perimeter_extrusion_width" | "infill_extrusion_width" => {
                d.extrusion_width_mm = d.extrusion_width_mm.or_else(|| num(val))
            }
            "fill_angle" | "infill_angle" => d.infill_angle_deg = num(val).or(d.infill_angle_deg),
            "fill_density" | "infill_density" if !val.is_empty() => {
                d.infill_density = d.infill_density.take().or_else(|| Some(val.to_string()))
            }
            _ => {}
        }
    }
    d
}

/// The dominant infill direction(s) in degrees (mod 180), via a 5° histogram. Empty when too few
/// samples or no clear mode. Returns up to two modes (e.g. alternating 45°/135° infill).
fn dominant_angles(angles: Vec<f64>) -> Vec<f64> {
    const BIN: f64 = 5.0;
    let nbins = (180.0 / BIN) as usize;
    if angles.len() < 3 {
        return Vec::new();
    }
    let mut bins: Vec<Vec<f64>> = vec![Vec::new(); nbins];
    for a in &angles {
        let idx = ((a / BIN) as usize).min(nbins - 1);
        bins[idx].push(*a);
    }
    let maxc = bins.iter().map(|b| b.len()).max().unwrap_or(0);
    if maxc == 0 {
        return Vec::new();
    }
    let mut idxs: Vec<usize> = (0..nbins)
        .filter(|&i| !bins[i].is_empty() && bins[i].len() >= maxc.div_ceil(2))
        .collect();
    idxs.sort_by_key(|&i| std::cmp::Reverse(bins[i].len()));
    idxs.truncate(2);
    let mut out: Vec<f64> = idxs
        .into_iter()
        .map(|i| {
            let mut v = bins[i].clone();
            (median(&mut v).unwrap() * 10.0).round() / 10.0
        })
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Median perpendicular spacing between parallel infill lines, plus the gap coefficient-of-variation
/// (a regularity signal). Needs the dominant infill angle and ≥ 2 distinct parallel lines.
fn infill_spacing(mids: &[(f64, f64)], angle_deg: Option<f64>) -> Option<(f64, f64)> {
    let angle = angle_deg?;
    if mids.len() < 3 {
        return None;
    }
    let perp = (angle + 90.0).to_radians();
    let (c, s) = (libm::cos(perp), libm::sin(perp));
    let mut offs: Vec<f64> = mids.iter().map(|(x, y)| x * c + y * s).collect();
    offs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut lines: Vec<f64> = Vec::new();
    for o in offs {
        if lines.last().is_none_or(|l| (o - l).abs() > 0.05) {
            lines.push(o);
        }
    }
    if lines.len() < 2 {
        return None;
    }
    let gaps: Vec<f64> = lines.windows(2).map(|w| w[1] - w[0]).collect();
    let mut g = gaps.clone();
    let spacing = median(&mut g)?;
    let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
    let var = gaps.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / gaps.len() as f64;
    let cv = if mean.abs() > 1e-9 {
        libm::sqrt(var) / mean
    } else {
        0.0
    };
    Some((spacing, cv))
}

/// Classify the seam from where outer-wall loops start: a loop start is an outer-wall extruding segment
/// whose predecessor was not outer-wall-extruding.
fn seam_hint(segments: &[Segment], feature_at_line: &[&str], source_lines: &[usize]) -> SeamHint {
    let mut starts: Vec<(f64, f64)> = Vec::new();
    let mut prev_outer = false;
    for (i, s) in segments.iter().enumerate() {
        let line = source_lines.get(i).copied().unwrap_or(0);
        let feature = feature_at_line.get(line).copied().unwrap_or("other");
        let is_outer = !s.travel && s.volume.value() > 0.0 && feature == "outer-wall";
        if is_outer && !prev_outer {
            if let (Some(x), Some(y)) = (s.start[0].or(s.end[0]), s.start[1].or(s.end[1])) {
                starts.push((x.value(), y.value()));
            }
        }
        prev_outer = is_outer;
    }
    let loops = starts.len();
    if loops < 2 {
        return SeamHint {
            strategy: "unknown".to_string(),
            loops,
            source: Confidence::Inferred,
        };
    }
    let cx = starts.iter().map(|p| p.0).sum::<f64>() / loops as f64;
    let cy = starts.iter().map(|p| p.1).sum::<f64>() / loops as f64;
    let maxd = starts
        .iter()
        .map(|(x, y)| libm::hypot(x - cx, y - cy))
        .fold(0.0, f64::max);
    let strategy = if maxd < 1.0 {
        "aligned"
    } else if maxd < 5.0 {
        "clustered"
    } else {
        "scattered"
    };
    SeamHint {
        strategy: strategy.to_string(),
        loops,
        source: Confidence::Inferred,
    }
}

#[derive(Default, Clone)]
struct Agg {
    segments: usize,
    distance: f64,
    time: f64,
    min_speed: f64,
    max_speed: f64,
    peak_flow: f64,
    seen: bool,
}

/// Analyze imported g-code into a [`ForensicsReport`].
pub fn analyze(imported: &ImportedGcode) -> ForensicsReport {
    let lines = &imported.source_lines;
    let slicer = detect_slicer(lines);
    let declared = parse_declared_settings(lines);

    // Active feature per 1-based source line: carry the most recent marker forward.
    let mut feature_at_line: Vec<&'static str> = vec!["other"; lines.len() + 2];
    let mut any_marker = false;
    let mut current = "other";
    for (idx, line) in lines.iter().enumerate() {
        if let Some(payload) = feature_marker_payload(line) {
            current = canonical_feature(payload);
            any_marker = true;
        }
        feature_at_line[idx + 1] = current; // source lines are 1-based
    }

    let segments = &imported.toolpath.segments;
    let mut feats: std::collections::BTreeMap<&'static str, Agg> =
        std::collections::BTreeMap::new();
    let mut zs: Vec<f64> = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    let mut travel_moves = 0usize;
    let mut travel_distance = 0.0;
    let mut retractions = 0usize;
    let mut tiny = 0usize;
    let mut infill_dirs: Vec<f64> = Vec::new();
    let mut infill_mids: Vec<(f64, f64)> = Vec::new();
    let mut mults: Vec<f64> = Vec::new();

    // First pass: layer Z set (for the line-width estimate that needs layer height).
    for s in segments {
        if !s.travel && s.volume.value() > 0.0 {
            if let Some(z) = s.end[2].or(s.start[2]) {
                zs.push(z.value());
            }
        }
    }
    zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    zs.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    let mut z_deltas: Vec<f64> = zs.windows(2).map(|w| w[1] - w[0]).collect();
    let layer_height = median(&mut z_deltas);

    // Nominal bead for the extrusion-multiplier estimate (needs a *declared* width to be meaningful).
    let nominal_w = declared.extrusion_width_mm.filter(|w| *w > 0.0);
    let nominal_h = declared
        .layer_height_mm
        .or(layer_height)
        .filter(|h| *h > 0.0);

    for (i, s) in segments.iter().enumerate() {
        let speed = s.speed.value();
        let time = segment_motion_time(s).map(|t| t.value()).unwrap_or(0.0);

        if s.filament.value() < 0.0 {
            retractions += 1;
        }
        if s.travel {
            travel_moves += 1;
            travel_distance += s.length.value();
            continue; // travel handled by TravelStat, not per-feature
        }
        if s.volume.value() <= 0.0 {
            continue; // non-extruding, non-travel (dwell/retract/etc.)
        }

        let len = s.length.value();
        if len > 0.0 && len < TINY_SEGMENT_MM {
            tiny += 1;
        }
        if let Some(lh) = layer_height {
            if len > 0.0 && lh > 0.0 {
                widths.push(s.volume.value() / (len * lh));
            }
        }
        if let (Some(w), Some(h)) = (nominal_w, nominal_h) {
            if len > 0.0 {
                mults.push(s.volume.value() / (w * h * len));
            }
        }

        let line = imported.segment_source_lines.get(i).copied().unwrap_or(0);
        let feature = feature_at_line.get(line).copied().unwrap_or("other");

        if feature == "infill" && len > 0.0 {
            if let (Some(sx), Some(sy), Some(ex), Some(ey)) =
                (s.start[0], s.start[1], s.end[0], s.end[1])
            {
                let mut deg = libm::atan2(ey.value() - sy.value(), ex.value() - sx.value())
                    .to_degrees()
                    .rem_euclid(180.0);
                if deg >= 180.0 {
                    deg -= 180.0;
                }
                infill_dirs.push(deg);
                infill_mids.push((
                    (sx.value() + ex.value()) / 2.0,
                    (sy.value() + ey.value()) / 2.0,
                ));
            }
        }
        let a = feats.entry(feature).or_default();
        if !a.seen {
            a.seen = true;
            a.min_speed = speed;
            a.max_speed = speed;
        }
        a.segments += 1;
        a.distance += len;
        a.time += time;
        a.min_speed = a.min_speed.min(speed);
        a.max_speed = a.max_speed.max(speed);
        let flow = if time > 0.0 {
            s.volume.value() / time
        } else {
            0.0
        };
        a.peak_flow = a.peak_flow.max(flow);
    }

    let source = if any_marker {
        Confidence::FromComment
    } else {
        Confidence::Inferred
    };
    let features: Vec<FeatureStat> = feats
        .into_iter()
        .map(|(feature, a)| FeatureStat {
            feature: if any_marker {
                feature.to_string()
            } else {
                "unknown".to_string()
            },
            source,
            segments: a.segments,
            extruding_distance_mm: a.distance,
            time_s: a.time,
            min_speed_mm_min: a.min_speed,
            max_speed_mm_min: a.max_speed,
            peak_flow_mm3_s: a.peak_flow,
        })
        .collect();

    let mut hotspots = Vec::new();
    if tiny > 0 {
        hotspots.push(Hotspot {
            kind: "tiny-segments".to_string(),
            count: tiny,
            note: format!("{tiny} extruding moves shorter than {TINY_SEGMENT_MM} mm — possible planner load / resonance"),
        });
    }

    let extrusion_multiplier = if nominal_w.is_some() {
        Estimate {
            value: median(&mut mults),
            confidence: Confidence::Inferred,
            note: "median deposited / nominal-bead volume, using the declared extrusion width"
                .to_string(),
        }
    } else {
        Estimate {
            value: None,
            confidence: Confidence::Inferred,
            note: "needs a declared extrusion width (no slicer config block found)".to_string(),
        }
    };

    let infill_angles_deg = dominant_angles(infill_dirs);
    let infill_spacing_mm = match infill_spacing(&infill_mids, infill_angles_deg.first().copied()) {
        Some((spacing, cv)) => Estimate {
            value: Some(spacing),
            confidence: Confidence::Inferred,
            note: format!(
                "median perpendicular gap between parallel infill lines (gap CV {cv:.2})"
            ),
        },
        None => Estimate {
            value: None,
            confidence: Confidence::Inferred,
            note: "needs ≥ 2 parallel infill lines at a dominant angle".to_string(),
        },
    };
    let seam = seam_hint(segments, &feature_at_line, &imported.segment_source_lines);

    ForensicsReport {
        slicer,
        source_lines: lines.len(),
        segment_count: segments.len(),
        layers: LayerModel {
            layer_count: zs.len(),
            layer_height_mm: Estimate {
                value: layer_height,
                confidence: Confidence::Inferred,
                note: "median Z delta between extruding layers".to_string(),
            },
        },
        line_width_mm: Estimate {
            value: median(&mut widths),
            confidence: Confidence::Inferred,
            note: "median of volume / (length × layer-height) over extruding moves".to_string(),
        },
        declared,
        features,
        infill_angles_deg,
        infill_spacing_mm,
        extrusion_multiplier,
        seam,
        travel: TravelStat {
            travel_moves,
            travel_distance_mm: travel_distance,
            retractions,
        },
        hotspots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gcode::{import_gcode_with_map, GcodeImportParams};

    const CURA: &str = "\
;FLAVOR:Marlin
;Generated with Cura_SteamEngine 5.0
M140 S60
M104 S210
G28
G90
M83
;LAYER:0
G1 Z0.2 F600
;TYPE:WALL-OUTER
G1 X0 Y0 F9000
G1 X20 Y0 E0.8 F1200
G1 X20 Y20 E0.8
;TYPE:FILL
G1 X2 Y2 F9000
G1 X18 Y18 E0.5 F1800
;LAYER:1
G1 Z0.4 F600
;TYPE:WALL-OUTER
G1 X0 Y0 F9000
G1 X20 Y0 E0.8 F1200
;TYPE:FILL
G1 X2 Y2 F9000
G1 X18 Y18 E0.5 F1800
M104 S0
";

    #[test]
    fn attributes_features_from_cura_markers() {
        let imported = import_gcode_with_map(CURA, &GcodeImportParams::default()).unwrap();
        let r = analyze(&imported);
        assert_eq!(r.slicer, "Cura");
        assert_eq!(r.layers.layer_count, 2);
        let features: Vec<&str> = r.features.iter().map(|f| f.feature.as_str()).collect();
        assert!(features.contains(&"outer-wall"), "{features:?}");
        assert!(features.contains(&"infill"), "{features:?}");
        for f in &r.features {
            assert_eq!(f.source, Confidence::FromComment);
        }
        assert!(r.travel.travel_moves >= 2);
        assert!(r.layers.layer_height_mm.value.unwrap() > 0.0);
    }

    #[test]
    fn marker_less_gcode_is_graceful() {
        let plain = "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X20 Y0 E0.8 F1200\n";
        let imported = import_gcode_with_map(plain, &GcodeImportParams::default()).unwrap();
        let r = analyze(&imported);
        assert_eq!(r.slicer, "unknown");
        // no markers -> a single inferred "unknown" feature bucket
        assert!(r.features.iter().all(|f| f.feature == "unknown"));
        assert!(r.features.iter().all(|f| f.source == Confidence::Inferred));
        // no config block -> no declared settings, no recoverable multiplier
        assert!(r.declared.layer_height_mm.is_none());
        assert!(r.extrusion_multiplier.value.is_none());
    }

    const PRUSA: &str = "\
; generated by PrusaSlicer 2.7.0
M83
;Z:0.2
;TYPE:External perimeter
G1 X0 Y0 F9000
G1 X10 Y0 E0.4 F1200
G1 X10 Y10 E0.4
G1 X0 Y10 E0.4
G1 X0 Y0 E0.4
;TYPE:Internal infill
G1 X0 Y0 F9000
G1 X10 Y10 E0.6 F1800
G1 X2 Y0 F9000
G1 X10 Y8 E0.5 F1800
G1 X0 Y2 F9000
G1 X8 Y10 E0.5 F1800
G1 X4 Y0 F9000
G1 X10 Y6 E0.4 F1800
; layer_height = 0.2
; extrusion_width = 0.45
; fill_angle = 45
; fill_density = 20%
";

    #[test]
    fn round2_declared_settings_infill_angle_and_multiplier() {
        let imported = import_gcode_with_map(PRUSA, &GcodeImportParams::default()).unwrap();
        let r = analyze(&imported);
        assert_eq!(r.slicer, "PrusaSlicer");

        // declared settings from the config block (from-comment).
        assert_eq!(r.declared.layer_height_mm, Some(0.2));
        assert_eq!(r.declared.extrusion_width_mm, Some(0.45));
        assert_eq!(r.declared.infill_angle_deg, Some(45.0));
        assert_eq!(r.declared.infill_density.as_deref(), Some("20%"));

        // infill direction inferred from geometry (the diagonals are 45°).
        assert!(!r.infill_angles_deg.is_empty(), "no infill angle inferred");
        assert!(
            r.infill_angles_deg.iter().any(|a| (a - 45.0).abs() < 6.0),
            "{:?}",
            r.infill_angles_deg
        );

        // multiplier recoverable now that a nominal width is declared.
        assert!(r.extrusion_multiplier.value.is_some());

        // External perimeter -> outer-wall, Internal infill -> infill.
        let features: Vec<&str> = r.features.iter().map(|f| f.feature.as_str()).collect();
        assert!(features.contains(&"outer-wall"), "{features:?}");
        assert!(features.contains(&"infill"), "{features:?}");

        // round 3: spacing between the four parallel 45° infill lines.
        assert!(
            r.infill_spacing_mm.value.unwrap() > 0.0,
            "{:?}",
            r.infill_spacing_mm
        );
    }

    // Two layers whose outer-wall loops both start at (0,0) → an aligned seam.
    const ALIGNED_SEAM: &str = "\
;Generated with Cura_SteamEngine 5.0
M83
;LAYER:0
G1 Z0.2 F600
;TYPE:WALL-OUTER
G1 X0 Y0 F9000
G1 X10 Y0 E0.4 F1200
G1 X10 Y10 E0.4
G1 X0 Y10 E0.4
G1 X0 Y0 E0.4
;LAYER:1
G1 Z0.4 F600
;TYPE:WALL-OUTER
G1 X0 Y0 F9000
G1 X10 Y0 E0.4 F1200
G1 X10 Y10 E0.4
G1 X0 Y10 E0.4
G1 X0 Y0 E0.4
";

    #[test]
    fn seam_is_aligned_when_loops_share_a_start() {
        let imported = import_gcode_with_map(ALIGNED_SEAM, &GcodeImportParams::default()).unwrap();
        let r = analyze(&imported);
        assert_eq!(r.seam.loops, 2, "{:?}", r.seam);
        assert_eq!(r.seam.strategy, "aligned", "{:?}", r.seam);
        assert_eq!(r.seam.source, Confidence::Inferred);
    }

    #[test]
    fn seam_unknown_with_fewer_than_two_loops() {
        let single = "\
;Generated with Cura_SteamEngine 5.0
M83
;TYPE:WALL-OUTER
G1 X0 Y0 F9000
G1 X10 Y0 E0.4 F1200
G1 X10 Y10 E0.4
G1 X0 Y0 E0.4
";
        let imported = import_gcode_with_map(single, &GcodeImportParams::default()).unwrap();
        let r = analyze(&imported);
        assert_eq!(r.seam.loops, 1);
        assert_eq!(r.seam.strategy, "unknown");
    }
}
