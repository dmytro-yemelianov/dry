//! G-code import primitives.
//!
//! This is the first parse layer for post-slicer review workflows: it preserves source lines and
//! comments, tracks modal state, and exposes motion records without trying to immediately rebuild a
//! full Dry [`crate::Toolpath`].

mod lift;

pub use lift::{
    import_gcode, import_gcode_reader, import_gcode_reader_with_map, import_gcode_with_map,
    import_parsed_gcode, import_parsed_gcode_with_map, GcodeImportError, GcodeImportParams,
    GcodeMotionSpan, ImportedGcode,
};

use std::io::{BufRead, BufReader, Read};

// KRL command tokens (`PTP`/`LIN`/`CIRC`/`WAIT`) are lifted into single-letter pseudo-words so the
// classifier can treat them like G-codes.
//
// `CIRC`'s marker is deliberately **not** a word letter: it used to be `'A'`, which is also the
// rotary word of a 5-axis program, so every `G1 X.. A.. B..` line was classified as a clockwise arc
// and then refused by the importer as an arc with no I/J centre. `@` cannot come from a source line
// — [`parse_words`] only ever pushes ASCII-uppercase letters — so the two channels cannot collide.
// `Q`/`L`/`W` are real RS-274 word letters and carry the same latent collision; leaving them is a
// deliberate scope call, not a claim that they are safe.
const ROBOT_PT: char = 'Q';
const ROBOT_LIN: char = 'L';
const ROBOT_CIRC: char = '@';
const ROBOT_WAIT: char = 'W';

/// Linear-axis distance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMode {
    Absolute,
    Relative,
}

/// Extruder axis distance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrusionMode {
    Absolute,
    Relative,
}

/// Active G-code unit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitMode {
    Millimeters,
    Inches,
}

/// Active motion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionMode {
    Rapid,
    Linear,
    ClockwiseArc,
    CounterClockwiseArc,
    Dwell,
}

/// The parser's modal state after a source line has been applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GcodeModalState {
    pub motion: Option<MotionMode>,
    pub distance_mode: DistanceMode,
    pub extrusion_mode: ExtrusionMode,
    pub units: UnitMode,
    pub feedrate: Option<f64>,
}

impl Default for GcodeModalState {
    fn default() -> Self {
        GcodeModalState {
            motion: None,
            distance_mode: DistanceMode::Absolute,
            extrusion_mode: ExtrusionMode::Absolute,
            units: UnitMode::Millimeters,
            feedrate: None,
        }
    }
}

/// A single `LETTER value` word parsed from a G-code line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GcodeWord {
    pub letter: char,
    pub value: f64,
}

/// A known non-motion state command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateCommand {
    DistanceMode(DistanceMode),
    ExtrusionMode(ExtrusionMode),
    Units(UnitMode),
    SetPosition,
}

/// A known process-state command that affects subsequent motion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProcessCommand {
    NozzleTemperature(f64),
    Fan(f64),
    Flow(f64),
    Tool(u32),
}

/// A motion command with its modal context and source location.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionRecord {
    pub source_line: usize,
    pub mode: MotionMode,
    pub state: GcodeModalState,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub e: Option<f64>,
    pub i: Option<f64>,
    pub j: Option<f64>,
    pub k: Option<f64>,
    /// Rotary words in **degrees**, as written. Which two of them the machine actually has — and how
    /// they map back to a toolframe orientation — is a property of the kinematic model, not of the
    /// program: see [`GcodeImportParams::kinematics`].
    pub a: Option<f64>,
    pub b: Option<f64>,
    pub c: Option<f64>,
    pub f: Option<f64>,
    pub s: Option<f64>,
    pub p: Option<f64>,
}

/// The semantic record identified for one source line.
#[derive(Debug, Clone, PartialEq)]
pub enum GcodeRecord {
    Empty,
    Comment,
    /// Boxed because `MotionRecord` is far larger than every sibling variant, and this enum is
    /// collected into a `Vec` with one entry per source line. Clippy's `large_enum_variant` fires
    /// otherwise, and this repo's discipline is to fix that structurally rather than `#[allow]` it.
    Motion(Box<MotionRecord>),
    State(StateCommand),
    Process(ProcessCommand),
    Other {
        letter: char,
        value: f64,
    },
}

