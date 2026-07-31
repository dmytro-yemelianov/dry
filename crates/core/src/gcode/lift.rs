use super::{
    parse_gcode_lines_with_state, DistanceMode, ExtrusionMode, GcodeModalState, GcodeParseError,
    GcodeParser, GcodeRecord, MotionMode, ParsedGcodeLine, ProcessCommand, StateCommand, UnitMode,
};
use crate::emit::{emit_stream, format_number, EmitParams};
use crate::ir::{Meta, Segment, SegmentKind, Toolpath};
use crate::units::{Angle, Area, Feedrate, Length, Volume};
use std::f64::consts::{PI, TAU};
use std::io::Read;

/// Parameters for lifting parsed G-code motion into Dry L2.
///
/// G-code does not carry enough information to reconstruct line width/layer height reliably, so those
/// fields are optional. Filament diameter is enough to recover deposited volume from positive E motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GcodeImportParams {
    pub version: u32,
    pub filament_diameter: f64,
    pub line_width: Option<f64>,
    pub layer_height: Option<f64>,
    pub relative_e: bool,
}

impl Default for GcodeImportParams {
    fn default() -> Self {
        GcodeImportParams {
            version: 0,
            filament_diameter: 1.75,
            line_width: None,
            layer_height: None,
            relative_e: false,
        }
    }
}

/// A located G-code import error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcodeImportError {
    pub source_line: usize,
    pub message: String,
}

impl GcodeImportError {
    fn new(source_line: usize, message: impl Into<String>) -> Self {
        GcodeImportError {
            source_line,
            message: message.into(),
        }
    }
}

impl From<GcodeParseError> for GcodeImportError {
    fn from(value: GcodeParseError) -> Self {
        GcodeImportError {
            source_line: value.source_line,
            message: value.message,
        }
    }
}

impl std::fmt::Display for GcodeImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.source_line, self.message)
    }
}

impl std::error::Error for GcodeImportError {}

/// A lifted G-code program plus provenance from Dry segments back to source lines.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedGcode {
    pub toolpath: Toolpath,
    pub source_lines: Vec<String>,
    pub segment_source_lines: Vec<usize>,
    pub source_line_segments: Vec<Option<usize>>,
    /// Explicit G/M commands the importer preserves but does not model or verify.
    pub unmodeled_commands: Vec<UnmodeledGcode>,
}

/// A source-located command that survives source-preserving output but is opaque to Dry's verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmodeledGcode {
    pub source_line: usize,
    pub command: String,
    pub raw: String,
}

/// A contiguous source block containing only imported motion records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcodeMotionSpan {
    pub first_source_line: usize,
    pub last_source_line: usize,
    pub first_segment: usize,
    pub segment_count: usize,
}

impl GcodeMotionSpan {
    pub fn segment_range(self) -> std::ops::Range<usize> {
        self.first_segment..self.first_segment + self.segment_count
    }
}

impl ImportedGcode {
    pub fn source_line_for_segment(&self, segment: usize) -> Option<usize> {
        self.segment_source_lines.get(segment).copied()
    }

    pub fn segment_for_source_line(&self, source_line: usize) -> Option<usize> {
        source_line
            .checked_sub(1)
            .and_then(|idx| self.source_line_segments.get(idx))
            .copied()
            .flatten()
    }

    pub fn source_text(&self) -> String {
        self.source_lines.join("\n")
    }

    pub fn motion_spans(&self) -> Vec<GcodeMotionSpan> {
        let mut spans = Vec::new();
        let mut idx = 0;
        while idx < self.source_line_segments.len() {
            let Some(first_segment) = self.source_line_segments[idx] else {
                idx += 1;
                continue;
            };
            let first_source_line = idx + 1;
            let mut last_source_line = first_source_line;
            let mut segment_count = 1;
            idx += 1;
            while idx < self.source_line_segments.len() {
                let expected_segment = first_segment + segment_count;
                if self.source_line_segments[idx] != Some(expected_segment) {
                    break;
                }
                last_source_line = idx + 1;
                segment_count += 1;
                idx += 1;
            }
            spans.push(GcodeMotionSpan {
                first_source_line,
                last_source_line,
                first_segment,
                segment_count,
            });
        }
        spans
    }

    pub fn splice_motion_lines(
        &self,
        motion_lines: &[String],
    ) -> Result<Vec<String>, GcodeImportError> {
        if motion_lines.len() != self.segment_source_lines.len() {
            return Err(GcodeImportError::new(
                0,
                format!(
                    "cannot splice {} emitted motion lines into {} imported motion slots",
                    motion_lines.len(),
                    self.segment_source_lines.len()
                ),
            ));
        }

        let mut lines = self.source_lines.clone();
        for (segment, line) in motion_lines.iter().enumerate() {
            let source_line = self.segment_source_lines[segment];
            let Some(slot) = source_line
                .checked_sub(1)
                .and_then(|idx| lines.get_mut(idx))
            else {
                return Err(GcodeImportError::new(
                    source_line,
                    "segment source line is outside the imported source",
                ));
            };
            *slot = line.clone();
        }
        Ok(lines)
    }

    pub fn splice_motion_spans(
        &self,
        span_motion_lines: &[Vec<String>],
    ) -> Result<Vec<String>, GcodeImportError> {
        let spans = self.motion_spans();
        if span_motion_lines.len() != spans.len() {
            return Err(GcodeImportError::new(
                0,
                format!(
                    "cannot splice {} emitted motion spans into {} imported motion spans",
                    span_motion_lines.len(),
                    spans.len()
                ),
            ));
        }

        let mut lines = Vec::new();
        let mut source_idx = 0;
        for (span, motion_lines) in spans.into_iter().zip(span_motion_lines) {
            let start = span.first_source_line - 1;
            let end = span.last_source_line;
            lines.extend(self.source_lines[source_idx..start].iter().cloned());
            lines.extend(motion_lines.iter().cloned());
            source_idx = end;
        }
        lines.extend(self.source_lines[source_idx..].iter().cloned());
        Ok(lines)
    }

