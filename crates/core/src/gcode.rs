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
// **Nothing emits this dialect any more.** It is the g-code-shaped KRL Dry wrote before #181; the
// KRL renderer now produces a real `DEF`/`END` module of `{E6POS: ...}` aggregates, which this
// scanner cannot read and does not try to (`DEF` scans as a `D` word and the parse fails loudly).
// The lifting is kept because it still reads programs written in the old dialect; it is an import
// path with no matching export, not a round-trip.
//
// `CIRC`'s marker is deliberately **not** a word letter: it used to be `'A'`, which is also the
// rotary word of a 5-axis program, so every `G1 X.. A.. B..` line was classified as a clockwise arc
// and then refused by the importer as an arc with no I/J centre. `@` cannot come from a source line
// — [`parse_words`] only ever pushes ASCII-uppercase letters — so the two channels cannot collide.
// `Q`/`L`/`W` are real RS-274 word letters and carry the same latent collision. It is *narrowed*, not
// closed: [`classify_record`] promotes a KRL marker to a command only on a line that states no
// G/M/T command of its own, so `M1006 ... L100 ...` is the macro it is, but a bare `L100` line with
// nothing else on it still reads as `LIN`.
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
    Power(f64),
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
    let code = strip_checksum(code);
    // Classification comes before tokenization. Whether Dry models a line is a property of the
    // line's *command*, not of whether its parameters happen to scan as `LETTER value` words: a
    // vendor macro (`M1002 set_gcode_claim_speed_level : 5`) and a firmware capability check
    // (`M862.3 P "MK4"`) are ordinary lines of the file they came from, and refusing to tokenize
    // them used to abort the whole import — before the "preserved byte-for-byte and reported as
    // `unmodeled-gcode`" contract could ever apply to them. So an unmodeled line's parameters are
    // scanned leniently: what scans becomes words, and what does not leaves the command word alone
    // to carry the line into [`GcodeRecord::Other`], with `raw` preserving the text verbatim.
    //
    // [`line_dialect`] is evaluated only when the scan fails, which is equivalent to running it
    // first: on a line that scans, the strict word list is what a lenient scan would have produced
    // anyway, and no dialect widens what the scanner accepts.
    let words = match parse_words(source_line, code) {
        Ok(words) => words,
        Err(failure) => match recovered_command(line_dialect(code), &failure) {
            Some(command) => vec![command],
            None => return Err(failure.into_error()),
        },
    };
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

/// Why the word scanner stopped, so [`parse_line`] can tell "this is not the dialect Dry models"
/// from "this is the dialect Dry models and the number is unusable".
///
/// The distinction is the whole boundary: the first is a *classification* statement about a line and
/// may be answered by preserving the line verbatim; the second is a *value-domain* refusal and is
/// never recovered from, on any command (see [`parse_word_value`]).
#[derive(Debug)]
enum ScanFailure {
    /// A word letter with no value — `M84 E`, `M221 S`. Firmware shorthand for a flag, and never a
    /// number: it says the line is not the modeled numeric form of its command.
    FlagWord(GcodeParseError),
    /// Text that is not a `LETTER value` word at all: a bareword, a quoted string, a `:`, base64,
    /// a version string, a numeric literal that is not one.
    NotAWord(GcodeParseError),
    /// A word scanned, and its value is outside the domain the IR admits. The H1.2 ingress gate.
    BadValue(GcodeParseError),
}

impl ScanFailure {
    fn into_error(self) -> GcodeParseError {
        match self {
            ScanFailure::FlagWord(error)
            | ScanFailure::NotAWord(error)
            | ScanFailure::BadValue(error) => error,
        }
    }
}

/// How strictly a source line's words must scan, decided from its leading command.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LineDialect {
    /// The motion dialect: `G0`–`G4`, the KRL pseudo-commands, and a bare modal continuation
    /// (`X5.5Y-2.25E1e-3`). Every word here is lifted into the IR, so nothing about one may be
    /// guessed — `G1 X` and `G1 Xnope` stay hard errors.
    ///
    /// A line whose leading token is not a recognizable command falls here too: "unrecognizable"
    /// must never be the way a line buys leniency.
    Motion,
    /// A non-motion command whose word *values* the importer reads: `G92 E0`, `M104 S210`, `T0`.
    Modeled(GcodeWord),
    /// A command Dry does not model. It survives as text and is reported through `unmodeled-gcode`;
    /// its parameters are the vendor's dialect, not Dry's, and are not policed as G-code words.
    Unmodeled(GcodeWord),
}

