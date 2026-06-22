//! `verify` — check a resolved [`Toolpath`] against machine-safety **contracts** and structural
//! invariants, returning a located [`Report`] (`docs/01-architecture.md` §7). This is where Dry stops
//! merely *compiling* a toolpath and starts *catching* unsafe ones.
//!
//! The contracts are Dry's own, clean-room (each is a well-specified property of a safe toolpath, not a
//! reproduction of any oracle's wording):
//!  - **structural** (always checked): every quantity is finite; a travel deposits no material; an
//!    extruding move has a positive bead (`width`,`height` > 0).
//!  - **contract-driven** (checked when the contract supplies a limit): the move stays inside the build
//!    **bounds**; the volumetric **flow** stays under a ceiling; the feedrate stays within a **speed**
//!    range; **Z is monotonic** (non-decreasing) when required (e.g. vase mode).

use crate::engine::segment_motion_time;
use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::resolve::{catmull_rom, SAMPLES};
use crate::units::Length;
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// The limits a toolpath is checked against. An unset (`None`/`false`) field disables that check.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Contracts {
    /// Build volume as `[[x_lo, x_hi], [y_lo, y_hi], [z_lo, z_hi]]` (mm).
    pub bounds: Option<[[f64; 2]; 3]>,
    /// Maximum volumetric flow rate (mm³/s).
    pub max_flow: Option<f64>,
    /// Allowed feedrate range `[min, max]` (mm/min) for extruding moves.
    pub speed_range: Option<[f64; 2]>,
    /// Require Z never to decrease along the path.
    pub monotonic_z: bool,
    /// Minimum nozzle temperature (°C) required to extrude (cold-extrusion guard).
    pub min_temp: Option<f64>,
    /// Maximum retraction distance (mm).
    pub max_retraction_distance: Option<f64>,
    /// Maximum retraction speed (mm/min).
    pub max_retraction_speed: Option<f64>,
    /// Maximum travel run distance without a retraction (mm).
    pub max_travel_without_retract: Option<f64>,
    /// Allowed Z height range `[min, max]` (mm) for the first layer.
    pub first_layer_height_range: Option<[f64; 2]>,
    /// Allowed speed range `[min, max]` (mm/min) for the first layer.
    pub first_layer_speed_range: Option<[f64; 2]>,
}

/// A user-facing contract configuration parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractParseError {
    message: String,
}

impl ContractParseError {
    fn new(message: impl Into<String>) -> Self {
        ContractParseError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ContractParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ContractParseError {}

fn parse_csv_f64s(name: &str, s: &str, expected: usize) -> Result<Vec<f64>, ContractParseError> {
    let values: Result<Vec<f64>, _> = s.split(',').map(|t| t.trim().parse::<f64>()).collect();
    let values = values.map_err(|e| ContractParseError::new(format!("bad {name} value: {e}")))?;
    if values.len() != expected {
        return Err(ContractParseError::new(format!(
            "{name} needs {expected} comma-separated numbers"
        )));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(ContractParseError::new(format!(
            "{name} values must all be finite"
        )));
    }
    Ok(values)
}

/// Parse `x0,x1,y0,y1,z0,z1` into build-volume bounds.
pub fn parse_bounds_csv(s: &str) -> Result<[[f64; 2]; 3], ContractParseError> {
    let v = parse_csv_f64s("bounds", s, 6)?;
    Ok([[v[0], v[1]], [v[2], v[3]], [v[4], v[5]]])
}

/// Parse `min,max` into an extruding-move feedrate range.
pub fn parse_speed_range_csv(s: &str) -> Result<[f64; 2], ContractParseError> {
    let v = parse_csv_f64s("speed range", s, 2)?;
    Ok([v[0], v[1]])
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The toolpath is unsafe / invalid.
    Error,
    /// Suspicious but not necessarily fatal.
    Warning,
}

/// One located issue found by [`verify`].
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// A stable kebab-case rule id (`bounds`, `max-flow`, `speed`, `monotonic-z`, `cold-extrusion`,
    /// `finite`, `travel-extrudes`, `bead`, `orientation-not-unit`).
    pub rule: String,
    pub severity: Severity,
    /// The offending segment index, if the finding is local to one move.
    pub segment: Option<usize>,
    /// A human-readable description.
    pub message: String,
}

/// The result of verifying a toolpath.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    /// True when there are no `Error`-severity findings.
    pub fn ok(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }
    /// The number of `Error`-severity findings.
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }
}

const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;