    pub fn emit_source_preserving(
        &self,
        toolpath: &Toolpath,
        params: &EmitParams,
    ) -> Result<Vec<String>, GcodeImportError> {
        let mut span_toolpaths = Vec::new();
        for span in self.motion_spans() {
            let range = span.segment_range();
            if range.end > toolpath.segments.len() {
                return Err(GcodeImportError::new(
                    span.last_source_line,
                    "replacement toolpath has fewer segments than the imported source map",
                ));
            }
            span_toolpaths.push(Toolpath {
                version: toolpath.version,
                meta: toolpath.meta.clone(),
                segments: toolpath.segments[range].to_vec(),
            });
        }
        self.emit_source_preserving_spans(&span_toolpaths, params)
    }

    pub fn emit_source_preserving_spans(
        &self,
        span_toolpaths: &[Toolpath],
        params: &EmitParams,
    ) -> Result<Vec<String>, GcodeImportError> {
        let span_motion_lines = self.emit_normalized_span_lines(span_toolpaths, params)?;
        self.splice_motion_spans(&span_motion_lines)
    }

    /// Emit each span's motion, prefixed by the modal prologue that makes the span self-contained.
    ///
    /// The RS-274 program frame is emitted once per *program*, never per span: with a
    /// `cnc_frame` set, `emit` would prepend the preamble to the whole-program stream *and* to
    /// every per-span stream, so the line accounting below would desync and each splice would
    /// carry a duplicate frame. Callers may hand us a profile's `EmitParams` verbatim, so the
    /// invariant is enforced here rather than left to every caller.
    fn emit_normalized_span_lines(
        &self,
        span_toolpaths: &[Toolpath],
        params: &EmitParams,
    ) -> Result<Vec<Vec<String>>, GcodeImportError> {
        let params = &EmitParams {
            cnc_frame: None,
            ..params.clone()
        };
        let reset_flow = self
            .toolpath
            .segments
            .iter()
            .any(|segment| segment.flow.is_some());
        let flat_toolpath = Toolpath {
            version: self.toolpath.version,
            meta: self.toolpath.meta.clone(),
            segments: span_toolpaths
                .iter()
                .flat_map(|toolpath| toolpath.segments.iter().cloned())
                .collect(),
        };
        // the fallible emit entry point: a refused program must surface as an import error here, not
        // as an empty line vector that the accounting below would read as "0 lines, all consumed".
        let emitted = emit_toolpath_lines(&flat_toolpath, params)?;
        let mut emitted_offset = 0usize;
        let mut absolute_e_start = Length::ZERO;
        let mut span_motion_lines = Vec::with_capacity(span_toolpaths.len());

        for span_toolpath in span_toolpaths {
            let span_line_count = emit_toolpath_lines(span_toolpath, params)?.len();
            let end = emitted_offset + span_line_count;
            if end > emitted.len() {
                return Err(GcodeImportError::new(
                    0,
                    format!(
                        "internal rewrite line accounting failed: span needs {span_line_count} lines at offset {emitted_offset}, but only {} total lines were emitted",
                        emitted.len()
                    ),
                ));
            }
            let mut lines = modal_rewrite_prologue(params, reset_flow, absolute_e_start);
            lines.extend(emitted[emitted_offset..end].iter().cloned());
            emitted_offset = end;
            for segment in &span_toolpath.segments {
                absolute_e_start = absolute_e_start + segment.filament;
            }
            span_motion_lines.push(lines);
        }

        if emitted_offset != emitted.len() {
            return Err(GcodeImportError::new(
                0,
                format!(
                    "internal rewrite line accounting failed: consumed {emitted_offset} of {} emitted lines",
                    emitted.len()
                ),
            ));
        }

        Ok(span_motion_lines)
    }
}

fn emit_toolpath_lines(
    toolpath: &Toolpath,
    params: &EmitParams,
) -> Result<Vec<String>, GcodeImportError> {
    emit_stream(toolpath.segments.iter().cloned().map(Ok), params)
        .map_err(|error| GcodeImportError::new(0, format!("emit refused the rewrite: {error}")))
}

fn modal_rewrite_prologue(
    params: &EmitParams,
    reset_flow: bool,
    absolute_e_start: Length,
) -> Vec<String> {
    // G21/G90 are universal; every other line here addresses a filament axis. CNC, laser and robot
    // controllers have none, and an unknown M-code aborts the program on LinuxCNC/Fanuc — so the
    // extruder modals are gated on the same predicate the emitter uses to decide whether to write
    // `E` words at all.
    let mut lines = vec!["G21".to_string(), "G90".to_string()];
    if params.flavor.has_extruder() {
        lines.push(if params.relative_e { "M83" } else { "M82" }.to_string());
        if !params.relative_e {
            lines.push(format!("G92 E{}", format_number(absolute_e_start.value())));
        }
        if reset_flow {
            lines.push("M221 S100".to_string());
        }
    }
    lines
}

#[derive(Debug, Clone)]
struct LiftState {
    pos: [Option<f64>; 3],
    e: f64,
    feedrate: Option<f64>,
    temperature: Option<f64>,
    fan: Option<f64>,
    flow: f64,
    tool: Option<u32>,
}

