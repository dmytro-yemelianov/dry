//! APT-CL toolpath lifter.
//!
//! Lifts ISO 4343 APT-CL statement sequences into L2 [`crate::ir::Toolpath`] structures
//! with exact rational scaling, MULTAX state machine tracking, MOVARC planar arc reconstruction,
//! DRILL cycle expansion, and source map tracking per `docs/28-apt-irbcam-dialects.md`.

use std::fmt;
use std::io::Read;
use std::ops::Range;

use crate::apt::{
    parse_apt_statements, AptErrorCode, AptMotion, AptParseError, AptParser, AptProcess, AptRecord,
    AptRefusalReason, AptState, FedratUnit, ParsedAptStatement, SpindleDirection,
};
use crate::ir::{Meta, Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};

/// Units configuration for APT programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AptUnits {
    Mm,
    Inches,
}

/// Unknown major word handling policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UnknownMajorWordPolicy {
    #[default]
    Refuse,
    Preserve,
}

/// Parameters controlling APT import.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AptImportParams {
    pub version: u32,
    pub assume_units: Option<AptUnits>,
    pub unknown_major_words: UnknownMajorWordPolicy,
    pub arc_tolerance_rel: f64,
}

impl Default for AptImportParams {
    fn default() -> Self {
        Self {
            version: 0,
            assume_units: None,
            unknown_major_words: UnknownMajorWordPolicy::Refuse,
            arc_tolerance_rel: 1e-6,
        }
    }
}

/// Resource limits for APT import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AptImportLimits {
    pub max_source_bytes: usize,
    pub max_statements: usize,
    pub max_line_bytes: usize,
    pub max_continuation_lines: usize,
    pub max_values_per_statement: usize,
    pub max_cycle_holes: usize,
}

impl Default for AptImportLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 256 * 1024 * 1024,
            max_statements: 8_000_000,
            max_line_bytes: 4096,
            max_continuation_lines: 64,
            max_values_per_statement: 64,
            max_cycle_holes: 1_000_000,
        }
    }
}

/// Located import error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptImportError {
    pub source_line: usize,
    pub code: AptErrorCode,
    pub message: String,
}

impl fmt::Display for AptImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}: [{}] {}",
            self.source_line,
            self.code.as_str(),
            self.message
        )
    }
}

impl std::error::Error for AptImportError {}

impl From<AptParseError> for AptImportError {
    fn from(err: AptParseError) -> Self {
        AptImportError {
            source_line: err.source_line,
            code: err.code,
            message: err.message,
        }
    }
}

/// Preserved unmodeled statement (class I).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmodeledApt {
    pub source_line: usize,
    pub major: String,
    pub raw: String,
}

/// Advisory warning or informational finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptAdvisory {
    pub source_line: usize,
    pub code: String,
    pub message: String,
}

/// Complete imported APT program including toolpath, source lines, and source maps.
#[derive(Debug, Clone)]
pub struct ImportedApt {
    pub toolpath: Toolpath,
    pub source_lines: Vec<String>,
    pub segment_source_lines: Vec<usize>,
    pub statement_segments: Vec<Option<Range<usize>>>,
    pub unmodeled_statements: Vec<UnmodeledApt>,
    pub advisories: Vec<AptAdvisory>,
}

#[derive(Debug, Clone)]
struct ActiveCycle {
    depth: f64,
    feed: f64,
    rapto: f64,
    dwell: Option<f64>,
}

#[derive(Debug, Clone)]
struct PendingMovarc {
    source_line: usize,
    cx: f64,
    cy: f64,
    _cz: f64,
    r: f64,
    theta: f64,
    clockwise: bool,
}

fn signed_sweep(start: f64, end: f64, clockwise: bool) -> f64 {
    let tau = std::f64::consts::TAU;
    let forward = (end - start).rem_euclid(tau);
    if clockwise {
        if forward == 0.0 {
            -tau
        } else {
            forward - tau
        }
    } else if forward == 0.0 {
        tau
    } else {
        forward
    }
}

fn checked_mm(val: f64, source_line: usize, field: &str) -> Result<Length, AptImportError> {
    Length::try_mm(val).ok_or_else(|| AptImportError {
        source_line,
        code: AptErrorCode::NonFinite,
        message: format!("computed {field} {val} is non-finite or out of range"),
    })
}