/// The command word to keep when a line's scan is allowed to degrade, or `None` to fail the import.
fn recovered_command(dialect: LineDialect, failure: &ScanFailure) -> Option<GcodeWord> {
    match (dialect, failure) {
        // H1.2 is unconditional: a word that scanned to a non-finite number is refused whatever
        // command carries it. Recovering here would reopen the ingress the check exists to close.
        (_, ScanFailure::BadValue(_)) | (LineDialect::Motion, _) => None,
        // `M221 S` (Bambu pushes the soft-endstop status with a flag) is not `M221 S100`. The flag
        // says so syntactically, so the line degrades to its command rather than aborting the file
        // — but `M221 Snope` is still the modeled form with an unusable number, and still fails.
        (LineDialect::Modeled(command), ScanFailure::FlagWord(_)) => Some(command),
        (LineDialect::Modeled(_), ScanFailure::NotAWord(_)) => None,
        (LineDialect::Unmodeled(command), _) => Some(command),
    }
}

fn line_dialect(code: &str) -> LineDialect {
    let Some(command) = leading_command(code) else {
        return LineDialect::Motion;
    };
    match (command.letter, rounded_code(command.value)) {
        ('G', Some(0..=4)) => LineDialect::Motion,
        // The non-motion commands `classify_record` reads words from, plus the mode commands whose
        // effect the importer carries modally. Keep in step with `classify_record`.
        ('G', Some(20 | 21 | 90 | 91 | 92))
        | ('M', Some(82 | 83 | 104 | 106 | 107 | 109 | 221))
        | ('T', Some(_)) => LineDialect::Modeled(command),
        ('G' | 'M', _) => LineDialect::Unmodeled(command),
        // Not a command: a modal continuation, which is motion.
        _ => LineDialect::Motion,
    }
}

/// Scan the line's leading command word without tokenizing the rest of the line.
///
/// `None` means the line does not begin with a single-letter `LETTER value` word — a modal
/// continuation, a KRL keyword (`PTP`), or something malformed. All of those keep the strict
/// dialect.
fn leading_command(code: &str) -> Option<GcodeWord> {
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    let mut i = 0;
    let first = scan_leading_word(code, &chars, &mut i)?;
    // `N42 G1 X1` — the line number is not the command.
    if first.letter == 'N' {
        return scan_leading_word(code, &chars, &mut i);
    }
    Some(first)
}

fn scan_leading_word(code: &str, chars: &[(usize, char)], i: &mut usize) -> Option<GcodeWord> {
    while chars.get(*i).is_some_and(|(_, c)| c.is_ascii_whitespace()) {
        *i += 1;
    }
    let (_, letter) = *chars.get(*i)?;
    if !letter.is_ascii_alphabetic() {
        return None;
    }
    // A multi-letter token is a KRL keyword or a malformed word, never a G/M/T command.
    if chars
        .get(*i + 1)
        .is_some_and(|(_, c)| c.is_ascii_alphabetic())
    {
        return None;
    }
    let value_start = chars.get(*i + 1).map(|(idx, _)| *idx)?;
    *i += 1;
    // The same value extent `parse_words` uses, so the two cannot disagree about where a word ends.
    while *i < chars.len() {
        let (_, next) = chars[*i];
        if next.is_ascii_whitespace() {
            break;
        }
        if next.is_ascii_alphabetic() && !is_exponent_marker(chars, *i) {
            break;
        }
        *i += 1;
    }
    let value_end = chars.get(*i).map(|(idx, _)| *idx).unwrap_or(code.len());
    let value = code[value_start..value_end].trim().parse::<f64>().ok()?;
    value.is_finite().then_some(GcodeWord {
        letter: letter.to_ascii_uppercase(),
        value,
    })
}