impl Default for LiftState {
    fn default() -> Self {
        LiftState {
            pos: [None, None, None],
            e: 0.0,
            feedrate: None,
            temperature: None,
            fan: None,
            flow: 1.0,
            tool: None,
        }
    }
}

/// Parse and lift a G-code string into a Dry L2 [`Toolpath`].
pub fn import_gcode(
    source: &str,
    params: &GcodeImportParams,
) -> Result<Toolpath, GcodeImportError> {
    Ok(import_gcode_with_map(source, params)?.toolpath)
}

/// Parse and lift a G-code string into Dry L2 with source-line mapping.
pub fn import_gcode_with_map(
    source: &str,
    params: &GcodeImportParams,
) -> Result<ImportedGcode, GcodeImportError> {
    let mut initial_state = GcodeModalState::default();
    if params.relative_e {
        initial_state.extrusion_mode = ExtrusionMode::Relative;
    }
    import_parsed_gcode_with_map(parse_gcode_lines_with_state(source, initial_state)?, params)
}

/// Parse and lift a G-code reader into a Dry L2 [`Toolpath`].
pub fn import_gcode_reader<R: Read>(
    reader: R,
    params: &GcodeImportParams,
) -> Result<Toolpath, GcodeImportError> {
    Ok(import_gcode_reader_with_map(reader, params)?.toolpath)
}

/// Parse and lift a G-code reader into Dry L2 with source-line mapping.
pub fn import_gcode_reader_with_map<R: Read>(
    reader: R,
    params: &GcodeImportParams,
) -> Result<ImportedGcode, GcodeImportError> {
    let mut initial_state = GcodeModalState::default();
    if params.relative_e {
        initial_state.extrusion_mode = ExtrusionMode::Relative;
    }
    let lines = GcodeParser::from_reader(reader)
        .with_state(initial_state)
        .collect::<Result<Vec<_>, _>>()?;
    import_parsed_gcode_with_map(lines, params)
}

/// Lift already parsed G-code lines into a Dry L2 [`Toolpath`].
pub fn import_parsed_gcode<I>(
    lines: I,
    params: &GcodeImportParams,
) -> Result<Toolpath, GcodeImportError>
where
    I: IntoIterator<Item = ParsedGcodeLine>,
{
    Ok(import_parsed_gcode_with_map(lines, params)?.toolpath)
}

/// Lift already parsed G-code lines into Dry L2 with source-line mapping.
pub fn import_parsed_gcode_with_map<I>(
    lines: I,
    params: &GcodeImportParams,
) -> Result<ImportedGcode, GcodeImportError>
where
    I: IntoIterator<Item = ParsedGcodeLine>,
{
    validate_params(params)?;
    let filament_area = filament_area(params.filament_diameter);
    let width = params.line_width.map(Length::mm);
    let height = params.layer_height.map(Length::mm);
    let mut state = LiftState::default();
    let mut segments = Vec::new();
    let mut source_lines = Vec::new();
    let mut segment_source_lines = Vec::new();
    let mut source_line_segments = Vec::new();
    let mut unmodeled_commands = Vec::new();

    for line in lines {
        source_lines.push(line.raw.clone());
        source_line_segments.push(None);
        match &line.record {
            GcodeRecord::Motion(motion) => {
                if let Some(segment) =
                    lift_motion(motion, filament_area, width, height, &mut state)?
                {
                    let segment_idx = segments.len();
                    segments.push(segment);
                    segment_source_lines.push(motion.source_line);
                    if let Some(slot) = source_line_segments.last_mut() {
                        *slot = Some(segment_idx);
                    }
                }
            }
            GcodeRecord::State(StateCommand::SetPosition) => apply_g92(&line, &mut state)?,
            GcodeRecord::Process(command) => apply_process(*command, &mut state),
            GcodeRecord::Other { letter, value } => {
                let value = if value.fract() == 0.0 {
                    format!("{value:.0}")
                } else {
                    value.to_string()
                };
                unmodeled_commands.push(UnmodeledGcode {
                    source_line: line.source_line,
                    command: format!("{letter}{value}"),
                    raw: line.raw.clone(),
                });
            }
            _ => {}
        }
    }

    Ok(ImportedGcode {
        toolpath: Toolpath {
            version: params.version,
            meta: Some(Meta {
                generator: Some("dry gcode importer".to_string()),
                units: Some("mm".to_string()),
                source_hash: None,
                invariants: vec!["imported-from-gcode".to_string()],
            }),
            segments,
        },
        source_lines,
        segment_source_lines,
        source_line_segments,
        unmodeled_commands,
    })
}

fn validate_params(params: &GcodeImportParams) -> Result<(), GcodeImportError> {
    if !params.filament_diameter.is_finite() || params.filament_diameter <= 0.0 {
        return Err(GcodeImportError::new(
            0,
            "filament_diameter must be finite and positive",
        ));
    }
    // A finite diameter can still square to a non-finite cross-section, which would then multiply
    // every deposited length into a non-finite `volume`.
    if !filament_area(params.filament_diameter).value().is_finite() {
        return Err(GcodeImportError::new(
            0,
            "filament_diameter is too large to give a finite cross-section",
        ));
    }
    for (name, value) in [
        ("line_width", params.line_width),
        ("layer_height", params.layer_height),
    ] {
        if let Some(value) = value {
            if !value.is_finite() || value <= 0.0 {
                return Err(GcodeImportError::new(
                    0,
                    format!("{name} must be finite and positive when set"),
                ));
            }
        }
    }
    Ok(())
}

fn filament_area(diameter: f64) -> Area {
    let radius = diameter / 2.0;
    Area(PI * radius * radius)
}

