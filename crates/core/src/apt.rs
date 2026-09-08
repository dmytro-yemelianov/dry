//! ISO 4343 APT-CL dialect scanner, parser, and intermediate representation.
//!
//! Handles statement tokenization with `$` line continuation, `$$` comment stripping,
//! classification into Modeled, Inert, and Hazardous statements, and error taxonomy
//! per `docs/28-apt-irbcam-dialects.md`.

pub mod lift;

use std::fmt;
use std::io::{BufRead, BufReader, Read};

pub use lift::{
    import_apt, import_apt_reader, import_apt_reader_with_limits, import_apt_reader_with_map,
    import_apt_with_limits, import_apt_with_map, import_parsed_apt_with_map, AptAdvisory,
    AptImportError, AptImportLimits, AptImportParams, AptUnits, ImportedApt,
    UnknownMajorWordPolicy, UnmodeledApt,
};

/// Error codes taxonomy for APT-CL import per §11.3 of specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AptErrorCode {
    // syntax
    Syntax,
    ContinuationUnterminated,
    NonFinite,
    GotoArity,
    LimitExceeded,
    // units/feed
    UnitMissing,
    UnitUnknown,
    UnitLate,
    FeedNegative,
    FeedUnitUnsupported,
    // orientation
    MultaxInconsistent,
    TlaxisDegenerate,
    // arc
    ArcNormalDegenerate,
    ArcNormalUnsupported,
    ArcFullTurn,
    ArcOffCircle,
    ArcSweepMismatch,
    ArcRadiusNonpositive,
    ArcEndpointMissing,
    RapidArc,
    // cycle
    CycleUnsupported,
    CycleMinorWord,
    CycleDomain,
    CycleOffInactive,
    CycleStatementInside,
    CycleClearanceViolation,
    // channels
    LoadtlDomain,
    SpindleDomain,
    SpindleOnWithoutRpm,
    SpindleDirectionUnsupported,
    IrbcamChannelCollision,
    // policy / emission
    CutcomOn,
    InsertOpaque,
    MajorWordHazardous,
    MajorWordUnknown,
    IrbcamFeedUnstated,
    IrbcamTravelSpeedUnstated,
}

impl AptErrorCode {
    /// String code name formatted as kebab-case.
    pub fn as_str(self) -> &'static str {
        match self {
            AptErrorCode::Syntax => "syntax",
            AptErrorCode::ContinuationUnterminated => "continuation-unterminated",
            AptErrorCode::NonFinite => "non-finite",
            AptErrorCode::GotoArity => "goto-arity",
            AptErrorCode::LimitExceeded => "limit-exceeded",
            AptErrorCode::UnitMissing => "unit-missing",
            AptErrorCode::UnitUnknown => "unit-unknown",
            AptErrorCode::UnitLate => "unit-late",
            AptErrorCode::FeedNegative => "feed-negative",
            AptErrorCode::FeedUnitUnsupported => "feed-unit-unsupported",
            AptErrorCode::MultaxInconsistent => "multax-inconsistent",
            AptErrorCode::TlaxisDegenerate => "tlaxis-degenerate",
            AptErrorCode::ArcNormalDegenerate => "arc-normal-degenerate",
            AptErrorCode::ArcNormalUnsupported => "arc-normal-unsupported",
            AptErrorCode::ArcFullTurn => "arc-full-turn",
            AptErrorCode::ArcOffCircle => "arc-off-circle",
            AptErrorCode::ArcSweepMismatch => "arc-sweep-mismatch",
            AptErrorCode::ArcRadiusNonpositive => "arc-radius-nonpositive",
            AptErrorCode::ArcEndpointMissing => "arc-endpoint-missing",
            AptErrorCode::RapidArc => "rapid-arc",
            AptErrorCode::CycleUnsupported => "cycle-unsupported",
            AptErrorCode::CycleMinorWord => "cycle-minor-word",
            AptErrorCode::CycleDomain => "cycle-domain",
            AptErrorCode::CycleOffInactive => "cycle-off-inactive",
            AptErrorCode::CycleStatementInside => "cycle-statement-inside",
            AptErrorCode::CycleClearanceViolation => "cycle-clearance-violation",
            AptErrorCode::LoadtlDomain => "loadtl-domain",
            AptErrorCode::SpindleDomain => "spindle-domain",
            AptErrorCode::SpindleOnWithoutRpm => "spindle-on-without-rpm",
            AptErrorCode::SpindleDirectionUnsupported => "spindle-direction-unsupported",
            AptErrorCode::IrbcamChannelCollision => "irbcam-channel-collision",
            AptErrorCode::CutcomOn => "cutcom-on",
            AptErrorCode::InsertOpaque => "insert-opaque",
            AptErrorCode::MajorWordHazardous => "major-word-hazardous",
            AptErrorCode::MajorWordUnknown => "major-word-unknown",
            AptErrorCode::IrbcamFeedUnstated => "irbcam-feed-unstated",
            AptErrorCode::IrbcamTravelSpeedUnstated => "irbcam-travel-speed-unstated",
        }
    }
}