#[allow(clippy::too_many_arguments)]
fn make_segment(
    start: [f64; 3],
    end: [f64; 3],
    kind: SegmentKind,
    travel: bool,
    speed: f64,
    length: f64,
    centre: Option<[f64; 2]>,
    clockwise: bool,
    dwell_s: Option<f64>,
    tool: Option<u32>,
    power: Option<f64>,
    orientation: Option<[f64; 3]>,
    line: usize,
) -> Result<Segment, AptImportError> {
    let s = [
        Some(checked_mm(start[0], line, "start X")?),
        Some(checked_mm(start[1], line, "start Y")?),
        Some(checked_mm(start[2], line, "start Z")?),
    ];
    let e = [
        Some(checked_mm(end[0], line, "end X")?),
        Some(checked_mm(end[1], line, "end Y")?),
        Some(checked_mm(end[2], line, "end Z")?),
    ];
    let c = match centre {
        Some([cx, cy]) => Some([
            checked_mm(cx, line, "centre X")?,
            checked_mm(cy, line, "centre Y")?,
        ]),
        None => None,
    };
    Ok(Segment {
        start: s,
        end: e,
        travel,
        speed: Feedrate(speed),
        length: checked_mm(length, line, "length")?,
        volume: Volume::ZERO,
        filament: Length::ZERO,
        width: None,
        height: None,
        kind,
        centre: c,
        clockwise,
        temperature: None,
        fan: None,
        flow: None,
        tool,
        power,
        dwell_s,
        manual_gcode: None,
        orientation,
        control_points: None,
    })
}

/// Imports an APT-CL source string into a Toolpath.
pub fn import_apt(source: &str, params: &AptImportParams) -> Result<Toolpath, AptImportError> {
    Ok(import_apt_with_map(source, params)?.toolpath)
}

/// Imports an APT-CL source string with explicit limits into a Toolpath.
pub fn import_apt_with_limits(
    source: &str,
    params: &AptImportParams,
    limits: &AptImportLimits,
) -> Result<Toolpath, AptImportError> {
    import_apt_reader_with_limits(source.as_bytes(), params, limits).map(|res| res.toolpath)
}

/// Imports an APT-CL source string into an ImportedApt structure with source maps.
pub fn import_apt_with_map(
    source: &str,
    params: &AptImportParams,
) -> Result<ImportedApt, AptImportError> {
    let limits = AptImportLimits::default();
    let source_lines: Vec<String> = source.lines().map(str::to_string).collect();
    let statements = parse_apt_statements(source, &limits)?;
    let mut imported = import_parsed_apt_with_map(statements, params)?;
    imported.source_lines = source_lines;
    Ok(imported)
}

/// Imports an APT-CL reader into a Toolpath.
pub fn import_apt_reader<R: Read>(
    reader: R,
    params: &AptImportParams,
) -> Result<Toolpath, AptImportError> {
    Ok(import_apt_reader_with_limits(reader, params, &AptImportLimits::default())?.toolpath)
}

/// Imports an APT-CL reader into an ImportedApt structure.
pub fn import_apt_reader_with_map<R: Read>(
    reader: R,
    params: &AptImportParams,
) -> Result<ImportedApt, AptImportError> {
    import_apt_reader_with_limits(reader, params, &AptImportLimits::default())
}

/// Imports an APT-CL reader with explicit limits into an ImportedApt structure.
pub fn import_apt_reader_with_limits<R: Read>(
    reader: R,
    params: &AptImportParams,
    limits: &AptImportLimits,
) -> Result<ImportedApt, AptImportError> {
    let mut parser = AptParser::new(reader, *limits);
    let mut statements = Vec::new();
    while let Some(stmt) = parser.next_statement()? {
        statements.push(stmt);
    }
    import_parsed_apt_with_map(statements, params)
}