fn unit_factor(units: UnitMode) -> f64 {
    match units {
        UnitMode::Millimeters => 1.0,
        UnitMode::Inches => 25.4,
    }
}

fn lift_motion(
    motion: &super::MotionRecord,
    filament_area: Area,
    width: Option<Length>,
    height: Option<Length>,
    state: &mut LiftState,
) -> Result<Option<Segment>, GcodeImportError> {
    let factor = unit_factor(motion.state.units);
    if let Some(f) = motion.f {
        // A negative feedrate is not a slow move — it has no meaning on any machine, and it used to
        // reach `simulate` as a *negative* duration subtracted from the total. (Non-finite values
        // were already refused by the word scanner.)
        if f < 0.0 {
            return Err(GcodeImportError::new(
                motion.source_line,
                format!("feedrate F{f} must not be negative"),
            ));
        }
        // Inches scale the word by 25.4, which overflows a finite `F` to `inf` (`G20` + `F1e307`).
        let scaled = f * factor;
        if !scaled.is_finite() {
            return Err(GcodeImportError::new(
                motion.source_line,
                format!("feedrate F{f} is not finite after unit conversion"),
            ));
        }
        state.feedrate = Some(scaled);
    }
    // Zero means "not stated by this file": motion before the first `F` inherits the machine's
    // modal feedrate, which the program does not record. It stays accepted — the program is valid
    // on the machine — and such a move still contributes nothing to `simulate`'s metrics: that is
    // the branch `Dry.Semantics.SimulateMetrics.segmentMotionTime` models, and an attempt to change
    // it failed the `FM1.SIMULATE_METRICS` refinement corpus. See `engine::segment_motion_time` and
    // the pin in `crates/core/tests/ingress_validation.rs`.
    let speed = Feedrate(state.feedrate.unwrap_or(0.0));

    if motion.mode == MotionMode::Dwell {
        let dwell_s = motion.s.or_else(|| motion.p.map(|p| p / 1000.0));
        if dwell_s.is_none() {
            return Ok(None);
        }
        let pos = lengths(state.pos, motion.source_line)?;
        return Ok(Some(Segment {
            start: pos,
            end: pos,
            travel: true,
            speed,
            length: Length::ZERO,
            volume: Volume::ZERO,
            filament: Length::ZERO,
            width: None,
            height: None,
            kind: SegmentKind::Dwell,
            centre: None,
            clockwise: false,
            temperature: state.temperature,
            fan: state.fan,
            flow: None,
            tool: state.tool,
            dwell_s,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        }));
    }

    let start = state.pos;
    let end = [
        apply_axis(state.pos[0], motion.x, motion.state.distance_mode, factor),
        apply_axis(state.pos[1], motion.y, motion.state.distance_mode, factor),
        apply_axis(state.pos[2], motion.z, motion.state.distance_mode, factor),
    ];
    let filament_delta = extrusion_delta(motion, state, factor);
    let deposited = filament_delta.max(0.0) * state.flow;
    let filament = if filament_delta < 0.0 {
        checked_mm(filament_delta, motion.source_line, "extrusion")?
    } else {
        checked_mm(deposited, motion.source_line, "extrusion")?
    };
    // Both factors are finite by here, but their product need not be: a huge `M221` flow ratio
    // scales a modest `E` into a length that overflows once multiplied by the cross-section.
    let volume = filament_area * checked_mm(deposited, motion.source_line, "extrusion")?;
    if !volume.value().is_finite() {
        return Err(GcodeImportError::new(
            motion.source_line,
            format!("deposited volume is not finite ({})", volume.value()),
        ));
    }
    let travel = motion.mode == MotionMode::Rapid || motion.e.is_none();
    let flow = if state.flow == 1.0 {
        None
    } else {
        Some(state.flow)
    };

    let (kind, centre, clockwise, length) = match motion.mode {
        MotionMode::Rapid | MotionMode::Linear => (
            SegmentKind::Line,
            None,
            false,
            checked_mm(point_dist(start, end), motion.source_line, "move length")?,
        ),
        MotionMode::ClockwiseArc | MotionMode::CounterClockwiseArc => {
            let clockwise = motion.mode == MotionMode::ClockwiseArc;
            let arc = arc_geometry(motion, start, end, factor, clockwise)?;
            (
                SegmentKind::Arc,
                Some([
                    checked_mm(arc.centre[0], motion.source_line, "arc centre I")?,
                    checked_mm(arc.centre[1], motion.source_line, "arc centre J")?,
                ]),
                clockwise,
                checked_mm(arc.length, motion.source_line, "arc length")?,
            )
        }
        MotionMode::Dwell => unreachable!("handled above"),
    };

    let start_lengths = lengths(start, motion.source_line)?;
    let end_lengths = lengths(end, motion.source_line)?;
    state.pos = end;

    Ok(Some(Segment {
        start: start_lengths,
        end: end_lengths,
        travel,
        speed,
        length,
        volume,
        filament,
        width: if travel { None } else { width },
        height: if travel { None } else { height },
        kind,
        centre,
        clockwise,
        temperature: state.temperature,
        fan: state.fan,
        flow,
        tool: state.tool,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }))
}

fn apply_axis(
    current: Option<f64>,
    value: Option<f64>,
    mode: DistanceMode,
    factor: f64,
) -> Option<f64> {
    match (value, mode) {
        (Some(value), DistanceMode::Absolute) => Some(value * factor),
        (Some(value), DistanceMode::Relative) => Some(current.unwrap_or(0.0) + value * factor),
        (None, _) => current,
    }
}