/// Per-segment volumetric flow (mm³/s), or `None` for a move with no duration.
fn flow(s: &Segment) -> Option<f64> {
    segment_motion_time(s).map(|time| (s.volume / time).value())
}

fn segment_numbers(s: &Segment) -> Vec<f64> {
    let mut nums = vec![
        s.speed.value(),
        s.length.value(),
        s.volume.value(),
        s.filament.value(),
    ];
    nums.extend(s.start.iter().flatten().map(|v| v.value()));
    nums.extend(s.end.iter().flatten().map(|v| v.value()));
    if let Some(w) = s.width {
        nums.push(w.value());
    }
    if let Some(h) = s.height {
        nums.push(h.value());
    }
    if let Some([cx, cy]) = s.centre {
        nums.push(cx.value());
        nums.push(cy.value());
    }
    nums.extend(
        [s.temperature, s.fan, s.flow, s.dwell_s]
            .into_iter()
            .flatten(),
    );
    if let Some(o) = s.orientation {
        nums.extend(o);
    }
    if let Some(points) = &s.control_points {
        for p in points {
            nums.extend(p.iter().map(|v| v.value()));
        }
    }
    nums
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

fn delta_to_angle(start: f64, angle: f64, clockwise: bool) -> f64 {
    normalised_angle(if clockwise {
        start - angle
    } else {
        angle - start
    })
}

fn push_arc_bounds_points(s: &Segment, points: &mut Vec<[Option<Length>; 3]>) {
    let Some([cx, cy]) = s.centre else {
        return;
    };
    let (Some(sx), Some(sy), Some(ex), Some(ey)) = (s.start[0], s.start[1], s.end[0], s.end[1])
    else {
        return;
    };
    let radius = (sx - cx).hypot(sy - cy).value();
    if !radius.is_finite() {
        return;
    }
    let start_a = (sy - cy).atan2(sx - cx).value();
    let end_a = (ey - cy).atan2(ex - cx).value();
    let sweep = swept_delta(start_a, end_a, s.clockwise);

    for angle in [0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2] {
        let delta = delta_to_angle(start_a, angle, s.clockwise);
        if delta <= sweep + 1e-12 {
            let z = match (s.start[2], s.end[2]) {
                (Some(z0), Some(z1)) => Some(Length::mm(
                    z0.value() + (z1.value() - z0.value()) * (delta / sweep),
                )),
                _ => None,
            };
            points.push([
                Some(Length::mm(cx.value() + radius * libm::cos(angle))),
                Some(Length::mm(cy.value() + radius * libm::sin(angle))),
                z,
            ]);
        }
    }
}

fn push_spline_bounds_points(s: &Segment, points: &mut Vec<[Option<Length>; 3]>) {
    let Some(control_points) = &s.control_points else {
        return;
    };
    let start = [
        s.start[0].unwrap_or(Length::ZERO).value(),
        s.start[1].unwrap_or(Length::ZERO).value(),
        s.start[2].unwrap_or(Length::ZERO).value(),
    ];
    points.push([
        Some(Length::mm(start[0])),
        Some(Length::mm(start[1])),
        Some(Length::mm(start[2])),
    ]);
    let mut through = Vec::with_capacity(control_points.len() + 1);
    through.push(start);
    through.extend(
        control_points
            .iter()
            .map(|p| [p[0].value(), p[1].value(), p[2].value()]),
    );

    for i in 0..through.len().saturating_sub(1) {
        let p0 = through[i.saturating_sub(1)];
        let p1 = through[i];
        let p2 = through[i + 1];
        let p3 = through[(i + 2).min(through.len() - 1)];
        for step in 1..=SAMPLES {
            let pt = if step == SAMPLES {
                p2
            } else {
                catmull_rom(p0, p1, p2, p3, step as f64 / SAMPLES as f64)
            };
            points.push([
                Some(Length::mm(pt[0])),
                Some(Length::mm(pt[1])),
                Some(Length::mm(pt[2])),
            ]);
        }
    }
}

fn bounds_points(s: &Segment) -> Vec<[Option<Length>; 3]> {
    let mut points = vec![s.start, s.end];
    if s.kind == SegmentKind::Arc {
        push_arc_bounds_points(s, &mut points);
    } else if s.kind == SegmentKind::Spline {
        push_spline_bounds_points(s, &mut points);
    }
    points
}

fn arc_radius_error(s: &Segment) -> Option<String> {
    if s.kind != SegmentKind::Arc {
        return None;
    }
    let Some([cx, cy]) = s.centre else {
        return Some("arc segment is missing centre".to_string());
    };
    let (Some(sx), Some(sy), Some(ex), Some(ey)) = (s.start[0], s.start[1], s.end[0], s.end[1])
    else {
        return Some("arc segment needs defined start and end X/Y".to_string());
    };
    let start_radius = (sx - cx).hypot(sy - cy).value();
    let end_radius = (ex - cx).hypot(ey - cy).value();
    if start_radius <= 0.0 || end_radius <= 0.0 {
        return Some("arc segment needs a non-zero radius".to_string());
    }
    let tolerance = ARC_RADIUS_TOLERANCE_MM * start_radius.max(end_radius).max(1.0);
    let delta = (start_radius - end_radius).abs();
    if delta > tolerance {
        Some(format!(
            "arc endpoint radius differs from start radius by {delta:.6} mm"
        ))
    } else {
        None
    }
}

/// Verify a stream of segments against the contracts, returning all findings (structural + contract-driven).
pub fn verify_stream<I>(segments: I, c: &Contracts) -> Result<Report, crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<Segment, crate::codec::CodecError>>,
{
    let mut r = Report::default();
    let mut push = |rule: &str, severity, segment, message: String| {
        r.findings.push(Finding {
            rule: rule.to_string(),
            severity,
            segment,
            message,
        });
    };
    let axis = ['X', 'Y', 'Z'];

    let segments_vec: Vec<Segment> = segments.into_iter().collect::<Result<_, _>>()?;

    let first_layer_z = segments_vec
        .iter()
        .filter(|s| !s.travel && s.volume.value() > 0.0)
        .filter_map(|s| s.end[2].or(s.start[2]))
        .map(|z| z.value())
        .fold(f64::INFINITY, |a, b| if b < a { b } else { a });

    let mut travel_run_length = 0.0;
    let mut retracted = true;
    let mut flagged_travel = false;

    for (i, s) in segments_vec.into_iter().enumerate() {
        // --- structural invariants (always on) ---
        let nums = segment_numbers(&s);
        if nums.iter().any(|v| !v.is_finite()) {
            push(
                "finite",
                Severity::Error,
                Some(i),
                "segment carries a non-finite value".into(),
            );
        }
        if s.travel && s.volume.value() > 0.0 {
            push(
                "travel-extrudes",
                Severity::Error,
                Some(i),
                format!(
                    "travel move deposits {:.4} mm³ (should be 0)",
                    s.volume.value()
                ),
            );
        }
        if !s.travel && s.length > Length::ZERO {
            let w = s.width.map(|l| l.value()).unwrap_or(0.0);
            let h = s.height.map(|l| l.value()).unwrap_or(0.0);
            if w <= 0.0 || h <= 0.0 {
                push(
                    "bead",
                    Severity::Error,
                    Some(i),
                    format!("extruding move has a non-positive bead (width {w}, height {h})"),
                );
            }
        }
        if let Some([x, y, z]) = s.orientation {
            // the toolframe orientation must be a unit direction vector.
            let mag = libm::sqrt(x * x + y * y + z * z);
            if (mag - 1.0).abs() > 1e-6 {
                push(
                    "orientation-not-unit",
                    Severity::Error,
                    Some(i),
                    format!(
                        "toolframe orientation [{x}, {y}, {z}] has magnitude {mag} (must be 1)"
                    ),
                );
            }
        }
        if let Some(message) = arc_radius_error(&s) {
            push("arc-radius", Severity::Error, Some(i), message);
        }

        // --- contract-driven checks ---
        if let Some(b) = c.bounds {
            'points: for point in bounds_points(&s) {
                for (k, coord) in point.iter().enumerate() {
                    if let Some(v) = coord {
                        let v = v.value();
                        if v < b[k][0] || v > b[k][1] {
                            push(
                                "bounds",
                                Severity::Error,
                                Some(i),
                                format!(
                                    "{} = {v} is outside the build volume [{}, {}]",
                                    axis[k], b[k][0], b[k][1]
                                ),
                            );
                            break 'points; // one bounds finding per segment
                        }
                    }
                }
            }
        }
        if let (Some(max), Some(f)) = (c.max_flow, flow(&s)) {
            if f > max {
                push(
                    "max-flow",
                    Severity::Error,
                    Some(i),
                    format!("flow {f:.3} mm³/s exceeds the ceiling {max:.3}"),
                );
            }
        }
        if let Some([lo, hi]) = c.speed_range {
            if !s.travel && s.length.value() > 0.0 && s.volume.value() > 0.0 {
                let v = s.speed.value();
                if v < lo || v > hi {
                    push(
                        "speed",
                        Severity::Error,
                        Some(i),
                        format!("feedrate {v} is outside [{lo}, {hi}] mm/min"),
                    );
                }
            }
        }
        if c.monotonic_z {
            if let (Some(z0), Some(z1)) = (s.start[2], s.end[2]) {
                if z1 < z0 {
                    push(
                        "monotonic-z",
                        Severity::Error,
                        Some(i),
                        format!("Z decreases from {} to {}", z0.value(), z1.value()),
                    );
                }
            }
        }
        if let Some(min) = c.min_temp {
            // an extruding move below the minimum nozzle temperature (or with none set) is cold extrusion.
            if !s.travel && s.volume.value() > 0.0 && s.temperature.map(|t| t < min).unwrap_or(true)
            {
                let got = s
                    .temperature
                    .map(|t| format!("{t}"))
                    .unwrap_or_else(|| "unset".into());
                push(
                    "cold-extrusion",
                    Severity::Error,
                    Some(i),
                    format!("extruding at nozzle temperature {got} (< {min} °C)"),
                );
            }
        }

        // --- retraction checks ---
        let is_retract = s.filament.value() < 0.0;
        let is_unretract = s.filament.value() > 0.0 && s.length.value() == 0.0;
        if is_retract || is_unretract {
            if let Some(max_speed) = c.max_retraction_speed {
                if s.speed.value() > max_speed {
                    push(
                        "retraction-speed",
                        Severity::Error,
                        Some(i),
                        format!(
                            "retraction speed {} mm/min exceeds the limit of {}",
                            s.speed.value(),
                            max_speed
                        ),
                    );
                }
            }
        }
        if is_retract {
            let dist = -s.filament.value();
            if let Some(max_dist) = c.max_retraction_distance {
                if dist > max_dist {
                    push(
                        "retraction-distance",
                        Severity::Error,
                        Some(i),
                        format!(
                            "retraction distance {dist:.3} mm exceeds the limit of {max_dist:.3}"
                        ),
                    );
                }
            }
            retracted = true;
        } else if is_unretract || (!s.travel && s.length.value() > 0.0 && s.volume.value() > 0.0) {
            retracted = false;
            travel_run_length = 0.0;
            flagged_travel = false;
        } else if s.travel {
            travel_run_length += s.length.value();
            if let Some(max_travel) = c.max_travel_without_retract {
                if travel_run_length > max_travel && !retracted && !flagged_travel {
                    push(
                        "travel-without-retraction",
                        Severity::Error,
                        Some(i),
                        format!(
                            "travel run distance {travel_run_length:.3} mm exceeds limit of {max_travel:.3} without retraction"
                        ),
                    );
                    flagged_travel = true;
                }
            }
        }

        // --- first-layer checks ---
        if !s.travel && s.volume.value() > 0.0 {
            let is_first_layer = if first_layer_z.is_finite() {
                if let Some(z) = s.end[2].or(s.start[2]) {
                    (z.value() - first_layer_z).abs() < 1e-4
                } else {
                    false
                }
            } else {
                false
            };

            if is_first_layer {
                if let Some([min_h, max_h]) = c.first_layer_height_range {
                    let h_val = s.height.map(|h| h.value()).unwrap_or(first_layer_z);
                    if h_val < min_h || h_val > max_h {
                        push(
                            "first-layer-height",
                            Severity::Error,
                            Some(i),
                            format!(
                                "first layer height {h_val:.3} mm is outside the range [{min_h:.3}, {max_h:.3}]"
                            ),
                        );
                    }
                }
                if let Some([min_s, max_s]) = c.first_layer_speed_range {
                    let speed_val = s.speed.value();
                    if speed_val < min_s || speed_val > max_s {
                        push(
                            "first-layer-speed",
                            Severity::Error,
                            Some(i),
                            format!(
                                "first layer speed {speed_val:.3} mm/min is outside the range [{min_s:.3}, {max_s:.3}]"
                            ),
                        );
                    }
                }
            }
        }
    }
    Ok(r)
}

/// Verify a resolved [`Toolpath`] against machine-safety **contracts** and structural
/// invariants, returning a located [`Report`].
pub fn verify(tp: &Toolpath, c: &Contracts) -> Report {
    verify_stream(tp.segments.iter().cloned().map(Ok), c).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toolpath_is_ok() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert!(verify(&tp, &Contracts::default()).ok());
    }
}