impl fmt::Display for AptErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Fatal, located parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptParseError {
    pub source_line: usize,
    pub code: AptErrorCode,
    pub message: String,
}

impl fmt::Display for AptParseError {
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

impl std::error::Error for AptParseError {}

/// Spindle rotation direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpindleDirection {
    Clw,
    Cclw,
}

/// Feedrate unit specifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FedratUnit {
    Mmpm,
    Ipm,
}

/// Motion record representing modeled motion.
#[derive(Debug, Clone, PartialEq)]
pub enum AptMotion {
    Goto(Vec<f64>),
    Movarc {
        center: [f64; 3],
        normal: [f64; 3],
        radius: f64,
        sweep_deg: f64,
    },
    Circle {
        center: [f64; 3],
        axis: [f64; 3],
        radius: f64,
    },
}

/// State change record.
#[derive(Debug, Clone, PartialEq)]
pub enum AptState {
    Units(AptUnits),
    Multax(bool),
    Fedrat {
        unit: Option<FedratUnit>,
        value: f64,
    },
    Rapid,
    CycleDrill {
        depth: f64,
        feed_unit: Option<FedratUnit>,
        feed: f64,
        rapto: f64,
        dwell: Option<f64>,
    },
    CycleOff,
}

/// Process channel record.
#[derive(Debug, Clone, PartialEq)]
pub enum AptProcess {
    Loadtl {
        tool: u32,
        extra: Vec<String>,
    },
    SpindlRpm {
        rpm: f64,
        direction: Option<SpindleDirection>,
    },
    SpindlOn,
    SpindlOff,
    Delay(f64),
    Dwell(f64),
}

/// Refusal reason for hazardous or unmodeled statements.
#[derive(Debug, Clone, PartialEq)]
pub enum AptRefusalReason {
    Hazardous(String),
    UnknownMajor(String),
    Cutcom(String),
    Insert(String),
    CycleUnsupported(String),
    FeedUnitUnsupported(String),
    GotoArity(usize),
    NonFinite(String),
}

/// High-level statement record after syntax parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum AptRecord {
    Motion(Box<AptMotion>),
    State(AptState),
    Process(AptProcess),
    Inert {
        major: String,
    },
    Refused {
        major: String,
        reason: AptRefusalReason,
    },
    Comment,
    Empty,
}

/// Parsed APT statement with source location tracking.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAptStatement {
    pub source_line: usize,
    pub line_count: usize,
    pub raw: String,
    pub record: AptRecord,
}

/// APT scanner and statement parser streaming from a reader.
pub struct AptParser<R: Read> {
    reader: BufReader<R>,
    limits: AptImportLimits,
    current_line: usize,
    total_bytes: usize,
    statement_count: usize,
}

impl<R: Read> AptParser<R> {
    /// Creates a new parser with resource limits.
    pub fn new(reader: R, limits: AptImportLimits) -> Self {
        Self {
            reader: BufReader::new(reader),
            limits,
            current_line: 0,
            total_bytes: 0,
            statement_count: 0,
        }
    }