fn extrusion_delta(motion: &super::MotionRecord, state: &mut LiftState, factor: f64) -> f64 {
    let Some(value) = motion.e else {
        return 0.0;
    };
    let value = value * factor;
    match motion.state.extrusion_mode {
        ExtrusionMode::Absolute => {
            let delta = value - state.e;
            state.e = value;
            delta
        }
        ExtrusionMode::Relative => {
            state.e += value;
            value
        }
    }
}

fn apply_g92(line: &ParsedGcodeLine, state: &mut LiftState) -> Result<(), GcodeImportError> {
    let factor = unit_factor(line.state_after.units);
    for word in &line.words {
        // `G92` writes the position directly, so an inch conversion that overflows would seed every
        // later move with a non-finite origin. Refuse it here, where the source line is known.
        let scaled = word.value * factor;
        if matches!(word.letter, 'X' | 'Y' | 'Z' | 'E') && !scaled.is_finite() {
            return Err(GcodeImportError::new(
                line.source_line,
                format!(
                    "G92 {}{} is not finite after unit conversion",
                    word.letter, word.value
                ),
            ));
        }
        match word.letter {
            'X' => state.pos[0] = Some(scaled),
            'Y' => state.pos[1] = Some(scaled),
            'Z' => state.pos[2] = Some(scaled),
            'E' => state.e = scaled,
            _ => {}
        }
    }
    Ok(())
}

fn apply_process(command: ProcessCommand, state: &mut LiftState) {
    match command {
        ProcessCommand::NozzleTemperature(temp) => state.temperature = Some(temp),
        ProcessCommand::Fan(speed) => state.fan = Some(speed),
        ProcessCommand::Flow(ratio) => state.flow = ratio,
        ProcessCommand::Tool(index) => state.tool = Some(index),
    }
}

/// Build a [`Length`] from a value the importer *computed*, refusing a non-finite result.
///
/// The word scanner rejects a non-finite word, but the arithmetic between it and the IR can still
/// overflow a finite one: `G20` scales every coordinate by 25.4, and `point_dist` squares the
/// deltas. `Length::mm` only *asserts* finiteness (and only in debug builds), so an overflow here
/// would panic a debug consumer and put `Length(inf)` in the IR of a release one — this is the
/// ingress-side enforcement that assertion documents.
fn checked_mm(value: f64, source_line: usize, what: &str) -> Result<Length, GcodeImportError> {
    Length::try_mm(value).ok_or_else(|| {
        GcodeImportError::new(source_line, format!("{what} is not finite ({value})"))
    })
}

fn lengths(
    pos: [Option<f64>; 3],
    source_line: usize,
) -> Result<[Option<Length>; 3], GcodeImportError> {
    const AXES: [&str; 3] = ["coordinate X", "coordinate Y", "coordinate Z"];
    let mut out = [None; 3];
    for (axis, value) in pos.iter().enumerate() {
        if let Some(value) = value {
            out[axis] = Some(checked_mm(*value, source_line, AXES[axis])?);
        }
    }
    Ok(out)
}

fn point_dist(a: [Option<f64>; 3], b: [Option<f64>; 3]) -> f64 {
    let mut sq = 0.0;
    for axis in 0..3 {
        if let (Some(a), Some(b)) = (a[axis], b[axis]) {
            let delta = b - a;
            sq += delta * delta;
        }
    }
    libm::sqrt(sq)
}

#[derive(Debug, Clone, Copy)]
struct ArcGeometry {
    centre: [f64; 2],
    length: f64,
}

