//! G-code forensics: infer slicer behavior from imported g-code and produce an **explainable** report.
//!
//! Every derived fact carries a [`Confidence`] tag so the report never presents a guess as a measurement:
//! - `from-comment` — taken directly from a slicer comment (e.g. a `;TYPE:` feature marker);
//! - `measured` — a deterministic count/sum over the motion (travel distance, segment count);
//! - `inferred` — an estimate derived from geometry (layer height, line width).
//!
//! This is a deterministic first cut (issue #29). Probabilistic inference (infill angle/spacing,
//! extrusion-multiplier recovery, seam/resonance modelling) is intentionally out of scope.

use crate::engine::segment_motion_time;
use crate::gcode::ImportedGcode;
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

/// The full forensics report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForensicsReport {
    pub slicer: String,
    pub source_lines: usize,
    pub segment_count: usize,
    pub layers: LayerModel,
    pub line_width_mm: Estimate,
    pub features: Vec<FeatureStat>,
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

        let line = imported.segment_source_lines.get(i).copied().unwrap_or(0);
        let feature = feature_at_line.get(line).copied().unwrap_or("other");
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
        features,
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
    }
}