/// Lifts parsed APT statements into an ImportedApt structure.
pub fn import_parsed_apt_with_map<I: IntoIterator<Item = ParsedAptStatement>>(
    statements: I,
    params: &AptImportParams,
) -> Result<ImportedApt, AptImportError> {
    let mut units = params.assume_units;
    let mut motion_occurred = false;
    let mut current_feed: Option<f64> = None;
    let mut is_rapid = false;
    let mut multax = false;
    let mut tool_axis = [0.0, 0.0, 1.0];
    let mut active_tool: Option<u32> = None;
    let mut active_power: Option<f64> = None;
    let mut last_rpm: Option<f64> = None;
    let mut active_cycle: Option<ActiveCycle> = None;
    let mut cycle_hole_count = 0;
    let mut current_pos = [0.0, 0.0, 0.0];

    let mut pending_movarc: Option<PendingMovarc> = None;
    let mut segments: Vec<Segment> = Vec::new();
    let mut segment_source_lines: Vec<usize> = Vec::new();
    let mut statement_segments: Vec<Option<Range<usize>>> = Vec::new();
    let mut unmodeled_statements: Vec<UnmodeledApt> = Vec::new();
    let mut advisories: Vec<AptAdvisory> = Vec::new();

    let limits = AptImportLimits::default();

    for stmt in statements {
        let line = stmt.source_line;
        let start_seg_idx = segments.len();

        if pending_movarc.is_some() {
            if let AptRecord::Motion(ref motion) = stmt.record {
                if !matches!(motion.as_ref(), AptMotion::Goto(_)) {
                    return Err(AptImportError {
                        source_line: line,
                        code: AptErrorCode::ArcEndpointMissing,
                        message: "MOVARC must be immediately followed by GOTO endpoint".to_string(),
                    });
                }
            } else {
                return Err(AptImportError {
                    source_line: line,
                    code: AptErrorCode::ArcEndpointMissing,
                    message: "MOVARC must be immediately followed by GOTO endpoint".to_string(),
                });
            }
        }

        match stmt.record {
            AptRecord::Empty | AptRecord::Comment => {
                statement_segments.push(None);
                continue;
            }
            AptRecord::Inert { major } => {
                unmodeled_statements.push(UnmodeledApt {
                    source_line: line,
                    major,
                    raw: stmt.raw,
                });
                statement_segments.push(None);
                continue;
            }
            AptRecord::Refused { major, reason } => {
                if let AptRefusalReason::UnknownMajor(_) = &reason {
                    if params.unknown_major_words == UnknownMajorWordPolicy::Preserve {
                        unmodeled_statements.push(UnmodeledApt {
                            source_line: line,
                            major: major.clone(),
                            raw: stmt.raw.clone(),
                        });
                        continue;
                    }
                }
                let code = match &reason {
                    AptRefusalReason::Cutcom(_) => AptErrorCode::CutcomOn,
                    AptRefusalReason::Insert(_) => AptErrorCode::InsertOpaque,
                    AptRefusalReason::Hazardous(_) => AptErrorCode::MajorWordHazardous,
                    AptRefusalReason::UnknownMajor(_) => AptErrorCode::MajorWordUnknown,
                    AptRefusalReason::CycleUnsupported(_) => AptErrorCode::CycleUnsupported,
                    AptRefusalReason::FeedUnitUnsupported(_) => AptErrorCode::FeedUnitUnsupported,
                    AptRefusalReason::GotoArity(_) => AptErrorCode::GotoArity,
                    AptRefusalReason::NonFinite(_) => AptErrorCode::NonFinite,
                };
                let message = match &reason {
                    AptRefusalReason::Cutcom(msg)
                    | AptRefusalReason::Insert(msg)
                    | AptRefusalReason::Hazardous(msg)
                    | AptRefusalReason::UnknownMajor(msg)
                    | AptRefusalReason::CycleUnsupported(msg)
                    | AptRefusalReason::FeedUnitUnsupported(msg)
                    | AptRefusalReason::NonFinite(msg) => msg.clone(),
                    AptRefusalReason::GotoArity(count) => {
                        format!("GOTO arity {count} is unsupported")
                    }
                };
                return Err(AptImportError {
                    source_line: line,
                    code,
                    message,
                });
            }
            AptRecord::State(state) => match state {
                AptState::Units(new_units) => {
                    if motion_occurred {
                        return Err(AptImportError {
                            source_line: line,
                            code: AptErrorCode::UnitLate,
                            message: "UNITS statement cannot appear after motion has started"
                                .to_string(),
                        });
                    }
                    if let Some(existing) = units {
                        if existing != new_units {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::UnitLate,
                                message: "UNITS cannot change mid-program".to_string(),
                            });
                        }
                    } else {
                        units = Some(new_units);
                    }
                }
                AptState::Multax(enabled) => {
                    multax = enabled;
                    if !enabled {
                        tool_axis = [0.0, 0.0, 1.0];
                    }
                }
                AptState::Fedrat { unit, value } => {
                    if active_cycle.is_some() {
                        return Err(AptImportError {
                            source_line: line,
                            code: AptErrorCode::CycleStatementInside,
                            message: "FEDRAT cannot appear inside an active CYCLE".to_string(),
                        });
                    }
                    let unit_scale = match unit {
                        Some(FedratUnit::Mmpm) => 1.0,
                        Some(FedratUnit::Ipm) => 25.4,
                        None => match units {
                            Some(AptUnits::Inches) => 25.4,
                            _ => 1.0,
                        },
                    };
                    current_feed = Some(value * unit_scale);
                }
                AptState::Rapid => {
                    if active_cycle.is_some() {
                        return Err(AptImportError {
                            source_line: line,
                            code: AptErrorCode::CycleStatementInside,
                            message: "RAPID cannot appear inside an active CYCLE".to_string(),
                        });
                    }
                    is_rapid = true;
                }
                AptState::CycleOff => {
                    if active_cycle.is_none() {
                        return Err(AptImportError {
                            source_line: line,
                            code: AptErrorCode::CycleOffInactive,
                            message: "CYCLE/OFF commanded while no cycle is active".to_string(),
                        });
                    }
                    active_cycle = None;
                }
                AptState::CycleDrill {
                    depth,
                    feed_unit,
                    feed,
                    rapto,
                    dwell,
                } => {
                    let prog_units = units.ok_or_else(|| AptImportError {
                        source_line: line,
                        code: AptErrorCode::UnitMissing,
                        message: "CYCLE/DRILL requires units declaration".to_string(),
                    })?;
                    let len_scale = match prog_units {
                        AptUnits::Inches => 25.4,
                        AptUnits::Mm => 1.0,
                    };
                    let feed_scale = match feed_unit {
                        Some(FedratUnit::Ipm) => 25.4,
                        Some(FedratUnit::Mmpm) => 1.0,
                        None => len_scale,
                    };

                    active_cycle = Some(ActiveCycle {
                        depth: depth * len_scale,
                        feed: feed * feed_scale,
                        rapto: rapto * len_scale,
                        dwell,
                    });
                }
            },
            AptRecord::Process(proc) => match proc {
                AptProcess::Loadtl { tool, extra } => {
                    active_tool = Some(tool);
                    if !extra.is_empty() {
                        advisories.push(AptAdvisory {
                            source_line: line,
                            code: "apt-loadtl-minor-words-dropped".to_string(),
                            message: format!("LOADTL minor words [{}] dropped", extra.join(", ")),
                        });
                    }
                }
                AptProcess::SpindlRpm { rpm, direction } => {
                    if direction == Some(SpindleDirection::Cclw) {
                        return Err(AptImportError {
                            source_line: line,
                            code: AptErrorCode::SpindleDirectionUnsupported,
                            message: format!(
                                "SPINDL commands CCLW rotation at {rpm} RPM, which is unsupported"
                            ),
                        });
                    }
                    if direction == Some(SpindleDirection::Clw) {
                        advisories.push(AptAdvisory {
                            source_line: line,
                            code: "apt-spindle-direction-dropped".to_string(),
                            message: format!("spindle direction CLW dropped: power {rpm} carried"),
                        });
                    }
                    active_power = Some(rpm);
                    last_rpm = Some(rpm);
                }
                AptProcess::SpindlOff => {
                    active_power = Some(0.0);
                }
                AptProcess::SpindlOn => {
                    if let Some(rpm) = last_rpm {
                        active_power = Some(rpm);
                    } else {
                        return Err(AptImportError {
                            source_line: line,
                            code: AptErrorCode::SpindleOnWithoutRpm,
                            message: "SPINDL/ON commanded with no prior RPM setting".to_string(),
                        });
                    }
                }
                AptProcess::Delay(t) | AptProcess::Dwell(t) => {
                    if t > 0.0 {
                        let seg = make_segment(
                            current_pos,
                            current_pos,
                            SegmentKind::Dwell,
                            true,
                            0.0,
                            0.0,
                            None,
                            false,
                            Some(t),
                            active_tool,
                            active_power,
                            None,
                            line,
                        )?;
                        segments.push(seg);
                        segment_source_lines.push(line);
                    }
                }
            },
            AptRecord::Motion(motion) => {
                let prog_units = units.ok_or_else(|| AptImportError {
                    source_line: line,
                    code: AptErrorCode::UnitMissing,
                    message: "motion statement requires UNITS declaration".to_string(),
                })?;
                let scale = match prog_units {
                    AptUnits::Inches => 25.4,
                    AptUnits::Mm => 1.0,
                };
                motion_occurred = true;

                match *motion {
                    AptMotion::Circle { center, radius, .. } => {
                        advisories.push(AptAdvisory {
                            source_line: line,
                            code: "apt-circle-annotation-ignored".to_string(),
                            message: format!(
                                "CIRCLE about ({}, {}, {}) r={} is advisory: subsequent GOTO points lift as lines",
                                center[0] * scale,
                                center[1] * scale,
                                center[2] * scale,
                                radius * scale
                            ),
                        });
                    }
                    AptMotion::Movarc {
                        center,
                        normal,
                        radius,
                        sweep_deg,
                    } => {
                        if active_cycle.is_some() {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::CycleStatementInside,
                                message: "MOVARC cannot appear inside an active CYCLE".to_string(),
                            });
                        }
                        if is_rapid {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::RapidArc,
                                message: "RAPID cannot precede MOVARC".to_string(),
                            });
                        }

                        let mag = libm::hypot(libm::hypot(normal[0], normal[1]), normal[2]);
                        if !(mag.is_finite() && mag > 0.0) {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::ArcNormalDegenerate,
                                message: "MOVARC normal has zero or non-finite magnitude"
                                    .to_string(),
                            });
                        }
                        let (nx, ny, nz) = (normal[0] / mag, normal[1] / mag, normal[2] / mag);
                        if nx.abs() > 1e-6 || ny.abs() > 1e-6 || (nz.abs() - 1.0).abs() > 1e-6 {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::ArcNormalUnsupported,
                                message: format!(
                                    "MOVARC plane normal ({nx}, {ny}, {nz}) is not +/-Z; non-planar arcs are unsupported"
                                ),
                            });
                        }
                        if radius <= 0.0 {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::ArcRadiusNonpositive,
                                message: format!("MOVARC radius {radius} must be > 0"),
                            });
                        }
                        if sweep_deg <= 0.0 || sweep_deg >= 360.0 {
                            return Err(AptImportError {
                                source_line: line,
                                code: AptErrorCode::ArcFullTurn,
                                message: format!(
                                    "MOVARC sweep angle {sweep_deg} deg must satisfy 0 < theta < 360"
                                ),
                            });
                        }

                        pending_movarc = Some(PendingMovarc {
                            source_line: line,
                            cx: center[0],
                            cy: center[1],
                            _cz: center[2],
                            r: radius,
                            theta: sweep_deg,
                            clockwise: nz < 0.0,
                        });
                    }
                    AptMotion::Goto(coords) => {
                        let is_six = coords.len() == 6;
                        if is_six {
                            if !multax {
                                return Err(AptImportError {
                                    source_line: line,
                                    code: AptErrorCode::MultaxInconsistent,
                                    message: "6-value GOTO requires MULTAX/ON".to_string(),
                                });
                            }
                            let (i, j, k) = (coords[3], coords[4], coords[5]);
                            let mag = libm::hypot(libm::hypot(i, j), k);
                            if !(mag.is_finite() && mag > 0.0) {
                                return Err(AptImportError {
                                    source_line: line,
                                    code: AptErrorCode::TlaxisDegenerate,
                                    message: format!("tool axis vector ({i}, {j}, {k}) has zero or non-finite magnitude"),
                                });
                            }
                            tool_axis = [i / mag, j / mag, k / mag];
                        }

                        let target_pt = [coords[0] * scale, coords[1] * scale, coords[2] * scale];

                        if let Some(movarc) = pending_movarc.take() {
                            let (cx, cy) = (movarc.cx * scale, movarc.cy * scale);
                            let r = movarc.r * scale;
                            let [sx, sy, sz] = current_pos;
                            let [ex, ey, ez] = target_pt;

                            let tau_r = params.arc_tolerance_rel * (r * r).max(1.0);
                            let d_start_sq = (sx - cx).powi(2) + (sy - cy).powi(2);
                            let d_end_sq = (ex - cx).powi(2) + (ey - cy).powi(2);

                            if (d_start_sq - r * r).abs() > tau_r
                                || (d_end_sq - r * r).abs() > tau_r
                            {
                                return Err(AptImportError {
                                    source_line: movarc.source_line,
                                    code: AptErrorCode::ArcOffCircle,
                                    message: format!(
                                        "MOVARC start or end point is not on circle radius {r} (start delta={}, end delta={})",
                                        (d_start_sq - r * r).abs(),
                                        (d_end_sq - r * r).abs()
                                    ),
                                });
                            }

                            let phi_s = libm::atan2(sy - cy, sx - cx);
                            let phi_e = libm::atan2(ey - cy, ex - cx);
                            let sweep = signed_sweep(phi_s, phi_e, movarc.clockwise);
                            let sweep_calc_deg = sweep.abs().to_degrees();
                            let tau_theta = 1e-3 * movarc.theta.max(1.0);

                            if (sweep_calc_deg - movarc.theta).abs() > tau_theta {
                                return Err(AptImportError {
                                    source_line: movarc.source_line,
                                    code: AptErrorCode::ArcSweepMismatch,
                                    message: format!(
                                        "MOVARC sweep inconsistency: declared {} deg, calculated {} deg",
                                        movarc.theta, sweep_calc_deg
                                    ),
                                });
                            }

                            let arc_len = libm::hypot(r * sweep.abs(), ez - sz);
                            let orientation = if tool_axis[0].abs() < 1e-9
                                && tool_axis[1].abs() < 1e-9
                                && (tool_axis[2] - 1.0).abs() < 1e-9
                            {
                                None
                            } else {
                                Some(tool_axis)
                            };

                            let seg = make_segment(
                                current_pos,
                                target_pt,
                                SegmentKind::Arc,
                                false,
                                current_feed.unwrap_or(0.0),
                                arc_len,
                                Some([cx, cy]),
                                movarc.clockwise,
                                None,
                                active_tool,
                                active_power,
                                orientation,
                                line,
                            )?;

                            segments.push(seg);
                            segment_source_lines.push(line);
                            current_pos = [ex, ey, ez];
                            is_rapid = false;
                        } else if let Some(ref cycle) = active_cycle {
                            cycle_hole_count += 1;
                            if cycle_hole_count > limits.max_cycle_holes {
                                return Err(AptImportError {
                                    source_line: line,
                                    code: AptErrorCode::LimitExceeded,
                                    message: format!(
                                        "cycle holes {} exceeded limit {}",
                                        cycle_hole_count, limits.max_cycle_holes
                                    ),
                                });
                            }

                            let a = tool_axis;
                            let p = target_pt;
                            let r = cycle.rapto;
                            let clearance_pt = [p[0] + r * a[0], p[1] + r * a[1], p[2] + r * a[2]];

                            // Check clearance condition: (cur - (P + r*a)) . a >= -eps
                            let dot = (current_pos[0] - clearance_pt[0]) * a[0]
                                + (current_pos[1] - clearance_pt[1]) * a[1]
                                + (current_pos[2] - clearance_pt[2]) * a[2];
                            if dot < -1e-6 {
                                return Err(AptImportError {
                                    source_line: line,
                                    code: AptErrorCode::CycleClearanceViolation,
                                    message: format!(
                                        "entry position ({}, {}, {}) is below clearance plane (dot {dot} < 0)",
                                        current_pos[0], current_pos[1], current_pos[2]
                                    ),
                                });
                            }

                            let plunge_pt = [
                                p[0] - cycle.depth * a[0],
                                p[1] - cycle.depth * a[1],
                                p[2] - cycle.depth * a[2],
                            ];

                            let orientation = if tool_axis[0].abs() < 1e-9
                                && tool_axis[1].abs() < 1e-9
                                && (tool_axis[2] - 1.0).abs() < 1e-9
                            {
                                None
                            } else {
                                Some(tool_axis)
                            };

                            // s1: rapid to clearance
                            let s1_len = libm::hypot(
                                libm::hypot(
                                    clearance_pt[0] - current_pos[0],
                                    clearance_pt[1] - current_pos[1],
                                ),
                                clearance_pt[2] - current_pos[2],
                            );
                            segments.push(make_segment(
                                current_pos,
                                clearance_pt,
                                SegmentKind::Line,
                                true,
                                0.0,
                                s1_len,
                                None,
                                false,
                                None,
                                active_tool,
                                active_power,
                                orientation,
                                line,
                            )?);
                            segment_source_lines.push(line);

                            // s2: plunge to bottom
                            let s2_len = cycle.depth + cycle.rapto;
                            segments.push(make_segment(
                                clearance_pt,
                                plunge_pt,
                                SegmentKind::Line,
                                false,
                                cycle.feed,
                                s2_len,
                                None,
                                false,
                                None,
                                active_tool,
                                active_power,
                                orientation,
                                line,
                            )?);
                            segment_source_lines.push(line);

                            // s3: dwell at bottom if requested
                            if let Some(t) = cycle.dwell {
                                if t > 0.0 {
                                    segments.push(make_segment(
                                        plunge_pt,
                                        plunge_pt,
                                        SegmentKind::Dwell,
                                        true,
                                        0.0,
                                        0.0,
                                        None,
                                        false,
                                        Some(t),
                                        active_tool,
                                        active_power,
                                        None,
                                        line,
                                    )?);
                                    segment_source_lines.push(line);
                                }
                            }

                            // s4: retract to clearance
                            segments.push(make_segment(
                                plunge_pt,
                                clearance_pt,
                                SegmentKind::Line,
                                true,
                                0.0,
                                s2_len,
                                None,
                                false,
                                None,
                                active_tool,
                                active_power,
                                orientation,
                                line,
                            )?);
                            segment_source_lines.push(line);

                            current_pos = clearance_pt;
                        } else {
                            let dist = libm::hypot(
                                libm::hypot(
                                    target_pt[0] - current_pos[0],
                                    target_pt[1] - current_pos[1],
                                ),
                                target_pt[2] - current_pos[2],
                            );

                            let orientation = if tool_axis[0].abs() < 1e-9
                                && tool_axis[1].abs() < 1e-9
                                && (tool_axis[2] - 1.0).abs() < 1e-9
                            {
                                None
                            } else {
                                Some(tool_axis)
                            };

                            let seg = make_segment(
                                current_pos,
                                target_pt,
                                SegmentKind::Line,
                                is_rapid,
                                if is_rapid {
                                    0.0
                                } else {
                                    current_feed.unwrap_or(0.0)
                                },
                                dist,
                                None,
                                false,
                                None,
                                active_tool,
                                active_power,
                                orientation,
                                line,
                            )?;

                            segments.push(seg);
                            segment_source_lines.push(line);
                            current_pos = target_pt;
                            is_rapid = false;
                        }
                    }
                }
            }
        }

        let end_seg_idx = segments.len();
        if start_seg_idx < end_seg_idx {
            statement_segments.push(Some(start_seg_idx..end_seg_idx));
        } else {
            statement_segments.push(None);
        }
    }

    if pending_movarc.is_some() {
        return Err(AptImportError {
            source_line: 0,
            code: AptErrorCode::ArcEndpointMissing,
            message: "file ended with pending MOVARC without GOTO endpoint".to_string(),
        });
    }

    if units.is_none() {
        return Err(AptImportError {
            source_line: 0,
            code: AptErrorCode::UnitMissing,
            message: "program missing UNITS statement and no units assumed".to_string(),
        });
    }

    let meta = Meta {
        generator: Some("dry apt importer".to_string()),
        units: Some("mm".to_string()),
        invariants: vec!["imported-from-apt".to_string()],
        ..Meta::default()
    };

    let toolpath = Toolpath {
        version: 0,
        meta: Some(meta),
        segments,
    };

    Ok(ImportedApt {
        toolpath,
        source_lines: Vec::new(),
        segment_source_lines,
        statement_segments,
        unmodeled_statements,
        advisories,
    })
}
