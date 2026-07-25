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
    /// Kinematic limits for the peak-acceleration / junction-velocity rules. `None` disables them.
    #[serde(default)]
    pub kinematics: Option<KinematicContracts>,
}

/// Kinematic limits checked by the `peak-acceleration` (arc centripetal) and `junction-velocity`
/// (per-junction Δv) rules. An unset field disables its check.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct KinematicContracts {
    pub max_acceleration_mm_s2: Option<f64>,
    pub max_junction_velocity_mm_s: Option<f64>,
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

fn validate_range_order(name: &str, [lo, hi]: [f64; 2]) -> Result<(), ContractParseError> {
    if lo > hi {
        return Err(ContractParseError::new(format!(
            "{name} lower bound must be <= upper bound"
        )));
    }
    Ok(())
}

/// Parse `x0,x1,y0,y1,z0,z1` into build-volume bounds.
pub fn parse_bounds_csv(s: &str) -> Result<[[f64; 2]; 3], ContractParseError> {
    let v = parse_csv_f64s("bounds", s, 6)?;
    let bounds = [[v[0], v[1]], [v[2], v[3]], [v[4], v[5]]];
    for (axis, range) in ["x", "y", "z"].into_iter().zip(bounds) {
        validate_range_order(&format!("bounds {axis}"), range)?;
    }
    Ok(bounds)
}

/// Parse `min,max` into an extruding-move feedrate range.
pub fn parse_speed_range_csv(s: &str) -> Result<[f64; 2], ContractParseError> {
    let v = parse_csv_f64s("speed range", s, 2)?;
    let range = [v[0], v[1]];
    validate_range_order("speed range", range)?;
    Ok(range)
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The toolpath is unsafe / invalid.
    Error,
    /// Suspicious but not necessarily fatal.
    Warning,
}

/// The closed set of verification rule ids. This is the single source of truth for the rule vocabulary,
/// each rule's default [`Severity`], and its one-line summary — the rule catalog (`docs/11`) and the
/// report schema (`spec/dry-reports-v1.schema.json`) are derived from it. A rule is **error** unless it
/// is a process/quality advisory (stringing, first-layer adhesion) rather than a machine-safety or
/// geometric-validity violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleId {
    /// A quantity is non-finite (NaN/inf).
    Finite,
    /// A travel move deposits material.
    TravelExtrudes,
    /// An extruding move has a non-positive bead (width/height).
    Bead,
    /// The toolframe orientation is not a unit vector.
    OrientationNotUnit,
    /// An arc's endpoint radius disagrees with its start radius (or the arc is malformed).
    ArcRadius,
    /// A move leaves the build volume.
    Bounds,
    /// Volumetric flow exceeds the ceiling.
    MaxFlow,
    /// An extruding feedrate is outside the allowed range.
    Speed,
    /// Z decreases where it must be non-decreasing.
    MonotonicZ,
    /// Extruding below the minimum nozzle temperature (or with none set).
    ColdExtrusion,
    /// A retraction distance exceeds the limit.
    RetractionDistance,
    /// A retraction/unretraction speed exceeds the limit.
    RetractionSpeed,
    /// A travel run exceeds the allowed distance without a retraction (stringing risk — advisory).
    TravelWithoutRetraction,
    /// First-layer height is outside the allowed range (adhesion — advisory).
    FirstLayerHeight,
    /// First-layer speed is outside the allowed range (adhesion — advisory).
    FirstLayerSpeed,
    /// An arc's centripetal acceleration exceeds the machine's max acceleration.
    PeakAcceleration,
    /// A junction's velocity change exceeds the machine's square-corner velocity.
    JunctionVelocity,
    /// Verbatim or imported G-code is preserved but not semantically verified.
    UnmodeledGcode,
}

/// One rule's catalog entry.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub id: RuleId,
    pub severity: Severity,
    pub summary: &'static str,
}

impl RuleId {
    /// Every rule, in catalog order.
    pub const ALL: [RuleId; 18] = [
        RuleId::Finite,
        RuleId::TravelExtrudes,
        RuleId::Bead,
        RuleId::OrientationNotUnit,
        RuleId::ArcRadius,
        RuleId::Bounds,
        RuleId::MaxFlow,
        RuleId::Speed,
        RuleId::MonotonicZ,
        RuleId::ColdExtrusion,
        RuleId::RetractionDistance,
        RuleId::RetractionSpeed,
        RuleId::TravelWithoutRetraction,
        RuleId::FirstLayerHeight,
        RuleId::FirstLayerSpeed,
        RuleId::PeakAcceleration,
        RuleId::JunctionVelocity,
        RuleId::UnmodeledGcode,
    ];