    /// Read the next parsed statement from the source.
    pub fn next_statement(&mut self) -> Result<Option<ParsedAptStatement>, AptParseError> {
        let mut raw_lines = Vec::new();
        let start_line = self.current_line + 1;
        let mut line_buf = String::new();
        let mut statement_content = String::new();
        let mut continuation_count = 0;

        loop {
            line_buf.clear();
            let bytes_read = self
                .reader
                .read_line(&mut line_buf)
                .map_err(|e| AptParseError {
                    source_line: self.current_line + 1,
                    code: AptErrorCode::Syntax,
                    message: format!("IO error reading line: {e}"),
                })?;

            if bytes_read == 0 {
                if continuation_count > 0 {
                    return Err(AptParseError {
                        source_line: start_line,
                        code: AptErrorCode::ContinuationUnterminated,
                        message: "statement continuation '$' at end of file".to_string(),
                    });
                }
                return Ok(None);
            }

            self.current_line += 1;
            self.total_bytes += bytes_read;

            if self.total_bytes > self.limits.max_source_bytes {
                return Err(AptParseError {
                    source_line: self.current_line,
                    code: AptErrorCode::LimitExceeded,
                    message: format!(
                        "source bytes {} exceeded limit {}",
                        self.total_bytes, self.limits.max_source_bytes
                    ),
                });
            }

            let trimmed_line = line_buf.trim_end_matches(&['\r', '\n'][..]);
            if trimmed_line.len() > self.limits.max_line_bytes {
                return Err(AptParseError {
                    source_line: self.current_line,
                    code: AptErrorCode::LimitExceeded,
                    message: format!(
                        "line bytes {} exceeded limit {}",
                        trimmed_line.len(),
                        self.limits.max_line_bytes
                    ),
                });
            }

            raw_lines.push(trimmed_line.to_string());

            // Strip line comments starting with $$
            let content_without_comment = match trimmed_line.split_once("$$") {
                Some((before, _)) => before,
                None => trimmed_line,
            };

            let trimmed_content = content_without_comment.trim();

            if trimmed_content.is_empty() {
                if continuation_count == 0 {
                    let is_comment = trimmed_line.trim_start().starts_with("$$");
                    return Ok(Some(ParsedAptStatement {
                        source_line: self.current_line,
                        line_count: 1,
                        raw: trimmed_line.to_string(),
                        record: if is_comment {
                            AptRecord::Comment
                        } else {
                            AptRecord::Empty
                        },
                    }));
                } else {
                    continue;
                }
            }

            if let Some(stripped) = trimmed_content.strip_suffix('$') {
                continuation_count += 1;
                if continuation_count > self.limits.max_continuation_lines {
                    return Err(AptParseError {
                        source_line: start_line,
                        code: AptErrorCode::LimitExceeded,
                        message: format!(
                            "continuation lines {} exceeded limit {}",
                            continuation_count, self.limits.max_continuation_lines
                        ),
                    });
                }
                let piece = stripped.trim();
                if !statement_content.is_empty() {
                    statement_content.push(' ');
                }
                statement_content.push_str(piece);
            } else {
                if !statement_content.is_empty() {
                    statement_content.push(' ');
                }
                statement_content.push_str(trimmed_content);
                break;
            }
        }

        self.statement_count += 1;
        if self.statement_count > self.limits.max_statements {
            return Err(AptParseError {
                source_line: start_line,
                code: AptErrorCode::LimitExceeded,
                message: format!(
                    "statement count {} exceeded limit {}",
                    self.statement_count, self.limits.max_statements
                ),
            });
        }

        let raw_statement = raw_lines.join("\n");
        let record = parse_statement_record(&statement_content, start_line, &self.limits)?;

        Ok(Some(ParsedAptStatement {
            source_line: start_line,
            line_count: raw_lines.len(),
            raw: raw_statement,
            record,
        }))
    }
}

/// Parses full APT source into parsed statement list.
pub fn parse_apt_statements(
    source: &str,
    limits: &AptImportLimits,
) -> Result<Vec<ParsedAptStatement>, AptParseError> {
    let mut parser = AptParser::new(source.as_bytes(), *limits);
    let mut statements = Vec::new();
    while let Some(stmt) = parser.next_statement()? {
        statements.push(stmt);
    }
    Ok(statements)
}

fn parse_number(token: &str, source_line: usize) -> Result<f64, AptParseError> {
    match token.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(AptParseError {
            source_line,
            code: AptErrorCode::NonFinite,
            message: format!("token '{token}' is non-finite or malformed number"),
        }),
    }
}