/// A parsed source line. `raw` is preserved without the trailing newline.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedGcodeLine {
    pub source_line: usize,
    pub raw: String,
    pub comment: Option<String>,
    pub words: Vec<GcodeWord>,
    pub state_after: GcodeModalState,
    pub record: GcodeRecord,
}

/// A located parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcodeParseError {
    pub source_line: usize,
    pub message: String,
}

impl GcodeParseError {
    fn new(source_line: usize, message: impl Into<String>) -> Self {
        GcodeParseError {
            source_line,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GcodeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.source_line, self.message)
    }
}

impl std::error::Error for GcodeParseError {}

/// Streaming G-code parser.
pub struct GcodeParser<R: BufRead> {
    reader: R,
    state: GcodeModalState,
    source_line: usize,
    buf: String,
}

impl<R: Read> GcodeParser<BufReader<R>> {
    pub fn from_reader(reader: R) -> Self {
        GcodeParser {
            reader: BufReader::new(reader),
            state: GcodeModalState::default(),
            source_line: 0,
            buf: String::new(),
        }
    }
}

impl<R: BufRead> GcodeParser<R> {
    pub fn new(reader: R) -> Self {
        GcodeParser {
            reader,
            state: GcodeModalState::default(),
            source_line: 0,
            buf: String::new(),
        }
    }
}

impl<R: BufRead> Iterator for GcodeParser<R> {
    type Item = Result<ParsedGcodeLine, GcodeParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.buf.clear();
        match self.reader.read_line(&mut self.buf) {
            Ok(0) => None,
            Ok(_) => {
                self.source_line += 1;
                let raw = self.buf.trim_end_matches(['\r', '\n']).to_string();
                Some(parse_line(self.source_line, &raw, &mut self.state))
            }
            Err(e) => {
                self.source_line += 1;
                Some(Err(GcodeParseError::new(
                    self.source_line,
                    format!("cannot read line: {e}"),
                )))
            }
        }
    }
}

impl<R: BufRead> GcodeParser<R> {
    pub fn with_state(mut self, state: GcodeModalState) -> Self {
        self.state = state;
        self
    }
}

/// Parse all G-code lines from a string.
pub fn parse_gcode_lines(source: &str) -> Result<Vec<ParsedGcodeLine>, GcodeParseError> {
    parse_gcode_lines_with_state(source, GcodeModalState::default())
}

/// Parse all G-code lines from a string with an initial modal state.
pub fn parse_gcode_lines_with_state(
    source: &str,
    state: GcodeModalState,
) -> Result<Vec<ParsedGcodeLine>, GcodeParseError> {
    GcodeParser::new(std::io::Cursor::new(source))
        .with_state(state)
        .collect::<Result<Vec<_>, _>>()
}

fn parse_line(
    source_line: usize,
    raw: &str,
    state: &mut GcodeModalState,
) -> Result<ParsedGcodeLine, GcodeParseError> {
    let (code, comment) = split_comment(raw);
    let words = parse_words(source_line, strip_checksum(code))?;
    let record = if words.is_empty() && comment.is_some() {
        GcodeRecord::Comment
    } else {
        classify_record(source_line, &words, state)
    };
    Ok(ParsedGcodeLine {
        source_line,
        raw: raw.to_string(),
        comment,
        words,
        state_after: *state,
        record,
    })
}

fn split_comment(raw: &str) -> (&str, Option<String>) {
    if let Some((code, comment)) = raw.split_once(';') {
        (code, Some(comment.trim().to_string()))
    } else {
        (raw, None)
    }
}

fn strip_checksum(code: &str) -> &str {
    code.split_once('*').map_or(code, |(prefix, _)| prefix)
}