    /// The stable kebab-case wire id.
    pub fn as_str(self) -> &'static str {
        match self {
            RuleId::Finite => "finite",
            RuleId::TravelExtrudes => "travel-extrudes",
            RuleId::Bead => "bead",
            RuleId::OrientationNotUnit => "orientation-not-unit",
            RuleId::ArcRadius => "arc-radius",
            RuleId::Bounds => "bounds",
            RuleId::MaxFlow => "max-flow",
            RuleId::Speed => "speed",
            RuleId::MonotonicZ => "monotonic-z",
            RuleId::ColdExtrusion => "cold-extrusion",
            RuleId::RetractionDistance => "retraction-distance",
            RuleId::RetractionSpeed => "retraction-speed",
            RuleId::TravelWithoutRetraction => "travel-without-retraction",
            RuleId::FirstLayerHeight => "first-layer-height",
            RuleId::FirstLayerSpeed => "first-layer-speed",
            RuleId::PeakAcceleration => "peak-acceleration",
            RuleId::JunctionVelocity => "junction-velocity",
            RuleId::UnmodeledGcode => "unmodeled-gcode",
        }
    }

    /// Parse a wire id back into a [`RuleId`].
    pub fn from_wire(s: &str) -> Option<RuleId> {
        RuleId::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// The rule's default severity.
    pub fn default_severity(self) -> Severity {
        match self {
            RuleId::TravelWithoutRetraction
            | RuleId::FirstLayerHeight
            | RuleId::FirstLayerSpeed
            | RuleId::JunctionVelocity
            | RuleId::UnmodeledGcode => Severity::Warning,
            _ => Severity::Error,
        }
    }

    /// A one-line human summary for the catalog/docs.
    pub fn summary(self) -> &'static str {
        match self {
            RuleId::Finite => "a quantity is non-finite (NaN or infinite)",
            RuleId::TravelExtrudes => "a travel (non-printing) move deposits material",
            RuleId::Bead => "an extruding move has a non-positive bead width or height",
            RuleId::OrientationNotUnit => "the toolframe orientation vector is not unit length",
            RuleId::ArcRadius => "an arc's endpoint radius disagrees with its start radius",
            RuleId::Bounds => "a move leaves the build volume",
            RuleId::MaxFlow => "volumetric flow exceeds the configured ceiling",
            RuleId::Speed => "an extruding feedrate is outside the allowed range",
            RuleId::MonotonicZ => "Z decreases where it is required to be non-decreasing",
            RuleId::ColdExtrusion => "extruding below the minimum nozzle temperature",
            RuleId::RetractionDistance => "a retraction distance exceeds the limit",
            RuleId::RetractionSpeed => "a retraction or unretraction speed exceeds the limit",
            RuleId::TravelWithoutRetraction => {
                "a travel run exceeds the allowed distance without a retraction"
            }
            RuleId::FirstLayerHeight => "the first-layer height is outside the allowed range",
            RuleId::FirstLayerSpeed => "the first-layer speed is outside the allowed range",
            RuleId::PeakAcceleration => {
                "an arc's centripetal acceleration exceeds the machine's max acceleration"
            }
            RuleId::JunctionVelocity => {
                "a junction's velocity change exceeds the machine's square-corner velocity"
            }
            RuleId::UnmodeledGcode => {
                "verbatim or imported G-code is preserved but not semantically verified"
            }
        }
    }
}

/// The full rule catalog (id, default severity, summary), in catalog order.
pub fn catalog() -> Vec<Rule> {
    RuleId::ALL
        .into_iter()
        .map(|id| Rule {
            id,
            severity: id.default_severity(),
            summary: id.summary(),
        })
        .collect()
}

/// One located issue found by [`verify`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// A stable kebab-case rule id from the [`RuleId`] catalog.
    pub rule: String,
    pub severity: Severity,
    /// The offending segment index, if the finding is local to one move.
    pub segment: Option<usize>,
    /// A human-readable description.
    pub message: String,
}

/// The result of verifying a toolpath.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