fn arc_geometry(
    motion: &super::MotionRecord,
    start: [Option<f64>; 3],
    end: [Option<f64>; 3],
    factor: f64,
    clockwise: bool,
) -> Result<ArcGeometry, GcodeImportError> {
    let (Some(sx), Some(sy), Some(ex), Some(ey)) = (start[0], start[1], end[0], end[1]) else {
        return Err(GcodeImportError::new(
            motion.source_line,
            "arc import needs defined start and end X/Y",
        ));
    };
    let cx = sx + motion.i.unwrap_or(0.0) * factor;
    let cy = sy + motion.j.unwrap_or(0.0) * factor;
    let radius = libm::hypot(sx - cx, sy - cy);
    if !radius.is_finite() || radius <= 0.0 {
        return Err(GcodeImportError::new(
            motion.source_line,
            "arc import needs a non-zero I/J centre offset",
        ));
    }
    let end_radius = libm::hypot(ex - cx, ey - cy);
    let radius_delta = (radius - end_radius).abs();
    let tolerance = 1e-6 * radius.max(end_radius).max(1.0);
    if !end_radius.is_finite() || end_radius <= 0.0 || radius_delta > tolerance {
        return Err(GcodeImportError::new(
            motion.source_line,
            format!("arc import endpoint radius differs from start radius by {radius_delta:.6} mm"),
        ));
    }
    let start_a = libm::atan2(sy - cy, sx - cx);
    let end_a = libm::atan2(ey - cy, ex - cx);
    let mut swept = Angle(if clockwise {
        start_a - end_a
    } else {
        end_a - start_a
    }) % TAU;
    if swept <= Angle::ZERO {
        swept = swept + Angle(TAU);
    }
    let dz = match (start[2], end[2]) {
        (Some(start), Some(end)) => end - start,
        _ => 0.0,
    };
    let planar = (Length::mm(radius) * swept).value();
    Ok(ArcGeometry {
        centre: [cx, cy],
        length: libm::hypot(planar, dz),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::SegmentKind;
    use crate::{simulate, verify, Contracts};

    #[test]
    fn imports_linear_moves_with_relative_extrusion() {
        let tp = import_gcode(
            "M83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E1.5 F1200\n",
            &GcodeImportParams {
                line_width: Some(0.45),
                layer_height: Some(0.2),
                ..GcodeImportParams::default()
            },
        )
        .unwrap();
        assert_eq!(tp.segments.len(), 2);
        assert!(tp.segments[0].travel);
        assert!(!tp.segments[1].travel);
        assert_eq!(tp.segments[1].start[0], Some(Length::mm(0.0)));
        assert_eq!(tp.segments[1].end[0], Some(Length::mm(10.0)));
        assert_eq!(tp.segments[1].filament, Length::mm(1.5));
        assert_eq!(tp.segments[1].width, Some(Length::mm(0.45)));
        assert_eq!(tp.segments[1].height, Some(Length::mm(0.2)));
    }

    #[test]
    fn imported_process_state_is_attached_to_lifted_segments() {
        let tp = import_gcode(
            "M104 S210\nM106 S128\nM221 S90\nT1\nM83\nG1 X10 E1 F1200\n",
            &GcodeImportParams {
                line_width: Some(0.45),
                layer_height: Some(0.2),
                ..GcodeImportParams::default()
            },
        )
        .unwrap();
        assert_eq!(tp.segments.len(), 1);
        let segment = &tp.segments[0];
        assert_eq!(segment.temperature, Some(210.0));
        assert_eq!(segment.fan, Some(128.0 / 255.0));
        assert_eq!(segment.flow, Some(0.9));
        assert_eq!(segment.tool, Some(1));
        assert!((segment.filament.value() - 0.9).abs() < 1e-12);
    }

    #[test]
    fn imported_nozzle_temperature_satisfies_cold_extrusion_guard() {
        let tp = import_gcode(
            "M104 S210\nM109 S210\nM83\nG1 X0 Y0 Z0.2 F9000\nG1 X10 E1 F1200\n",
            &GcodeImportParams {
                line_width: Some(0.45),
                layer_height: Some(0.2),
                ..GcodeImportParams::default()
            },
        )
        .unwrap();
        let report = verify(
            &tp,
            &Contracts {
                min_temp: Some(180.0),
                ..Contracts::default()
            },
        );
        assert!(
            !report.findings.iter().any(|f| f.rule == "cold-extrusion"),
            "hot imported G-code should not be flagged: {:?}",
            report.findings
        );
    }

    #[test]
    fn e_only_prime_moves_have_duration_and_flow() {
        let tp = import_gcode("M83\nG1 E5 F300\n", &Default::default()).unwrap();
        let metrics = simulate(&tp);
        assert_eq!(metrics.segment_count, 1);
        assert!((metrics.total_time_s.value() - 1.0).abs() < 1e-12);
        assert!(metrics.max_flow_rate.value() > 12.0);

        let report = verify(
            &tp,
            &Contracts {
                max_flow: Some(1.0),
                ..Contracts::default()
            },
        );
        assert!(report.findings.iter().any(|f| f.rule == "max-flow"));
    }

    #[test]
    fn mapped_import_tracks_segment_source_lines() {
        let imported = import_gcode_with_map(
            "; header\nM83\nG1 X0 Y0 Z0.2 F9000\nM104 S210\nG1 X10 E1.5 F1200\n",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(imported.toolpath.segments.len(), 2);
        assert_eq!(imported.segment_source_lines, vec![3, 5]);
        assert_eq!(imported.source_line_for_segment(1), Some(5));
        assert_eq!(imported.segment_for_source_line(5), Some(1));
        assert_eq!(imported.source_line_for_segment(2), None);
    }

    #[test]
    fn mapped_import_tracks_commands_the_verifier_cannot_model() {
        let imported = import_gcode_with_map("G1 X1\nG28 X Y\nM84\n", &Default::default()).unwrap();
        assert_eq!(imported.toolpath.segments.len(), 1);
        assert_eq!(
            imported.unmodeled_commands,
            vec![
                UnmodeledGcode {
                    source_line: 2,
                    command: "G28".to_string(),
                    raw: "G28 X Y".to_string(),
                },
                UnmodeledGcode {
                    source_line: 3,
                    command: "M84".to_string(),
                    raw: "M84".to_string(),
                },
            ]
        );
    }

    #[test]
    fn motion_spans_split_on_non_motion_source_lines() {
        let imported = import_gcode_with_map(
            "; header\nM83\nG1 X0 Y0 Z0.2 F9000\nG1 X1\nM104 S210\nG1 X2\nG1 X3\n",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            imported.motion_spans(),
            vec![
                GcodeMotionSpan {
                    first_source_line: 3,
                    last_source_line: 4,
                    first_segment: 0,
                    segment_count: 2,
                },
                GcodeMotionSpan {
                    first_source_line: 6,
                    last_source_line: 7,
                    first_segment: 2,
                    segment_count: 2,
                },
            ]
        );
    }

    #[test]
    fn source_preserving_emit_splices_only_motion_lines() {
        let imported = import_gcode_with_map(
            "; header\nM83\nG1 X0 Y0 Z0.2 F9000 ; move\nM104 S210\nG1 X10 E1.5 F1200\n",
            &Default::default(),
        )
        .unwrap();
        let lines = imported
            .emit_source_preserving(&imported.toolpath, &EmitParams::default())
            .unwrap();
        assert_eq!(lines[0], "; header");
        assert_eq!(lines[1], "M83");
        assert_eq!(lines[2], "G21");
        assert_eq!(lines[3], "G90");
        assert_eq!(lines[4], "M83");
        assert!(lines[5].starts_with("G0 "));
        assert_eq!(lines[6], "M104 S210");
        assert_eq!(lines[7], "G21");
        assert_eq!(lines[8], "G90");
        assert_eq!(lines[9], "M83");
        assert!(lines[10].starts_with("G1 "));
    }

    #[test]
    fn source_preserving_emit_keeps_feedrate_only_lines() {
        let imported = import_gcode_with_map(
            "M83\nG1 X0 Y0 Z0.2 F9000\nF1200\nG1 X10 E1\n",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(imported.segment_source_lines, vec![2, 4]);

        let lines = imported
            .emit_source_preserving(&imported.toolpath, &EmitParams::default())
            .unwrap();
        assert_eq!(lines[5], "F1200");
        assert_eq!(lines[6], "G21");
        assert_eq!(lines[7], "G90");
        assert_eq!(lines[8], "M83");
        assert!(lines[9].starts_with("G1 "));
    }

    /// The RS-274 frame belongs to the program, not to a span: a caller handing a cnc profile's
    /// `EmitParams` straight to the rewrite path must not get a preamble spliced into every span.
    ///
    /// The same applies to the *modal* prologue the emitter synthesises: `M83`/`M82`/`G92 E`/`M221`
    /// address a filament axis that an RS-274 controller does not have, and an unknown M-code aborts
    /// the program on LinuxCNC/Fanuc. Asserting only on the frame lines let that regression pass
    /// unseen.
    ///
    /// **Scope: emitter-synthesised lines only.** `splice_motion_spans` copies every source line
    /// *outside* a motion span through verbatim — that is this function's contract — so a Marlin
    /// source's own `M104 S210`/`M106`/`M221 S90` still reaches an RS-274 rewrite. The fixture below
    /// carries `M104 S210` and the output retains it; the assertions are worded to let it through on
    /// purpose. Filtering that echo fights the source-preserving contract and is a separate
    /// decision — what is pinned here is only that the *emitter* contributes no filament-axis modal
    /// of its own.
    #[test]
    fn source_preserving_emit_never_splices_the_cnc_frame() {
        let imported = import_gcode_with_map(
            "; header\nG1 X0 Y0 Z0.2 F9000\nM104 S210\nG1 X10 F1200\n",
            &Default::default(),
        )
        .unwrap();
        let framed = EmitParams {
            flavor: crate::emit::FirmwareFlavor::Rs274,
            cnc_frame: Some(crate::emit::CncFrame {
                wcs: Some(54),
                tool: Some(1),
                spindle_rpm: Some(10000.0),
                coolant: Some(true),
            }),
            ..EmitParams::default()
        };
        let lines = imported
            .emit_source_preserving(&imported.toolpath, &framed)
            .unwrap();
        // Non-vacuity: every assertion below is negative, so an empty `lines` would satisfy all of
        // them. Pin that the rewrite actually produced the span's motion first.
        assert!(
            lines.iter().any(|line| line == "G0 F9000 X0 Y0 Z0.2"),
            "the rewritten span lost its motion: {lines:?}"
        );
        for frame_line in ["G21 G17 G90", "G54", "T1 M6", "S10000 M3", "M8"] {
            assert!(
                !lines.iter().any(|line| line == frame_line),
                "frame line {frame_line:?} spliced into a rewritten span: {lines:?}"
            );
        }
        // No filament-axis modal *synthesised by the emitter* reaches an RS-274 program either.
        // Source lines echoed through from outside the motion spans are out of scope (see above):
        // this fixture's own `M104 S210` survives, and these patterns are exact enough not to catch
        // an echoed `M221 S90` — only the `M221 S100` the emitter would write itself.
        for line in &lines {
            assert!(
                !(line == "M83"
                    || line == "M82"
                    || line == "M221 S100"
                    || line.starts_with("G92 E")),
                "extruder modal {line:?} spliced into an RS-274 span: {lines:?}"
            );
        }
        // and no motion line carries an E word
        assert!(
            !lines.iter().any(|line| line
                .split(' ')
                .any(|word| word.starts_with('E') && word.len() > 1)),
            "E word in an RS-274 rewrite: {lines:?}"
        );
        // the same rewrite under absolute E must not reintroduce `M82`/`G92 E`
        let absolute = imported
            .emit_source_preserving(
                &imported.toolpath,
                &EmitParams {
                    relative_e: false,
                    ..framed.clone()
                },
            )
            .unwrap();
        assert!(
            !absolute
                .iter()
                .any(|line| line == "M82" || line.starts_with("G92 E")),
            "absolute-E modal spliced into an RS-274 span: {absolute:?}"
        );
        // and the rewrite still lines up with the unframed params
        assert_eq!(
            lines,
            imported
                .emit_source_preserving(
                    &imported.toolpath,
                    &EmitParams {
                        cnc_frame: None,
                        ..framed.clone()
                    }
                )
                .unwrap()
        );
    }

    #[test]
    fn source_preserving_emit_resets_flow_multiplier() {
        let imported =
            import_gcode_with_map("M221 S90\nM83\nG1 X10 E1 F1200\n", &Default::default()).unwrap();
        let lines = imported
            .emit_source_preserving(&imported.toolpath, &EmitParams::default())
            .unwrap();
        assert_eq!(lines[0], "M221 S90");
        assert!(lines.iter().any(|line| line == "M221 S100"));
        assert!(lines.iter().any(|line| line == "G1 F1200 X10 E0.9"));
    }

    #[test]
    fn source_preserving_absolute_e_realigns_after_g92() {
        let imported = import_gcode_with_map(
            "M82\nG1 X10 E1 F1200\nG92 E0\nG1 X20 E1 F1200\n",
            &Default::default(),
        )
        .unwrap();
        let lines = imported
            .emit_source_preserving(
                &imported.toolpath,
                &EmitParams {
                    relative_e: false,
                    ..EmitParams::default()
                },
            )
            .unwrap();

        assert_eq!(lines[0], "M82");
        assert_eq!(lines[4], "G92 E0");
        assert_eq!(lines[5], "G1 F1200 X10 E1");
        assert_eq!(lines[6], "G92 E0");
        assert_eq!(lines[10], "G92 E1");
        assert_eq!(lines[11], "G1 X20 E2");
    }

    #[test]
    fn span_splice_allows_motion_count_changes() {
        let imported = import_gcode_with_map(
            "; header\nM83\nG1 X0 Y0 Z0.2 F9000\nG1 X1\nM104 S210\nG1 X2\nG1 X3\n",
            &Default::default(),
        )
        .unwrap();
        let lines = imported
            .splice_motion_spans(&[
                vec!["G0 X0 Y0 Z0.2".to_string()],
                vec![
                    "G1 X2".to_string(),
                    "G1 X2.5".to_string(),
                    "G1 X3".to_string(),
                ],
            ])
            .unwrap();
        assert_eq!(
            lines,
            vec![
                "; header",
                "M83",
                "G0 X0 Y0 Z0.2",
                "M104 S210",
                "G1 X2",
                "G1 X2.5",
                "G1 X3",
            ]
        );
    }

    #[test]
    fn imports_absolute_extrusion_and_g92() {
        let tp = import_gcode("G92 E0\nG1 X5 E0.4\nG1 X10 E0.7\n", &Default::default()).unwrap();
        assert_eq!(tp.segments.len(), 2);
        assert_eq!(tp.segments[0].filament, Length::mm(0.4));
        assert!((tp.segments[1].filament.value() - 0.3).abs() < 1e-12);
    }

    #[test]
    fn imports_arcs_with_ij_offsets() {
        let tp = import_gcode(
            "G1 X10 Y0 Z0.2\nG3 X0 Y10 I-10 J0 E1\n",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(tp.segments.len(), 2);
        let arc = &tp.segments[1];
        assert_eq!(arc.kind, SegmentKind::Arc);
        assert_eq!(arc.centre, Some([Length::mm(0.0), Length::mm(0.0)]));
        assert!((arc.length.value() - std::f64::consts::FRAC_PI_2 * 10.0).abs() < 1e-9);
    }

    #[test]
    fn imports_robot_krl_arc_words_and_wait() {
        let tp = import_gcode(
            "PTP V1500 X10 Y20\nLIN X20 Y20\nCIRC X20 Y30 C0 D5\nWAIT 1.5\n",
            &Default::default(),
        )
        .unwrap();
        assert_eq!(tp.segments.len(), 4);

        assert_eq!(tp.segments[0].kind, SegmentKind::Line);
        assert_eq!(tp.segments[0].speed, crate::units::Feedrate(1500.0));
        assert_eq!(tp.segments[0].start, [None, None, None]);
        assert_eq!(tp.segments[0].end[0], Some(Length::mm(10.0)));
        assert_eq!(tp.segments[0].end[1], Some(Length::mm(20.0)));

        assert_eq!(tp.segments[1].kind, SegmentKind::Line);
        assert_eq!(tp.segments[1].end[0], Some(Length::mm(20.0)));

        assert_eq!(tp.segments[2].kind, SegmentKind::Arc);
        assert!(tp.segments[2].clockwise);
        assert_eq!(
            tp.segments[2].centre,
            Some([Length::mm(20.0), Length::mm(25.0)])
        );

        assert_eq!(tp.segments[3].kind, SegmentKind::Dwell);
        assert_eq!(tp.segments[3].dwell_s, Some(1.5));
    }

    #[test]
    fn rejects_arcs_whose_end_is_not_on_the_radius() {
        let err = import_gcode("G1 X10 Y0\nG3 X1 Y1 I-10 J0\n", &Default::default()).unwrap_err();
        assert_eq!(err.source_line, 2);
        assert!(err.message.contains("endpoint radius differs"));
    }

    #[test]
    fn converts_inches_to_mm() {
        let tp = import_gcode("G20\nM83\nG1 X1 E0.1 F60\n", &Default::default()).unwrap();
        assert_eq!(tp.segments[0].end[0], Some(Length::mm(25.4)));
        assert_eq!(tp.segments[0].filament, Length::mm(2.54));
        assert_eq!(tp.segments[0].speed, Feedrate(1524.0));
    }

    #[test]
    fn dwell_imports_as_dwell_segment_without_becoming_modal_motion() {
        let tp = import_gcode("G1 X1\nG4 S2\nX2\n", &Default::default()).unwrap();
        assert_eq!(tp.segments.len(), 3);
        assert_eq!(tp.segments[1].kind, SegmentKind::Dwell);
        assert_eq!(tp.segments[1].dwell_s, Some(2.0));
        assert_eq!(tp.segments[2].kind, SegmentKind::Line);
        assert_eq!(tp.segments[2].end[0], Some(Length::mm(2.0)));
    }

    #[test]
    fn dwell_imports_from_ms_g4_p_word() {
        let tp = import_gcode("G1 X1\nG4 P1500\nX2\n", &Default::default()).unwrap();
        assert_eq!(tp.segments.len(), 3);
        assert_eq!(tp.segments[1].kind, SegmentKind::Dwell);
        assert_eq!(tp.segments[1].dwell_s, Some(1.5));
        assert_eq!(tp.segments[2].kind, SegmentKind::Line);
        assert_eq!(tp.segments[2].end[0], Some(Length::mm(2.0)));
    }
}