fn parse_words(source_line: usize, code: &str) -> Result<Vec<GcodeWord>, GcodeParseError> {
    let mut words = Vec::new();
    let mut i = 0;
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    while i < chars.len() {
        let (_, c) = chars[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            if !c.is_ascii_digit() && !matches!(c, '.' | '-' | '+') {
                return Err(GcodeParseError::new(
                    source_line,
                    format!("expected word letter, found {c:?}"),
                ));
            }

            if words
                .last()
                .is_none_or(|word: &GcodeWord| word.letter != ROBOT_WAIT)
            {
                return Err(GcodeParseError::new(
                    source_line,
                    format!("expected word letter, found {c:?}"),
                ));
            }

            let value_start = chars[i].0;
            let mut value_end_i = i;
            while value_end_i < chars.len() {
                let (_, next) = chars[value_end_i];
                if next.is_ascii_whitespace() {
                    break;
                }
                if next.is_ascii_alphabetic() && !is_exponent_marker(&chars, value_end_i) {
                    break;
                }
                value_end_i += 1;
            }
            let value = code[value_start
                ..chars
                    .get(value_end_i)
                    .map(|(idx, _)| *idx)
                    .unwrap_or(code.len())]
                .trim();
            let value = parse_word_value(source_line, ROBOT_WAIT, value)?;
            words.push(GcodeWord { letter: 'S', value });
            i = value_end_i;
            continue;
        }

        let token_start = i;
        i += 1;
        while i < chars.len() && chars[i].1.is_ascii_alphabetic() {
            i += 1;
        }
        let token_end = chars.get(i).map(|(idx, _)| *idx).unwrap_or(code.len());
        let token = &code[chars[token_start].0..token_end];

        let robot_command = match token {
            _ if token.eq_ignore_ascii_case("PTP") => Some(ROBOT_PT),
            _ if token.eq_ignore_ascii_case("LIN") => Some(ROBOT_LIN),
            _ if token.eq_ignore_ascii_case("CIRC") => Some(ROBOT_CIRC),
            _ if token.eq_ignore_ascii_case("WAIT") => Some(ROBOT_WAIT),
            _ => None,
        };
        if let Some(letter) = robot_command {
            words.push(GcodeWord { letter, value: 0.0 });
            continue;
        }

        if token.len() > 1 {
            let letter = c.to_ascii_uppercase();
            let value_start = chars
                .get(token_start + 1)
                .map(|(idx, _)| *idx)
                .unwrap_or(code.len());
            let value = code[value_start..token_end].trim();
            let value = parse_word_value(source_line, letter, value)?;
            words.push(GcodeWord { letter, value });
            continue;
        }

        let letter = c.to_ascii_uppercase();
        let value_start = chars.get(i).map(|(idx, _)| *idx).unwrap_or(code.len());
        while i < chars.len() {
            let (_, next) = chars[i];
            if next.is_ascii_whitespace() {
                break;
            }
            if next.is_ascii_alphabetic() && !is_exponent_marker(&chars, i) {
                break;
            }
            i += 1;
        }
        let value_end = chars.get(i).map(|(idx, _)| *idx).unwrap_or(code.len());
        let value = code[value_start..value_end].trim();
        if value.is_empty() {
            // Some unmodeled firmware commands use parameter letters as flags (`G28 X Y`).
            // Preserve those commands for review without weakening validation of modeled motion
            // such as the invalid `G1 X`.
            let unmodeled_command = words.first().is_some_and(|command: &GcodeWord| {
                !matches!(
                    (command.letter, command.value),
                    (
                        'G',
                        0.0 | 1.0 | 2.0 | 3.0 | 4.0 | 20.0 | 21.0 | 90.0 | 91.0 | 92.0
                    ) | (
                        'M',
                        82.0 | 83.0
                            | 104.0
                            | 106.0
                            | 107.0
                            | 109.0
                            | 140.0
                            | 190.0
                            | 204.0
                            | 205.0
                            | 220.0
                            | 221.0
                    ) | ('T', _)
                )
            });
            if unmodeled_command {
                words.push(GcodeWord { letter, value: 0.0 });
                continue;
            }
            return Err(GcodeParseError::new(
                source_line,
                format!("missing value for {letter} word"),
            ));
        }
        let value = parse_word_value(source_line, letter, value)?;
        words.push(GcodeWord { letter, value });
    }
    Ok(words)
}