/// Arc radius in mm from the segment's start point and centre; `None` if the segment is not a
/// well-formed arc (missing centre/start, or degenerate).
fn arc_radius_mm(s: &Segment) -> Option<f64> {
    let [cx, cy] = s.centre?;
    let (sx, sy) = (s.start[0]?, s.start[1]?);
    let r = (sx - cx).hypot(sy - cy).value();
    if r > 0.0 {
        Some(r)
    } else {
        None
    }
}

/// True when the previous printing segment's end ≈ this segment's start (within 0.1 mm in X/Y/Z).
fn junction_contiguous(
    prev_end: &Option<[Option<Length>; 3]>,
    start: &[Option<Length>; 3],
) -> bool {
    let Some(pe) = prev_end else {
        return false;
    };
    (0..3).all(|k| match (pe[k], start[k]) {
        (Some(a), Some(b)) => (a.value() - b.value()).abs() <= 0.1,
        (None, None) => true,
        _ => false,
    })
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

fn push_finding(report: &mut Report, rule: RuleId, segment: Option<usize>, message: String) {
    report.findings.push(Finding {
        rule: rule.as_str().to_string(),
        severity: rule.default_severity(),
        segment,
        message,
    });
}

/// Verify a stream of segments against the contracts, returning all findings (structural + contract-driven).
pub fn verify_stream<I>(segments: I, c: &Contracts) -> Result<Report, crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<Segment, crate::codec::CodecError>>,
{
    let mut r = Report::default();
    let axis = ['X', 'Y', 'Z'];
    let mut first_layer_z: Option<f64> = None;

    let mut travel_run_length = 0.0;
    let mut retracted = true;
    let mut flagged_travel = false;
    // For junction-velocity: track the end position and speed of the previous printing segment
    // so we can compute Δv at contiguous junctions (reset on travel moves).
    let mut prev_print_end: Option<[Option<Length>; 3]> = None;
    let mut prev_speed_mm_s: Option<f64> = None;

    for (i, segment) in segments.into_iter().enumerate() {
        let s = segment?;
        // --- structural invariants (always on) ---
        if s.kind == SegmentKind::ManualGcode {
            push_finding(
                &mut r,
                RuleId::UnmodeledGcode,
                Some(i),
                "verbatim manual G-code is emitted without semantic verification".into(),
            );
        }
        let nums = segment_numbers(&s);
        if nums.iter().any(|v| !v.is_finite()) {
            push_finding(
                &mut r,
                RuleId::Finite,
                Some(i),
                "segment carries a non-finite value".into(),
            );
        }
        if s.travel && s.volume.value() > 0.0 {
            push_finding(
                &mut r,
                RuleId::TravelExtrudes,
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
                push_finding(
                    &mut r,
                    RuleId::Bead,
                    Some(i),
                    format!("extruding move has a non-positive bead (width {w}, height {h})"),
                );
            }
        }
        if let Some([x, y, z]) = s.orientation {
            // the toolframe orientation must be a unit direction vector.
            let mag = libm::sqrt(x * x + y * y + z * z);
            if (mag - 1.0).abs() > 1e-6 {
                push_finding(
                    &mut r,
                    RuleId::OrientationNotUnit,
                    Some(i),
                    format!(
                        "toolframe orientation [{x}, {y}, {z}] has magnitude {mag} (must be 1)"
                    ),
                );
            }
        }
        if let Some(message) = arc_radius_error(&s) {
            push_finding(&mut r, RuleId::ArcRadius, Some(i), message);
        }

        // --- contract-driven checks ---
        if let Some(b) = c.bounds {
            'points: for point in bounds_points(&s) {
                for (k, coord) in point.iter().enumerate() {
                    if let Some(v) = coord {
                        let v = v.value();
                        if v < b[k][0] || v > b[k][1] {
                            push_finding(
                                &mut r,
                                RuleId::Bounds,
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
                push_finding(
                    &mut r,
                    RuleId::MaxFlow,
                    Some(i),
                    format!("flow {f:.3} mm³/s exceeds the ceiling {max:.3}"),
                );
            }
        }
        if let Some([lo, hi]) = c.speed_range {
            if !s.travel && s.length.value() > 0.0 && s.volume.value() > 0.0 {
                let v = s.speed.value();
                if v < lo || v > hi {
                    push_finding(
                        &mut r,
                        RuleId::Speed,
                        Some(i),
                        format!("feedrate {v} is outside [{lo}, {hi}] mm/min"),
                    );
                }
            }
        }
        if c.monotonic_z {
            if let (Some(z0), Some(z1)) = (s.start[2], s.end[2]) {
                if z1 < z0 {
                    push_finding(
                        &mut r,
                        RuleId::MonotonicZ,
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
                push_finding(
                    &mut r,
                    RuleId::ColdExtrusion,
                    Some(i),
                    format!("extruding at nozzle temperature {got} (< {min} °C)"),
                );
            }
        }

        // --- retraction checks ---
        let is_retract = s.filament.value() < 0.0;
        let is_unretract =
            s.filament.value() > 0.0 && s.length.value() == 0.0 && s.volume.value() == 0.0;
        if is_retract || is_unretract {
            if let Some(max_speed) = c.max_retraction_speed {
                if s.speed.value() > max_speed {
                    push_finding(
                        &mut r,
                        RuleId::RetractionSpeed,
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
        let extrudes_material = !s.travel && s.volume.value() > 0.0;
        if is_retract {
            let dist = -s.filament.value();
            if let Some(max_dist) = c.max_retraction_distance {
                if dist > max_dist {
                    push_finding(
                        &mut r,
                        RuleId::RetractionDistance,
                        Some(i),
                        format!(
                            "retraction distance {dist:.3} mm exceeds the limit of {max_dist:.3}"
                        ),
                    );
                }
            }
            retracted = true;
        } else if is_unretract || extrudes_material {
            retracted = false;
            travel_run_length = 0.0;
            flagged_travel = false;
        } else if s.travel {
            travel_run_length += s.length.value();
            if let Some(max_travel) = c.max_travel_without_retract {
                if travel_run_length > max_travel && !retracted && !flagged_travel {
                    push_finding(
                        &mut r,
                        RuleId::TravelWithoutRetraction,
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
            let z = s.end[2]
                .or(s.start[2])
                .map(Length::value)
                .filter(|z| z.is_finite());
            let is_first_layer = if let Some(z) = z {
                match first_layer_z {
                    None => {
                        first_layer_z = Some(z);
                        true
                    }
                    Some(current) if z < current - 1e-4 => {
                        // A later, lower layer invalidates provisional findings from the earlier
                        // minimum. Removing only those two rule ids keeps this pass streaming.
                        r.findings.retain(|finding| {
                            finding.rule != RuleId::FirstLayerHeight.as_str()
                                && finding.rule != RuleId::FirstLayerSpeed.as_str()
                        });
                        first_layer_z = Some(z);
                        true
                    }
                    Some(current) => (z - current).abs() < 1e-4,
                }
            } else {
                false
            };

            if is_first_layer {
                if let Some([min_h, max_h]) = c.first_layer_height_range {
                    let h_val = s
                        .height
                        .map(|height| height.value())
                        .or(first_layer_z)
                        .unwrap_or(0.0);
                    if h_val < min_h || h_val > max_h {
                        push_finding(
                            &mut r,
                            RuleId::FirstLayerHeight,
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
                        push_finding(
                            &mut r,
                            RuleId::FirstLayerSpeed,
                            Some(i),
                            format!(
                                "first layer speed {speed_val:.3} mm/min is outside the range [{min_s:.3}, {max_s:.3}]"
                            ),
                        );
                    }
                }
            }
        }

        // --- kinematic checks ---
        if let Some(kin) = &c.kinematics {
            let is_print = !s.travel && s.length.value() > 0.0 && s.volume.value() > 0.0;

            // PeakAcceleration: centripetal acceleration of an arc must not exceed the machine max.
            // a = v² / r  where v is in mm/s and r is the arc radius in mm.
            if let Some(max_a) = kin.max_acceleration_mm_s2 {
                if s.kind == SegmentKind::Arc {
                    if let Some(radius) = arc_radius_mm(&s) {
                        let v = s.speed.value() / 60.0;
                        let a = v * v / radius;
                        if a > max_a {
                            push_finding(
                                &mut r,
                                RuleId::PeakAcceleration,
                                Some(i),
                                format!(
                                    "arc centripetal accel {a:.0} mm/s² exceeds max {max_a:.0}"
                                ),
                            );
                        }
                    }
                }
            }

            // JunctionVelocity: fire when two contiguous printing segments have a Δv that exceeds
            // the machine's square-corner velocity limit. Contiguity is required (within 0.1 mm)
            // so non-adjacent segments (e.g. after a travel) never produce a false positive.
            if let (Some(max_jv), Some(pv), true) =
                (kin.max_junction_velocity_mm_s, prev_speed_mm_s, is_print)
            {
                if junction_contiguous(&prev_print_end, &s.start) {
                    let dv = (s.speed.value() / 60.0 - pv).abs();
                    if dv > max_jv {
                        push_finding(
                            &mut r,
                            RuleId::JunctionVelocity,
                            Some(i),
                            format!(
                                "junction Δv {dv:.1} mm/s exceeds square-corner velocity {max_jv:.1}"
                            ),
                        );
                    }
                }
            }

            if is_print {
                prev_print_end = Some(s.end);
                prev_speed_mm_s = Some(s.speed.value() / 60.0);
            } else if s.travel {
                // Reset junction tracking across travel moves.
                prev_print_end = None;
                prev_speed_mm_s = None;
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
    use crate::units::{Feedrate, Volume};

    #[test]
    fn contract_csv_parsers_reject_inverted_ranges() {
        for (input, axis) in [
            ("1,0,0,1,0,1", "x"),
            ("0,1,1,0,0,1", "y"),
            ("0,1,0,1,1,0", "z"),
        ] {
            let error = parse_bounds_csv(input).unwrap_err().to_string();
            assert_eq!(
                error,
                format!("bounds {axis} lower bound must be <= upper bound")
            );
        }

        assert_eq!(
            parse_speed_range_csv("9000,300").unwrap_err().to_string(),
            "speed range lower bound must be <= upper bound"
        );
    }

    #[test]
    fn contract_csv_parsers_allow_equal_endpoints() {
        assert_eq!(
            parse_bounds_csv("1,1,2,2,3,3").unwrap(),
            [[1.0, 1.0], [2.0, 2.0], [3.0, 3.0]]
        );
        assert_eq!(parse_speed_range_csv("600,600").unwrap(), [600.0, 600.0]);
    }

    /// A single arc segment with the given radius (mm) and speed (mm/min), valid geometry.
    fn arc_toolpath(radius_mm: f64, speed_mm_min: f64) -> Toolpath {
        // Centre at origin; start at (radius, 0), end at (0, radius) — a CCW quarter-circle.
        // Both start and end radii equal radius_mm, so no arc-radius error fires.
        Toolpath {
            version: 0,
            meta: None,
            segments: vec![Segment {
                start: [
                    Some(Length::mm(radius_mm)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(radius_mm)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(speed_mm_min),
                length: Length::mm(radius_mm * std::f64::consts::FRAC_PI_2),
                volume: Volume(0.8),
                filament: Length::mm(0.33),
                width: Some(Length::mm(0.4)),
                height: Some(Length::mm(0.2)),
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(0.0), Length::mm(0.0)]),
                clockwise: false,
                travel: false,
                temperature: Some(210.0),
                fan: None,
                flow: None,
                tool: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            }],
        }
    }

    /// Two contiguous printing line segments (end of seg0 == start of seg1), at different speeds.
    fn two_segment_junction(v0_mm_min: f64, v1_mm_min: f64) -> Toolpath {
        let seg0 = Segment {
            start: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            speed: Feedrate(v0_mm_min),
            length: Length::mm(10.0),
            volume: Volume(0.8),
            filament: Length::mm(0.33),
            width: Some(Length::mm(0.4)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            travel: false,
            temperature: Some(210.0),
            fan: None,
            flow: None,
            tool: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        };
        let seg1 = Segment {
            start: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(10.0)),
                Some(Length::mm(0.2)),
            ],
            speed: Feedrate(v1_mm_min),
            ..seg0.clone()
        };
        Toolpath {
            version: 0,
            meta: None,
            segments: vec![seg0, seg1],
        }
    }

    #[test]
    fn arc_over_centripetal_limit_is_a_peak_acceleration_error() {
        // Arc of radius 5 mm at 6000 mm/min → v = 100 mm/s → a = v²/r = 2000 mm/s² > 1000.
        let tp = arc_toolpath(5.0, 6000.0);
        let c = Contracts {
            kinematics: Some(KinematicContracts {
                max_acceleration_mm_s2: Some(1000.0),
                max_junction_velocity_mm_s: None,
            }),
            ..Contracts::default()
        };
        let report = verify(&tp, &c);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "peak-acceleration" && f.severity == Severity::Error),
            "expected peak-acceleration Error, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn junction_over_scv_is_a_junction_velocity_warning() {
        // Two contiguous printing segments: v0 = 600 mm/min (10 mm/s), v1 = 6000 mm/min (100 mm/s).
        // Δv = 90 mm/s, limit = 5 mm/s → junction-velocity Warning.
        let tp = two_segment_junction(600.0, 6000.0);
        let c = Contracts {
            kinematics: Some(KinematicContracts {
                max_acceleration_mm_s2: None,
                max_junction_velocity_mm_s: Some(5.0),
            }),
            ..Contracts::default()
        };
        let report = verify(&tp, &c);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "junction-velocity" && f.severity == Severity::Warning),
            "expected junction-velocity Warning, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn no_kinematics_means_no_kinematic_findings() {
        let tp = arc_toolpath(5.0, 6000.0);
        let report = verify(&tp, &Contracts::default());
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.rule == "peak-acceleration" || f.rule == "junction-velocity"),
            "expected no kinematic findings, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn catalog_includes_the_two_kinematic_rules() {
        let cat = catalog();
        let pa = cat
            .iter()
            .find(|r| r.id == RuleId::PeakAcceleration)
            .expect("peak-acceleration in catalog");
        assert_eq!(pa.severity, Severity::Error);
        assert_eq!(RuleId::PeakAcceleration.as_str(), "peak-acceleration");
        let jv = cat
            .iter()
            .find(|r| r.id == RuleId::JunctionVelocity)
            .expect("junction-velocity in catalog");
        assert_eq!(jv.severity, Severity::Warning);
        assert_eq!(RuleId::JunctionVelocity.as_str(), "junction-velocity");
    }

    #[test]
    fn contracts_default_has_no_kinematics() {
        assert!(Contracts::default().kinematics.is_none());
    }

    #[test]
    fn empty_toolpath_is_ok() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert!(verify(&tp, &Contracts::default()).ok());
    }

    #[test]
    fn manual_gcode_is_always_reported_as_unmodeled() {
        let mut tp = two_segment_junction(600.0, 600.0);
        tp.segments.truncate(1);
        tp.segments[0].kind = SegmentKind::ManualGcode;
        tp.segments[0].manual_gcode = Some("M84".to_string());
        let report = verify(&tp, &Contracts::default());
        assert!(report.findings.iter().any(|finding| {
            finding.rule == "unmodeled-gcode" && finding.severity == Severity::Warning
        }));
    }

    #[test]
    fn later_lower_layer_replaces_provisional_first_layer_findings() {
        let mut tp = two_segment_junction(600.0, 600.0);
        tp.segments[0].start[2] = Some(Length::mm(0.3));
        tp.segments[0].end[2] = Some(Length::mm(0.3));
        tp.segments[0].height = Some(Length::mm(0.5));
        tp.segments[1].start[2] = Some(Length::mm(0.2));
        tp.segments[1].end[2] = Some(Length::mm(0.2));
        tp.segments[1].height = Some(Length::mm(0.2));
        let contracts = Contracts {
            first_layer_height_range: Some([0.1, 0.3]),
            ..Contracts::default()
        };
        let report = verify(&tp, &contracts);
        assert!(
            report
                .findings
                .iter()
                .all(|finding| finding.rule != "first-layer-height"),
            "only the true minimum-Z layer should be checked: {:?}",
            report.findings
        );
    }

    #[test]
    fn rule_catalog_is_consistent() {
        let cat = catalog();
        assert_eq!(cat.len(), RuleId::ALL.len());
        for rule in &cat {
            // wire id round-trips and is unique-mapping
            assert_eq!(RuleId::from_wire(rule.id.as_str()), Some(rule.id));
            assert!(!rule.summary.is_empty());
            assert_eq!(rule.severity, rule.id.default_severity());
        }
        // process/quality advisories are warnings; everything else is an error.
        let warnings: Vec<&str> = cat
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(
            warnings,
            vec![
                "travel-without-retraction",
                "first-layer-height",
                "first-layer-speed",
                "junction-velocity",
                "unmodeled-gcode",
            ]
        );
    }
}
