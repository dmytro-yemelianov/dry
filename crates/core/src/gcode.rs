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
    pub f: Option<f64>,
    pub s: Option<f64>,
    pub p: Option<f64>,
}

/// The semantic record identified for one source line.
#[derive(Debug, Clone, PartialEq)]
pub enum GcodeRecord {
    Empty,
    Comment,
    Motion(MotionRecord),
    State(StateCommand),
    Other { letter: char, value: f64 },
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

/// Parse all G-code lines from a string.
pub fn parse_gcode_lines(source: &str) -> Result<Vec<ParsedGcodeLine>, GcodeParseError> {
    GcodeParser::new(std::io::Cursor::new(source)).collect::<Result<Vec<_>, _>>()
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
            return Err(GcodeParseError::new(
                source_line,
                format!("expected word letter, found {c:?}"),
            ));
        }
        let letter = c.to_ascii_uppercase();
        i += 1;
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
            return Err(GcodeParseError::new(
                source_line,
                format!("missing value for {letter} word"),
            ));
        }
        let value = value.parse::<f64>().map_err(|e| {
            GcodeParseError::new(
                source_line,
                format!("bad {letter} word value {value:?}: {e}"),
            )
        })?;
        words.push(GcodeWord { letter, value });
    }
    Ok(words)
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
    let mut state_record = None;
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
            ('M', Some(82)) => {
                state.extrusion_mode = ExtrusionMode::Absolute;
                state_record = Some(StateCommand::ExtrusionMode(ExtrusionMode::Absolute));
            }
            ('M', Some(83)) => {
                state.extrusion_mode = ExtrusionMode::Relative;
                state_record = Some(StateCommand::ExtrusionMode(ExtrusionMode::Relative));
            }
            ('N', _) => {}
            (letter @ ('G' | 'M'), _) => {
                other = Some((letter, word.value));
            }
            _ => {}
        }
    }

    let f = word_value(words, 'F');
    if let Some(feedrate) = f {
        state.feedrate = Some(feedrate);
    }

    let has_axis_motion_words = ['X', 'Y', 'Z', 'E', 'I', 'J', 'K', 'F']
        .into_iter()
        .any(|letter| word_value(words, letter).is_some());
    let has_dwell_words = explicit_motion == Some(MotionMode::Dwell)
        && ['S', 'P']
            .into_iter()
            .any(|letter| word_value(words, letter).is_some());
    let has_motion_words = has_axis_motion_words || has_dwell_words;
    let modal_motion = if state_record.is_none() && has_motion_words {
        state.motion
    } else {
        None
    };
    if let Some(mode) = explicit_motion.or(modal_motion) {
        return GcodeRecord::Motion(MotionRecord {
            source_line,
            mode,
            state: *state,
            x: word_value(words, 'X'),
            y: word_value(words, 'Y'),
            z: word_value(words, 'Z'),
            e: word_value(words, 'E'),
            i: word_value(words, 'I'),
            j: word_value(words, 'J'),
            k: word_value(words, 'K'),
            f,
            s: word_value(words, 'S'),
            p: word_value(words, 'P'),
        });
    }

    if let Some(command) = state_record {
        GcodeRecord::State(command)
    } else if let Some((letter, value)) = other {
        GcodeRecord::Other { letter, value }
    } else {
        GcodeRecord::Comment
    }
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
            GcodeRecord::Other {
                letter: 'M',
                value: 104.0
            }
        );
        let GcodeRecord::Motion(m) = &lines[2].record else {
            panic!("expected motion");
        };
        assert_eq!(m.x, Some(1.0));
        assert_eq!(lines[2].raw, "N42 G1 X1*99");
        assert_eq!(
            lines[3].record,
            GcodeRecord::Other {
                letter: 'M',
                value: 104.0
            }
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
}