/// Parse one word's numeric text, refusing anything that is not a finite number.
///
/// The scanner deliberately admits exponent notation (see [`is_exponent_marker`]), so `1e400`
/// parses to `inf`; `Xnan` parses to NaN through the multi-letter token path. Both then flow
/// straight into the IR as quantities, where `M221 S1e400` became `flow = inf` and, one
/// `0.0 * inf` later, an `E NaN` word in emitted g-code. No machine accepts a non-finite word, so
/// this is the ingress that refuses them — one gate for every letter (X/Y/Z/E/F/S/P/I/J/K).
fn parse_word_value(source_line: usize, letter: char, text: &str) -> Result<f64, GcodeParseError> {
    let value = text.parse::<f64>().map_err(|e| {
        GcodeParseError::new(
            source_line,
            format!("bad {letter} word value {text:?}: {e}"),
        )
    })?;
    if !value.is_finite() {
        return Err(GcodeParseError::new(
            source_line,
            format!("non-finite {letter} word value {text:?}"),
        ));
    }
    Ok(value)
}

fn is_exponent_marker(chars: &[(usize, char)], i: usize) -> bool {
    let c = chars[i].1;
    if c != 'e' {
        return false;
    }
    let Some(prev) = i.checked_sub(1).and_then(|j| chars.get(j)).map(|(_, c)| *c) else {
        return false;
    };
    prev.is_ascii_digit() || prev == '.'
}