fn parse_words(source_line: usize, code: &str) -> Result<Vec<GcodeWord>, ScanFailure> {
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
                return Err(ScanFailure::NotAWord(GcodeParseError::new(
                    source_line,
                    format!("expected word letter, found {c:?}"),
                )));
            }

            if words
                .last()
                .is_none_or(|word: &GcodeWord| word.letter != ROBOT_WAIT)
            {
                return Err(ScanFailure::NotAWord(GcodeParseError::new(
                    source_line,
                    format!("expected word letter, found {c:?}"),
                )));
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
            // Firmware commands use parameter letters as flags (`G28 X Y`, `M84 E`, `M221 S`).
            // Whether that is legal is [`line_dialect`]'s call, not the scanner's: the caller keeps
            // the flagged line as its command when the command is one Dry does not read words from,
            // and fails the import when it is modeled motion such as the invalid `G1 X`.
            return Err(ScanFailure::FlagWord(GcodeParseError::new(
                source_line,
                format!("missing value for {letter} word"),
            )));
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
///
/// The two failures are reported apart because only one of them is recoverable: text that is not a
/// number says nothing about any command, while a number outside the IR's domain is refused on
/// every command, modeled or not ([`recovered_command`]).
fn parse_word_value(source_line: usize, letter: char, text: &str) -> Result<f64, ScanFailure> {
    let value = text.parse::<f64>().map_err(|e| {
        ScanFailure::NotAWord(GcodeParseError::new(
            source_line,
            format!("bad {letter} word value {text:?}: {e}"),
        ))
    })?;
    if !value.is_finite() {
        return Err(ScanFailure::BadValue(GcodeParseError::new(
            source_line,
            format!("non-finite {letter} word value {text:?}"),
        )));
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
    let mut krl_motion = None;
    let mut krl_arc = false;
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
            // The KRL markers are collected apart from `explicit_motion` and only promoted below,
            // once the whole line is known: three of the four are also real RS-274 word letters.
            (ROBOT_PT | ROBOT_LIN, _) => krl_motion = Some(MotionMode::Linear),
            (ROBOT_CIRC, _) => {
                krl_motion = Some(MotionMode::ClockwiseArc);
                krl_arc = true;
            }
            (ROBOT_WAIT, _) => krl_motion = Some(MotionMode::Dwell),
            ('M', Some(3 | 4)) => {
                let speed = word_value(words, 'S').unwrap_or(1.0);
                process_record = Some(ProcessCommand::Power(speed));
            }
            ('M', Some(5)) => {
                process_record = Some(ProcessCommand::Power(0.0));
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
                    other.get_or_insert(('M', word.value));
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
                    other.get_or_insert(('M', word.value));
                }
            }
            ('T', Some(tool)) if tool >= 0 => {
                process_record = Some(ProcessCommand::Tool(tool as u32));
            }
            ('N', _) => {}
            // The *first* G/M command names the line, so `unmodeled-gcode` reports the command a
            // reader has to go and look up. Bambu's `M1006 A0 B10 L100 C37 D10 M60 E37 F10 N60`
            // carries a second `M` word as a macro argument, and naming the line "M60" would send
            // that reader to a spindle-coolant code that is not there.
            (letter @ ('G' | 'M'), _) => {
                other.get_or_insert((letter, word.value));
            }
            _ => {}
        }
    }

    // A KRL marker is a *command* only on a line that states no G/M/T command of its own. `Q`, `L`
    // and `W` are real RS-274 word letters as well as the `PTP`/`LIN`/`WAIT` markers (see
    // [`ROBOT_PT`]), so a firmware macro that happens to use one is not a KRL move: Bambu's
    // `M1006 A0 B10 L100 C37 D10 M60 E37 F10 N60` — a macro that plays a note — scanned as a `LIN`
    // with a rotary pose and 37 mm of extrusion, which then refused the whole file for lack of a
    // kinematic model to read `A`/`B`/`C` with. This is the `G28 X Y` rule ("the line's own command
    // owns it") applied to the KRL channel; it narrows the collision rather than closing it, since a
    // bare `L100` line still has nothing else to go on.
    let krl_motion = if state_record.is_none() && other.is_none() && process_record.is_none() {
        krl_motion
    } else {
        None
    };
    let robot_arc = krl_arc && krl_motion.is_some();
    if let Some(mode) = krl_motion {
        explicit_motion = Some(mode);
        // `WAIT` is not a motion mode to inherit, exactly as `G4` is not.
        if mode != MotionMode::Dwell {
            state.motion = explicit_motion;
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

    /// A command Dry does not model owns its own parameter syntax. These four constructs are all
    /// present in stock OrcaSlicer output for the two most common printer ecosystems, and each one
    /// used to abort the whole import at the tokenizer — before the "preserved and reported as
    /// `unmodeled-gcode`" contract could apply to the line carrying it.
    #[test]
    fn unmodeled_commands_survive_parameters_that_are_not_gcode_words() {
        for (source, expected) in [
            // Bambu macro pseudo-command: `M1002 <bareword> : <number>`.
            ("M1002 set_gcode_claim_speed_level : 5\n", 1002.0),
            // Prusa firmware checks: a quoted string, with and without a space before the quote.
            ("M862.3 P \"MK4\"\n", 862.3),
            ("M862.6 P\"Input shaper\"\n", 862.6),
            // A firmware version string, and an AMS payload in base64.
            ("M115 U5.0.0-RC+11963\n", 115.0),
            ("M624 AQAAAAAAAAA=\n", 624.0),
        ] {
            let lines = parse_gcode_lines(source)
                .unwrap_or_else(|e| panic!("{source:?} must import, got {e}"));
            assert_eq!(
                lines[0].record,
                GcodeRecord::Other {
                    letter: 'M',
                    value: expected
                },
                "{source:?}"
            );
            assert_eq!(lines[0].raw, source.trim_end(), "raw must be verbatim");
        }
    }

    /// A flag word is not a number, so it says the line is not the modeled numeric form of its
    /// command — including on a command Dry does read words from. Bambu pushes and pops the
    /// soft-endstop status with `M221 S`/`M221 R`, which is not the `M221 S100` flow multiplier.
    #[test]
    fn a_flag_word_makes_a_modeled_command_unmodeled_rather_than_failing() {
        let lines = parse_gcode_lines("M221 S100\nM221 S\nM221 R\nM84 E\n").unwrap();
        assert_eq!(
            lines[0].record,
            GcodeRecord::Process(ProcessCommand::Flow(1.0))
        );
        for line in &lines[1..3] {
            assert_eq!(
                line.record,
                GcodeRecord::Other {
                    letter: 'M',
                    value: 221.0
                }
            );
        }
        assert_eq!(
            lines[3].record,
            GcodeRecord::Other {
                letter: 'M',
                value: 84.0
            }
        );
    }

    /// The motion dialect keeps failing loudly: a modeled move's words all reach the IR, so a
    /// missing or unreadable one is corruption, not a vendor extension. A bare modal continuation
    /// counts as motion — an unrecognizable line must never buy leniency by being unrecognizable.
    #[test]
    fn motion_words_are_still_refused_when_they_are_not_numbers() {
        for source in [
            "G1 X\n",
            "G1 Xnope\n",
            "G1 X10 Y\n",
            "G0 Z\"up\"\n",
            "G2 X1 Y1 I\n",
            "G4 P\n",
            "G1 X0 Y0 F900\nX5.5 Y\n",
            "G92 Enope\n",
            // A modeled command with an unreadable *number* is still the modeled form.
            "M221 Snope\n",
            "M104 S\"hot\"\n",
        ] {
            assert!(
                parse_gcode_lines(source).is_err(),
                "{source:?} must stay a hard parse error"
            );
        }
    }

    /// H1.2's word-level finiteness gate is a value-domain refusal, not a statement about which
    /// dialect a line belongs to, so no command recovers from a word that scans to a non-finite
    /// number. (A word *after* the point where an unmodeled line stops scanning is neither checked
    /// nor lifted — the line becomes its command alone, so no value from it reaches the IR.)
    #[test]
    fn a_non_finite_word_is_refused_on_modeled_and_unmodeled_commands_alike() {
        for source in [
            "M221 S1e400\n",
            "G1 X1e400\n",
            "M1002 S1e400\n",
            "M900 K1e400\n",
        ] {
            let error = parse_gcode_lines(source)
                .expect_err(&format!("{source:?} must be refused"))
                .message;
            assert!(
                error.contains("non-finite"),
                "expected a non-finite refusal for {source:?}, got {error}"
            );
        }
    }

    /// The command that owns the line is the leading one, found before tokenizing: a checksummed
    /// line number does not become the command, and a KRL keyword is not a G/M command at all.
    #[test]
    fn the_leading_command_is_found_past_a_line_number() {
        let lines = parse_gcode_lines("N42 M1002 judge_flag : 1*38\n").unwrap();
        assert_eq!(
            lines[0].record,
            GcodeRecord::Other {
                letter: 'M',
                value: 1002.0
            }
        );
        assert_eq!(lines[0].raw, "N42 M1002 judge_flag : 1*38");
        // `PTP`/`LIN`/`CIRC`/`WAIT` are motion, so a malformed one is still refused.
        assert!(parse_gcode_lines("PTP X10 Y\n").is_err());
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