fn parse_statement_record(
    stmt: &str,
    source_line: usize,
    limits: &AptImportLimits,
) -> Result<AptRecord, AptParseError> {
    let (major_raw, params_raw) = match stmt.split_once('/') {
        Some((m, p)) => (m.trim(), Some(p.trim())),
        None => (stmt.trim(), None),
    };

    let major = major_raw.to_ascii_uppercase();
    let tokens: Vec<&str> = match params_raw {
        Some(p) if !p.is_empty() => p.split(',').map(str::trim).collect(),
        _ => Vec::new(),
    };

    if tokens.len() > limits.max_values_per_statement {
        return Err(AptParseError {
            source_line,
            code: AptErrorCode::LimitExceeded,
            message: format!(
                "tokens {} exceeded limit {}",
                tokens.len(),
                limits.max_values_per_statement
            ),
        });
    }

    match major.as_str() {
        "UNITS" => {
            if tokens.is_empty() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::UnitUnknown,
                    message: "UNITS statement requires minor word (MM or INCHES)".to_string(),
                });
            }
            match tokens[0].to_ascii_uppercase().as_str() {
                "MM" => Ok(AptRecord::State(AptState::Units(AptUnits::Mm))),
                "INCHES" => Ok(AptRecord::State(AptState::Units(AptUnits::Inches))),
                other => Err(AptParseError {
                    source_line,
                    code: AptErrorCode::UnitUnknown,
                    message: format!("unknown unit '{other}' (expected MM or INCHES)"),
                }),
            }
        }
        "MULTAX" => {
            if tokens.is_empty() || tokens[0].eq_ignore_ascii_case("ON") {
                Ok(AptRecord::State(AptState::Multax(true)))
            } else if tokens[0].eq_ignore_ascii_case("OFF") {
                Ok(AptRecord::State(AptState::Multax(false)))
            } else {
                Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: format!("unknown MULTAX minor word '{}'", tokens[0]),
                })
            }
        }
        "FEDRAT" => {
            if tokens.is_empty() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: "FEDRAT requires a feed value".to_string(),
                });
            }
            let first_upper = tokens[0].to_ascii_uppercase();
            if first_upper == "PERREV" || first_upper.starts_with("PER") {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::FeedUnitUnsupported,
                    message: "FEDRAT/PERREV is unsupported".to_string(),
                });
            }
            let (unit, val_str) = if first_upper == "MMPM" {
                if tokens.len() < 2 {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::Syntax,
                        message: "FEDRAT/MMPM requires speed value".to_string(),
                    });
                }
                (Some(FedratUnit::Mmpm), tokens[1])
            } else if first_upper == "IPM" {
                if tokens.len() < 2 {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::Syntax,
                        message: "FEDRAT/IPM requires speed value".to_string(),
                    });
                }
                (Some(FedratUnit::Ipm), tokens[1])
            } else {
                (None, tokens[0])
            };

            let feed = parse_number(val_str, source_line)?;
            if feed < 0.0 {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::FeedNegative,
                    message: format!("FEDRAT value {feed} cannot be negative"),
                });
            }
            Ok(AptRecord::State(AptState::Fedrat { unit, value: feed }))
        }
        "RAPID" => Ok(AptRecord::State(AptState::Rapid)),
        "GOTO" => {
            if tokens.len() != 3 && tokens.len() != 6 {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::GotoArity,
                    message: format!(
                        "GOTO requires exactly 3 or 6 coordinates, got {}",
                        tokens.len()
                    ),
                });
            }
            let mut coords = Vec::with_capacity(tokens.len());
            for t in tokens {
                coords.push(parse_number(t, source_line)?);
            }
            Ok(AptRecord::Motion(Box::new(AptMotion::Goto(coords))))
        }
        "MOVARC" => {
            if tokens.len() != 8 {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: format!(
                        "MOVARC requires 8 values (cx, cy, cz, nx, ny, nz, r, theta), got {}",
                        tokens.len()
                    ),
                });
            }
            let cx = parse_number(tokens[0], source_line)?;
            let cy = parse_number(tokens[1], source_line)?;
            let cz = parse_number(tokens[2], source_line)?;
            let nx = parse_number(tokens[3], source_line)?;
            let ny = parse_number(tokens[4], source_line)?;
            let nz = parse_number(tokens[5], source_line)?;
            let r = parse_number(tokens[6], source_line)?;
            let theta = parse_number(tokens[7], source_line)?;

            Ok(AptRecord::Motion(Box::new(AptMotion::Movarc {
                center: [cx, cy, cz],
                normal: [nx, ny, nz],
                radius: r,
                sweep_deg: theta,
            })))
        }
        "CIRCLE" => {
            if tokens.len() < 7 {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: format!(
                        "CIRCLE requires at least 7 values (cx, cy, cz, i, j, k, r), got {}",
                        tokens.len()
                    ),
                });
            }
            let cx = parse_number(tokens[0], source_line)?;
            let cy = parse_number(tokens[1], source_line)?;
            let cz = parse_number(tokens[2], source_line)?;
            let i = parse_number(tokens[3], source_line)?;
            let j = parse_number(tokens[4], source_line)?;
            let k = parse_number(tokens[5], source_line)?;
            let r = parse_number(tokens[6], source_line)?;

            Ok(AptRecord::Motion(Box::new(AptMotion::Circle {
                center: [cx, cy, cz],
                axis: [i, j, k],
                radius: r,
            })))
        }
        "CYCLE" => {
            if tokens.is_empty() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: "CYCLE requires minor word".to_string(),
                });
            }
            let sub = tokens[0].to_ascii_uppercase();
            if sub == "OFF" {
                return Ok(AptRecord::State(AptState::CycleOff));
            }
            if sub != "DRILL" {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::CycleUnsupported,
                    message: format!("CYCLE/{sub} is unsupported; only DRILL is modeled"),
                });
            }

            // Parse CYCLE/DRILL, [DEPTH,] depth, [MMPM|IPM|PERMIN,] feed, [CLEAR, c,] RAPTO, r [, DWELL, t]
            let mut idx = 1;
            if idx < tokens.len() && tokens[idx].eq_ignore_ascii_case("DEPTH") {
                idx += 1;
            }
            if idx >= tokens.len() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: "CYCLE/DRILL missing depth".to_string(),
                });
            }
            let depth = parse_number(tokens[idx], source_line)?;
            idx += 1;

            let mut feed_unit = None;
            if idx < tokens.len() {
                let tok_upper = tokens[idx].to_ascii_uppercase();
                if tok_upper == "MMPM" {
                    feed_unit = Some(FedratUnit::Mmpm);
                    idx += 1;
                } else if tok_upper == "IPM" {
                    feed_unit = Some(FedratUnit::Ipm);
                    idx += 1;
                } else if tok_upper == "PERMIN" {
                    idx += 1;
                }
            }

            if idx >= tokens.len() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: "CYCLE/DRILL missing feed".to_string(),
                });
            }
            let feed = parse_number(tokens[idx], source_line)?;
            idx += 1;

            if idx < tokens.len() && tokens[idx].eq_ignore_ascii_case("CLEAR") {
                idx += 1;
                if idx >= tokens.len() {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::Syntax,
                        message: "CYCLE/DRILL CLEAR missing distance value".to_string(),
                    });
                }
                let _clear = parse_number(tokens[idx], source_line)?;
                idx += 1;
            }

            if idx >= tokens.len() || !tokens[idx].eq_ignore_ascii_case("RAPTO") {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::CycleMinorWord,
                    message: "CYCLE/DRILL requires RAPTO word".to_string(),
                });
            }
            idx += 1;

            if idx >= tokens.len() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: "CYCLE/DRILL RAPTO missing distance value".to_string(),
                });
            }
            let rapto = parse_number(tokens[idx], source_line)?;
            idx += 1;

            let mut dwell = None;
            if idx < tokens.len() {
                if tokens[idx].eq_ignore_ascii_case("DWELL") {
                    idx += 1;
                    if idx >= tokens.len() {
                        return Err(AptParseError {
                            source_line,
                            code: AptErrorCode::Syntax,
                            message: "CYCLE/DRILL DWELL missing seconds value".to_string(),
                        });
                    }
                    dwell = Some(parse_number(tokens[idx], source_line)?);
                    idx += 1;
                } else {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::CycleMinorWord,
                        message: format!("unexpected minor word in CYCLE/DRILL: '{}'", tokens[idx]),
                    });
                }
            }

            if idx < tokens.len() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::CycleMinorWord,
                    message: format!("extraneous minor words in CYCLE/DRILL: '{}'", tokens[idx]),
                });
            }

            if depth <= 0.0 || feed <= 0.0 || rapto < 0.0 || dwell.unwrap_or(0.0) < 0.0 {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::CycleDomain,
                    message: "CYCLE/DRILL parameters must satisfy depth > 0, feed > 0, rapto >= 0, dwell >= 0"
                        .to_string(),
                });
            }

            Ok(AptRecord::State(AptState::CycleDrill {
                depth,
                feed_unit,
                feed,
                rapto,
                dwell,
            }))
        }
        "DELAY" | "DWELL" => {
            if tokens.is_empty() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: format!("{major} requires seconds duration"),
                });
            }
            let t = parse_number(tokens[0], source_line)?;
            if t < 0.0 {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::NonFinite,
                    message: format!("{major} duration {t} cannot be negative"),
                });
            }
            if major == "DELAY" {
                Ok(AptRecord::Process(AptProcess::Delay(t)))
            } else {
                Ok(AptRecord::Process(AptProcess::Dwell(t)))
            }
        }
        "LOADTL" => {
            if tokens.is_empty() {
                return Err(AptParseError {
                    source_line,
                    code: AptErrorCode::Syntax,
                    message: "LOADTL requires tool number".to_string(),
                });
            }
            let tool_num: u32 = match tokens[0].trim().parse() {
                Ok(n) => n,
                Err(_) => {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::LoadtlDomain,
                        message: format!("invalid tool number literal '{}'", tokens[0]),
                    });
                }
            };
            let extra = tokens[1..].iter().map(|s| s.to_string()).collect();
            Ok(AptRecord::Process(AptProcess::Loadtl {
                tool: tool_num,
                extra,
            }))
        }
        "SPINDL" => {
            if tokens.is_empty() || tokens[0].eq_ignore_ascii_case("ON") {
                Ok(AptRecord::Process(AptProcess::SpindlOn))
            } else if tokens[0].eq_ignore_ascii_case("OFF") {
                Ok(AptRecord::Process(AptProcess::SpindlOff))
            } else {
                let mut idx = 0;
                if tokens[idx].eq_ignore_ascii_case("RPM") {
                    idx += 1;
                }
                if idx >= tokens.len() {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::Syntax,
                        message: "SPINDL/RPM requires speed value".to_string(),
                    });
                }
                let rpm_str = tokens[idx];
                let rpm = match parse_number(rpm_str, source_line) {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(AptParseError {
                            source_line,
                            code: AptErrorCode::MajorWordHazardous,
                            message: format!("SPINDL speed '{rpm_str}' is non-numeric"),
                        });
                    }
                };
                if rpm < 0.0 {
                    return Err(AptParseError {
                        source_line,
                        code: AptErrorCode::SpindleDomain,
                        message: format!("spindle speed {rpm} cannot be negative"),
                    });
                }
                idx += 1;

                let mut direction = None;
                if idx < tokens.len() {
                    let dir_str = tokens[idx].to_ascii_uppercase();
                    if dir_str == "CLW" {
                        direction = Some(SpindleDirection::Clw);
                    } else if dir_str == "CCLW" {
                        direction = Some(SpindleDirection::Cclw);
                    }
                }

                Ok(AptRecord::Process(AptProcess::SpindlRpm { rpm, direction }))
            }
        }
        // Inert-preserved
        "PARTNO" | "PPRINT" | "MACHIN" | "FINI" | "COOLNT" | "CLRSRF" | "REMARK" => {
            Ok(AptRecord::Inert { major })
        }
        // Refused-hazardous
        "CUTCOM" => {
            if tokens.first().map(|s| s.eq_ignore_ascii_case("OFF")) == Some(true) {
                Ok(AptRecord::Inert { major })
            } else {
                Ok(AptRecord::Refused {
                    major,
                    reason: AptRefusalReason::Cutcom(
                        "CUTCOM other than OFF is refused: tool radius compensation would alter geometry"
                            .to_string(),
                    ),
                })
            }
        }
        "INSERT" => Ok(AptRecord::Refused {
            major,
            reason: AptRefusalReason::Insert(
                "INSERT is refused: post-specific verbatim text cannot be semantically carried"
                    .to_string(),
            ),
        }),
        "TRANS" | "MATRIX" | "ORIGIN" | "ROTABL" | "ROTHED" | "TRACUT" | "COPY" | "INDEX"
        | "MODE" => Ok(AptRecord::Refused {
            major: major.clone(),
            reason: AptRefusalReason::Hazardous(format!(
                "hazardous coordinate or transformation major word '{major}' is refused"
            )),
        }),
        _ => Ok(AptRecord::Refused {
            major: major.clone(),
            reason: AptRefusalReason::UnknownMajor(format!("unknown major word '{major}'")),
        }),
    }
}