fn classify_record(
    source_line: usize,
    words: &[GcodeWord],
    state: &mut GcodeModalState,
) -> GcodeRecord {
    if words.is_empty() {
        return GcodeRecord::Empty;
    }

    let mut explicit_motion = None;
    let mut robot_arc = false;
    let mut state_record = None;
    let mut process_record = None;
    let mut other = None;
    for word in words {
        match (word.letter, rounded_code(word.value)) {
            ('G', Some(0)) => {
                explicit_motion = Some(MotionMode::Rapid);
                state.motion = explicit_motion;
            }
            ('G', Some(1)) => {
                explicit_motion = Some(MotionMode::Linear);
                state.motion = explicit_motion;
            }
            ('G', Some(2)) => {
                explicit_motion = Some(MotionMode::ClockwiseArc);
                state.motion = explicit_motion;
            }
            ('G', Some(3)) => {
                explicit_motion = Some(MotionMode::CounterClockwiseArc);
                state.motion = explicit_motion;
            }
            ('G', Some(4)) => {
                explicit_motion = Some(MotionMode::Dwell);
            }
            ('G', Some(20)) => {
                state.units = UnitMode::Inches;
                state_record = Some(StateCommand::Units(UnitMode::Inches));
            }
            ('G', Some(21)) => {
                state.units = UnitMode::Millimeters;
                state_record = Some(StateCommand::Units(UnitMode::Millimeters));
            }
            ('G', Some(90)) => {
                state.distance_mode = DistanceMode::Absolute;
                state_record = Some(StateCommand::DistanceMode(DistanceMode::Absolute));
            }
            ('G', Some(91)) => {
                state.distance_mode = DistanceMode::Relative;
                state_record = Some(StateCommand::DistanceMode(DistanceMode::Relative));
            }
            ('G', Some(92)) => {
                state_record = Some(StateCommand::SetPosition);
            }
            (ROBOT_PT, _) => {
                explicit_motion = Some(MotionMode::Linear);
                state.motion = explicit_motion;
            }
            (ROBOT_LIN, _) => {
                explicit_motion = Some(MotionMode::Linear);
                state.motion = explicit_motion;
            }
            (ROBOT_CIRC, _) => {
                explicit_motion = Some(MotionMode::ClockwiseArc);
                state.motion = explicit_motion;
                robot_arc = true;
            }
            (ROBOT_WAIT, _) => {
                explicit_motion = Some(MotionMode::Dwell);
            }
            ('M', Some(82)) => {
                state.extrusion_mode = ExtrusionMode::Absolute;
                state_record = Some(StateCommand::ExtrusionMode(ExtrusionMode::Absolute));
            }
            ('M', Some(83)) => {
                state.extrusion_mode = ExtrusionMode::Relative;
                state_record = Some(StateCommand::ExtrusionMode(ExtrusionMode::Relative));
            }
            ('M', Some(104 | 109)) => {
                if let Some(temp) = word_value(words, 'S').or_else(|| word_value(words, 'R')) {
                    process_record = Some(ProcessCommand::NozzleTemperature(temp));
                } else {
                    other = Some(('M', word.value));
                }
            }
            ('M', Some(106)) => {
                let speed = word_value(words, 'S').map_or(1.0, fan_ratio_from_s);
                process_record = Some(ProcessCommand::Fan(speed));
            }
            ('M', Some(107)) => {
                process_record = Some(ProcessCommand::Fan(0.0));
            }
            ('M', Some(221)) => {
                if let Some(percent) = word_value(words, 'S') {
                    process_record = Some(ProcessCommand::Flow(flow_ratio_from_percent(percent)));
                } else {
                    other = Some(('M', word.value));
                }
            }
            ('T', Some(tool)) if tool >= 0 => {
                process_record = Some(ProcessCommand::Tool(tool as u32));
            }
            ('N', _) => {}
            (letter @ ('G' | 'M'), _) => {
                other = Some((letter, word.value));
            }
            _ => {}
        }
    }

    let f = word_value(words, 'F').or_else(|| word_value(words, 'V'));
    if let Some(feedrate) = f {
        state.feedrate = Some(feedrate);
    }

    // A rotary word is motion: on a 5-axis machine `A30` after a `G1` re-points the tool without
    // moving a linear axis. Leaving `A`/`B`/`C` out here would not merely drop that line — the
    // importer carries the rotary words modally, so a dropped one would leave every *later* segment
    // claiming the previous orientation.
    let has_axis_motion_words = ['X', 'Y', 'Z', 'E', 'I', 'J', 'K', 'A', 'B', 'C']
        .into_iter()
        .any(|letter| word_value(words, letter).is_some());
    let has_dwell_words = explicit_motion == Some(MotionMode::Dwell)
        && ['S', 'P']
            .into_iter()
            .any(|letter| word_value(words, letter).is_some());
    let has_motion_words = has_axis_motion_words || has_dwell_words;
    // An explicit but unsupported G/M command owns the line. Do not reinterpret its axis-like
    // arguments through the previous modal motion (for example, `G28 X Y` after `G1` is homing,
    // not a linear move).
    let modal_motion = if state_record.is_none() && other.is_none() && has_motion_words {
        state.motion
    } else {
        None
    };
    if let Some(mode) = explicit_motion.or(modal_motion) {
        let i = if robot_arc {
            word_value(words, 'C').or_else(|| word_value(words, 'I'))
        } else {
            word_value(words, 'I')
        };
        let j = if robot_arc {
            word_value(words, 'D').or_else(|| word_value(words, 'J'))
        } else {
            word_value(words, 'J')
        };
        // On a KRL `CIRC` line `C`/`D` are the arc centre offsets, already consumed as `i`/`j` above;
        // reading `C` as a rotary word there would spend the same word twice.
        let (a, b, c) = if robot_arc {
            (None, None, None)
        } else {
            (
                word_value(words, 'A'),
                word_value(words, 'B'),
                word_value(words, 'C'),
            )
        };

        return GcodeRecord::Motion(Box::new(MotionRecord {
            source_line,
            mode,
            state: *state,
            x: word_value(words, 'X'),
            y: word_value(words, 'Y'),
            z: word_value(words, 'Z'),
            e: word_value(words, 'E'),
            i,
            j,
            k: word_value(words, 'K'),
            a,
            b,
            c,
            f,
            s: word_value(words, 'S'),
            p: word_value(words, 'P'),
        }));
    }

    if let Some(command) = state_record {
        GcodeRecord::State(command)
    } else if let Some(command) = process_record {
        GcodeRecord::Process(command)
    } else if let Some((letter, value)) = other {
        GcodeRecord::Other { letter, value }
    } else {
        GcodeRecord::Comment
    }
}

// Both ratio helpers are total: `parse_word_value` already refused a non-finite word, so the
// fallback arms are unreachable from the parser — they exist so neither helper can *return* a
// non-finite ratio if some other caller reaches them. `clamp` passes NaN through unchanged and
// `max(0.0)` returns NaN for NaN, so neither guard is implied by the arithmetic.
fn fan_ratio_from_s(value: f64) -> f64 {
    if !value.is_finite() {
        return 1.0; // the `M106` no-S default: full fan
    }
    if value <= 1.0 {
        value.clamp(0.0, 1.0)
    } else {
        (value / 255.0).clamp(0.0, 1.0)
    }
}

fn flow_ratio_from_percent(value: f64) -> f64 {
    if !value.is_finite() {
        return 1.0; // 100%: the neutral multiplier
    }
    (value / 100.0).max(0.0)
}

fn rounded_code(value: f64) -> Option<i32> {
    let rounded = value.round();
    if (value - rounded).abs() <= 1e-9 {
        Some(rounded as i32)
    } else {
        None
    }
}

fn word_value(words: &[GcodeWord], letter: char) -> Option<f64> {
    words
        .iter()
        .rev()
        .find(|word| word.letter == letter)
        .map(|word| word.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_motion_and_comments() {
        let lines = parse_gcode_lines("G1 X10 Y20 E0.4 F1200 ; outer wall\n").unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].comment.as_deref(), Some("outer wall"));
        let GcodeRecord::Motion(m) = &lines[0].record else {
            panic!("expected motion");
        };
        assert_eq!(m.source_line, 1);
        assert_eq!(m.mode, MotionMode::Linear);
        assert_eq!(m.x, Some(10.0));
        assert_eq!(m.y, Some(20.0));
        assert_eq!(m.e, Some(0.4));
        assert_eq!(m.f, Some(1200.0));
        assert_eq!(m.state.feedrate, Some(1200.0));
    }

    #[test]
    fn supports_modal_motion_reuse_without_spaces() {
        let lines = parse_gcode_lines("G1 X0 Y0 F900\nX5.5Y-2.25E1e-3\n").unwrap();
        let GcodeRecord::Motion(m) = &lines[1].record else {
            panic!("expected modal motion");
        };
        assert_eq!(m.mode, MotionMode::Linear);
        assert_eq!(m.x, Some(5.5));
        assert_eq!(m.y, Some(-2.25));
        assert_eq!(m.e, Some(1e-3));
        assert_eq!(m.state.feedrate, Some(900.0));
    }

    #[test]
    fn tracks_units_and_distance_modes() {
        let lines = parse_gcode_lines("G20\nG91\nM83\nG1 X1 E0.2\n").unwrap();
        assert_eq!(
            lines[0].record,
            GcodeRecord::State(StateCommand::Units(UnitMode::Inches))
        );
        let GcodeRecord::Motion(m) = &lines[3].record else {
            panic!("expected motion");
        };
        assert_eq!(m.state.units, UnitMode::Inches);
        assert_eq!(m.state.distance_mode, DistanceMode::Relative);
        assert_eq!(m.state.extrusion_mode, ExtrusionMode::Relative);
    }

    #[test]
    fn preserves_unknown_commands_and_comments() {
        let lines = parse_gcode_lines("; header\nM104 S210\nN42 G1 X1*99\nM104 S215\n").unwrap();
        assert_eq!(lines[0].record, GcodeRecord::Comment);
        assert_eq!(
            lines[1].record,
            GcodeRecord::Process(ProcessCommand::NozzleTemperature(210.0))
        );
        let GcodeRecord::Motion(m) = &lines[2].record else {
            panic!("expected motion");
        };
        assert_eq!(m.x, Some(1.0));
        assert_eq!(lines[2].raw, "N42 G1 X1*99");
        assert_eq!(
            lines[3].record,
            GcodeRecord::Process(ProcessCommand::NozzleTemperature(215.0))
        );
    }

    #[test]
    fn unsupported_command_with_axes_is_not_reinterpreted_as_modal_motion() {
        let lines = parse_gcode_lines("G1 X10 Y10\nG28 X Y\n").unwrap();
        assert!(matches!(lines[0].record, GcodeRecord::Motion(_)));
        assert_eq!(
            lines[1].record,
            GcodeRecord::Other {
                letter: 'G',
                value: 28.0
            }
        );
    }

    #[test]
    fn parses_common_process_state_commands() {
        let lines = parse_gcode_lines("M109 R205\nM106 S128\nM107\nM221 S95\nT2\n").unwrap();
        assert_eq!(
            lines[0].record,
            GcodeRecord::Process(ProcessCommand::NozzleTemperature(205.0))
        );
        assert_eq!(
            lines[1].record,
            GcodeRecord::Process(ProcessCommand::Fan(128.0 / 255.0))
        );
        assert_eq!(
            lines[2].record,
            GcodeRecord::Process(ProcessCommand::Fan(0.0))
        );
        assert_eq!(
            lines[3].record,
            GcodeRecord::Process(ProcessCommand::Flow(0.95))
        );
        assert_eq!(
            lines[4].record,
            GcodeRecord::Process(ProcessCommand::Tool(2))
        );
    }

    #[test]
    fn keeps_g92_as_state_after_modal_motion() {
        let lines = parse_gcode_lines("G1 X1 E0.2\nG92 E0\nX2 E0.3\n").unwrap();
        assert_eq!(
            lines[1].record,
            GcodeRecord::State(StateCommand::SetPosition)
        );
        let GcodeRecord::Motion(m) = &lines[2].record else {
            panic!("expected modal motion");
        };
        assert_eq!(m.mode, MotionMode::Linear);
        assert_eq!(m.x, Some(2.0));
        assert_eq!(m.e, Some(0.3));
    }

    #[test]
    fn reports_bad_words_with_source_line() {
        let err = parse_gcode_lines("G1 Xnope\n").unwrap_err();
        assert_eq!(err.source_line, 1);
        assert!(err.message.contains("X word"));
    }

    #[test]
    fn parses_robot_motion_commands_with_robot_words_and_wait() {
        let lines = parse_gcode_lines(
            "PTP V1500 X10 Y20\nLIN X20 Y20\nCIRC V1200 X30 Y30 C-5 D2.5\nWAIT 1.5\n",
        )
        .unwrap();

        let GcodeRecord::Motion(ptp) = &lines[0].record else {
            panic!("expected linear robot motion");
        };
        assert_eq!(ptp.mode, MotionMode::Linear);
        assert_eq!(ptp.x, Some(10.0));
        assert_eq!(ptp.y, Some(20.0));
        assert_eq!(ptp.f, Some(1500.0));
        assert_eq!(ptp.state.feedrate, Some(1500.0));

        let GcodeRecord::Motion(linear) = &lines[1].record else {
            panic!("expected linear robot motion");
        };
        assert_eq!(linear.mode, MotionMode::Linear);
        assert_eq!(linear.x, Some(20.0));

        let GcodeRecord::Motion(circ) = &lines[2].record else {
            panic!("expected circular robot motion");
        };
        assert_eq!(circ.mode, MotionMode::ClockwiseArc);
        assert_eq!(circ.i, Some(-5.0));
        assert_eq!(circ.j, Some(2.5));

        let GcodeRecord::Motion(wait) = &lines[3].record else {
            panic!("expected robot dwell");
        };
        assert_eq!(wait.mode, MotionMode::Dwell);
        assert_eq!(wait.s, Some(1.5));
    }
}
