//! The binary **columnar** encoding of the Dry IR (P0.3) — a compact, lossless, self-describing
//! alternative to JSON for the L2 [`Toolpath`].
//!
//! Layout (little-endian): a small plaintext header, then a DEFLATE-compressed Arrow-style
//! struct-of-arrays body (each field a contiguous column — so the many repeated values, e.g. a
//! constant feedrate or bead width, collapse under compression):
//!
//! ```text
//! header (uncompressed):
//!   magic    "DRY0"   (4 bytes)
//!   enc_ver  u8       (the encoding version; 0)
//!   ir_ver   u32      (Toolpath.version — the IR schema version)
//!   n        u32      (segment count)
//!   body_len u32      (uncompressed body length, the inflate bound)
//! body (DEFLATE-compressed):
//!   bool columns  (bitmaps, ceil(n/8) bytes each):  travel, clockwise
//!   nullable f64 columns  (validity bitmap + n×f64):  start{x,y,z}, end{x,y,z}, width, height,
//!                                                     centre{x,y}
//!   dense   f64 columns  (n×f64):  speed, length, volume, filament
//!   channel columns  (nullable):  temperature, fan, flow, dwell_s (f64); tool (u32);
//!                                  orientation (validity bitmap + n×3 f64)
//!   kind    dictionary:  dict_len u32, [str_len u32, bytes…]…, then n×u32 indices
//!   meta    trailer:  present u8 (0/1); if 1, a u32 length + the UTF-8 bytes of the IR header
//!                     ([`crate::ir::Meta`]) as JSON — the reserved provenance/invariants slot.
//! ```
//!
//! Columns store the *typed* IR quantities ([`crate::units`]) as their raw `f64` bits
//! (`to_le_bytes`/`from_le_bytes`), so the round-trip is exact. `None` is recorded in the validity
//! bitmap (the value slot holds a `0.0` placeholder), so `decode(encode(ir)) == ir` for any toolpath.
//! The IR header ([`crate::ir::Meta`] — provenance + declared invariants) rides in the meta trailer.

use crate::ir::{Meta, Segment, Toolpath};
use crate::units::{Feedrate, Length, Volume};

const MAGIC: [u8; 4] = *b"DRY0";
const ENC_VER: u8 = 0;

/// A decode error — the bytes are not a valid Dry IR v0 binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// Ran out of bytes mid-field.
    Truncated,
    /// The magic prefix did not match.
    BadMagic,
    /// The encoding version is not supported by this build.
    UnsupportedVersion(u8),
    /// A dictionary string was not valid UTF-8.
    BadUtf8,
    /// The compressed body could not be inflated.
    BadCompression,
    /// The meta trailer was not valid `Meta` JSON.
    BadMeta,
    /// Generic or underlying I/O / JSON deserialization error.
    Other(String),
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::Truncated => write!(f, "truncated Dry IR binary"),
            CodecError::BadMagic => write!(f, "not a Dry IR binary (bad magic)"),
            CodecError::UnsupportedVersion(v) => write!(f, "unsupported encoding version {v}"),
            CodecError::BadUtf8 => write!(f, "invalid UTF-8 in kind dictionary"),
            CodecError::BadCompression => write!(f, "corrupt compressed body"),
            CodecError::BadMeta => write!(f, "invalid IR meta header"),
            CodecError::Other(s) => write!(f, "error: {s}"),
        }
    }
}

impl std::error::Error for CodecError {}

// ---- encode ----

fn push_bits(out: &mut Vec<u8>, n: usize, mut bit: impl FnMut(usize) -> bool) {
    let mut byte = 0u8;
    for i in 0..n {
        if bit(i) {
            byte |= 1 << (i % 8);
        }
        if i % 8 == 7 {
            out.push(byte);
            byte = 0;
        }
    }
    if !n.is_multiple_of(8) {
        out.push(byte);
    }
}

fn push_opt_col(out: &mut Vec<u8>, segs: &[Segment], get: impl Fn(&Segment) -> Option<f64>) {
    push_bits(out, segs.len(), |i| get(&segs[i]).is_some());
    for s in segs {
        out.extend_from_slice(&get(s).unwrap_or(0.0).to_le_bytes());
    }
}

fn push_col(out: &mut Vec<u8>, segs: &[Segment], get: impl Fn(&Segment) -> f64) {
    for s in segs {
        out.extend_from_slice(&get(s).to_le_bytes());
    }
}

fn push_opt_u32_col(out: &mut Vec<u8>, segs: &[Segment], get: impl Fn(&Segment) -> Option<u32>) {
    push_bits(out, segs.len(), |i| get(&segs[i]).is_some());
    for s in segs {
        out.extend_from_slice(&get(s).unwrap_or(0).to_le_bytes());
    }
}

fn push_opt_vec3_col(
    out: &mut Vec<u8>,
    segs: &[Segment],
    get: impl Fn(&Segment) -> Option<[f64; 3]>,
) {
    push_bits(out, segs.len(), |i| get(&segs[i]).is_some());
    for s in segs {
        for v in get(s).unwrap_or([0.0; 3]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}

/// Encode a toolpath to the compact columnar binary form.
pub fn encode(tp: &Toolpath) -> Vec<u8> {
    let segs = &tp.segments;
    let n = segs.len();

    // build the column body, then DEFLATE it (columns put like-valued data adjacent, which compresses
    // far better than row-interleaved JSON).
    let mut body = Vec::new();
    push_bits(&mut body, n, |i| segs[i].travel);
    push_bits(&mut body, n, |i| segs[i].clockwise);

    push_opt_col(&mut body, segs, |s| s.start[0].map(Length::value));
    push_opt_col(&mut body, segs, |s| s.start[1].map(Length::value));
    push_opt_col(&mut body, segs, |s| s.start[2].map(Length::value));
    push_opt_col(&mut body, segs, |s| s.end[0].map(Length::value));
    push_opt_col(&mut body, segs, |s| s.end[1].map(Length::value));
    push_opt_col(&mut body, segs, |s| s.end[2].map(Length::value));
    push_opt_col(&mut body, segs, |s| s.width.map(Length::value));
    push_opt_col(&mut body, segs, |s| s.height.map(Length::value));
    push_opt_col(&mut body, segs, |s| s.centre.map(|c| c[0].value()));
    push_opt_col(&mut body, segs, |s| s.centre.map(|c| c[1].value()));

    push_col(&mut body, segs, |s| s.speed.value());
    push_col(&mut body, segs, |s| s.length.value());
    push_col(&mut body, segs, |s| s.volume.value());
    push_col(&mut body, segs, |s| s.filament.value());

    // process channels (§3): nullable, so absent on motion-only paths (one bitmap each, ~0 bytes).
    push_opt_col(&mut body, segs, |s| s.temperature);
    push_opt_col(&mut body, segs, |s| s.fan);
    push_opt_col(&mut body, segs, |s| s.flow);
    push_opt_col(&mut body, segs, |s| s.dwell_s);
    push_opt_u32_col(&mut body, segs, |s| s.tool);
    push_opt_vec3_col(&mut body, segs, |s| s.orientation);

    // kind dictionary (line/arc repeat, so this is tiny) + per-segment u32 index.
    let mut dict: Vec<&str> = Vec::new();
    let mut idx: Vec<u32> = Vec::with_capacity(n);
    for s in segs {
        let pos = dict.iter().position(|k| *k == s.kind).unwrap_or_else(|| {
            dict.push(&s.kind);
            dict.len() - 1
        });
        idx.push(pos as u32);
    }
    body.extend_from_slice(&(dict.len() as u32).to_le_bytes());
    for k in &dict {
        body.extend_from_slice(&(k.len() as u32).to_le_bytes());
        body.extend_from_slice(k.as_bytes());
    }
    for i in &idx {
        body.extend_from_slice(&i.to_le_bytes());
    }

    // meta trailer (the self-describing IR header): a present-flag, then — when present — a
    // length-prefixed JSON blob. Absent on a header-free toolpath, so it costs one byte.
    match &tp.meta {
        None => body.push(0),
        Some(meta) => {
            body.push(1);
            let json = serde_json::to_string(meta).expect("Meta serialises");
            body.extend_from_slice(&(json.len() as u32).to_le_bytes());
            body.extend_from_slice(json.as_bytes());
        }
    }

    let compressed = miniz_oxide::deflate::compress_to_vec(&body, 8);
    let mut out = Vec::with_capacity(17 + compressed.len());
    out.extend_from_slice(&MAGIC);
    out.push(ENC_VER);
    out.extend_from_slice(&tp.version.to_le_bytes());
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    out
}

// ---- decode ----

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Reader<'a> {
        Reader { buf, at: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let end = self.at.checked_add(n).ok_or(CodecError::Truncated)?;
        let slice = self.buf.get(self.at..end).ok_or(CodecError::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, CodecError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f64(&mut self) -> Result<f64, CodecError> {
        let b = self.take(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }
    fn bits(&mut self, n: usize) -> Result<Vec<bool>, CodecError> {
        let bytes = self.take(n.div_ceil(8))?;
        Ok((0..n).map(|i| (bytes[i / 8] >> (i % 8)) & 1 == 1).collect())
    }
    fn opt_col(&mut self, n: usize) -> Result<Vec<Option<f64>>, CodecError> {
        let valid = self.bits(n)?;
        let mut col = Vec::with_capacity(n);
        for v in valid {
            let x = self.f64()?;
            col.push(if v { Some(x) } else { None });
        }
        Ok(col)
    }
    fn col(&mut self, n: usize) -> Result<Vec<f64>, CodecError> {
        (0..n).map(|_| self.f64()).collect()
    }
    fn opt_u32_col(&mut self, n: usize) -> Result<Vec<Option<u32>>, CodecError> {
        let valid = self.bits(n)?;
        let mut col = Vec::with_capacity(n);
        for v in valid {
            let x = self.u32()?;
            col.push(if v { Some(x) } else { None });
        }
        Ok(col)
    }
    fn opt_vec3_col(&mut self, n: usize) -> Result<Vec<Option<[f64; 3]>>, CodecError> {
        let valid = self.bits(n)?;
        let mut col = Vec::with_capacity(n);
        for v in valid {
            let triple = [self.f64()?, self.f64()?, self.f64()?];
            col.push(if v { Some(triple) } else { None });
        }
        Ok(col)
    }
}

#[derive(Debug, Clone)]
pub struct BinarySegmentsIterator {
    pub n: usize,
    pub i: usize,
    pub travel: Vec<bool>,
    pub clockwise: Vec<bool>,
    pub sx: Vec<Option<f64>>,
    pub sy: Vec<Option<f64>>,
    pub sz: Vec<Option<f64>>,
    pub ex: Vec<Option<f64>>,
    pub ey: Vec<Option<f64>>,
    pub ez: Vec<Option<f64>>,
    pub width: Vec<Option<f64>>,
    pub height: Vec<Option<f64>>,
    pub cx: Vec<Option<f64>>,
    pub cy: Vec<Option<f64>>,
    pub speed: Vec<f64>,
    pub length: Vec<f64>,
    pub volume: Vec<f64>,
    pub filament: Vec<f64>,
    pub temperature: Vec<Option<f64>>,
    pub fan: Vec<Option<f64>>,
    pub flow: Vec<Option<f64>>,
    pub dwell_s: Vec<Option<f64>>,
    pub tool: Vec<Option<u32>>,
    pub orientation: Vec<Option<[f64; 3]>>,
    pub dict: Vec<String>,
    pub kind_indices: Vec<u32>,
}

impl Iterator for BinarySegmentsIterator {
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.n {
            return None;
        }
        let i = self.i;
        self.i += 1;

        let kind_idx = match self.kind_indices.get(i) {
            Some(&idx) => idx as usize,
            None => return Some(Err(CodecError::Truncated)),
        };
        let kind = match self.dict.get(kind_idx) {
            Some(k) => k.clone(),
            None => return Some(Err(CodecError::Truncated)),
        };

        let opt_len = |c: &[Option<f64>], idx: usize| c[idx].map(Length::mm);
        let centre = match (self.cx[i], self.cy[i]) {
            (Some(a), Some(b)) => Some([Length::mm(a), Length::mm(b)]),
            _ => None,
        };

        Some(Ok(Segment {
            start: [opt_len(&self.sx, i), opt_len(&self.sy, i), opt_len(&self.sz, i)],
            end: [opt_len(&self.ex, i), opt_len(&self.ey, i), opt_len(&self.ez, i)],
            travel: self.travel[i],
            speed: Feedrate(self.speed[i]),
            length: Length::mm(self.length[i]),
            volume: Volume(self.volume[i]),
            filament: Length::mm(self.filament[i]),
            width: opt_len(&self.width, i),
            height: opt_len(&self.height, i),
            kind,
            centre,
            clockwise: self.clockwise[i],
            temperature: self.temperature[i],
            fan: self.fan[i],
            flow: self.flow[i],
            tool: self.tool[i],
            dwell_s: self.dwell_s[i],
            orientation: self.orientation[i],
        }))
    }
}

/// Decode a toolpath from the columnar binary form streamingly.
pub fn decode_streaming(buf: &[u8]) -> Result<(u32, Option<Meta>, BinarySegmentsIterator), CodecError> {
    let mut h = Reader::new(buf);
    if h.take(4)? != MAGIC {
        return Err(CodecError::BadMagic);
    }
    let enc = h.u8()?;
    if enc != ENC_VER {
        return Err(CodecError::UnsupportedVersion(enc));
    }
    let version = h.u32()?;
    let n = h.u32()? as usize;
    let body_len = h.u32()? as usize;

    let body = miniz_oxide::inflate::decompress_to_vec_with_limit(&buf[h.at..], body_len)
        .map_err(|_| CodecError::BadCompression)?;
    let mut r = Reader::new(&body);

    let travel = r.bits(n)?;
    let clockwise = r.bits(n)?;

    let sx = r.opt_col(n)?;
    let sy = r.opt_col(n)?;
    let sz = r.opt_col(n)?;
    let ex = r.opt_col(n)?;
    let ey = r.opt_col(n)?;
    let ez = r.opt_col(n)?;
    let width = r.opt_col(n)?;
    let height = r.opt_col(n)?;
    let cx = r.opt_col(n)?;
    let cy = r.opt_col(n)?;

    let speed = r.col(n)?;
    let length = r.col(n)?;
    let volume = r.col(n)?;
    let filament = r.col(n)?;

    let temperature = r.opt_col(n)?;
    let fan = r.opt_col(n)?;
    let flow = r.opt_col(n)?;
    let dwell_s = r.opt_col(n)?;
    let tool = r.opt_u32_col(n)?;
    let orientation = r.opt_vec3_col(n)?;

    let dict_len = r.u32()? as usize;
    let mut dict: Vec<String> = Vec::with_capacity(dict_len);
    for _ in 0..dict_len {
        let len = r.u32()? as usize;
        let s = std::str::from_utf8(r.take(len)?).map_err(|_| CodecError::BadUtf8)?;
        dict.push(s.to_string());
    }

    let mut kind_indices = Vec::with_capacity(n);
    for _ in 0..n {
        kind_indices.push(r.u32()?);
    }

    // meta trailer: present-flag, then a length-prefixed JSON blob when present.
    let meta = match r.u8()? {
        0 => None,
        _ => {
            let len = r.u32()? as usize;
            let json = std::str::from_utf8(r.take(len)?).map_err(|_| CodecError::BadUtf8)?;
            Some(serde_json::from_str(json).map_err(|_| CodecError::BadMeta)?)
        }
    };

    let iter = BinarySegmentsIterator {
        n,
        i: 0,
        travel,
        clockwise,
        sx,
        sy,
        sz,
        ex,
        ey,
        ez,
        width,
        height,
        cx,
        cy,
        speed,
        length,
        volume,
        filament,
        temperature,
        fan,
        flow,
        dwell_s,
        tool,
        orientation,
        dict,
        kind_indices,
    };

    Ok((version, meta, iter))
}

/// Decode a toolpath from the columnar binary form.
pub fn decode(buf: &[u8]) -> Result<Toolpath, CodecError> {
    let (version, meta, iter) = decode_streaming(buf)?;
    let mut segments = Vec::with_capacity(iter.n);
    for res in iter {
        segments.push(res?);
    }
    Ok(Toolpath {
        version,
        meta,
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toolpath_round_trips() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert_eq!(decode(&encode(&tp)).unwrap(), tp);
    }

    #[test]
    fn bad_magic_is_an_error() {
        assert_eq!(decode(b"XXXX...."), Err(CodecError::BadMagic));
        assert_eq!(decode(b"DRY"), Err(CodecError::Truncated));
    }

    #[test]
    fn test_streaming_decoders() {
        use crate::units::{Feedrate, Length, Volume};
        let tp = Toolpath {
            version: 3,
            meta: None,
            segments: vec![
                Segment {
                    start: [Some(Length::mm(1.0)), Some(Length::mm(2.0)), Some(Length::mm(3.0))],
                    end: [Some(Length::mm(4.0)), Some(Length::mm(5.0)), Some(Length::mm(6.0))],
                    travel: false,
                    speed: Feedrate(1200.0),
                    length: Length::mm(5.196),
                    volume: Volume(0.62),
                    filament: Length::mm(0.2),
                    width: Some(Length::mm(0.6)),
                    height: Some(Length::mm(0.2)),
                    kind: "line".to_string(),
                    centre: None,
                    clockwise: false,
                    temperature: Some(210.0),
                    fan: Some(0.5),
                    flow: Some(1.0),
                    tool: Some(0),
                    dwell_s: None,
                    orientation: None,
                },
                Segment {
                    start: [Some(Length::mm(4.0)), Some(Length::mm(5.0)), Some(Length::mm(6.0))],
                    end: [Some(Length::mm(4.0)), Some(Length::mm(5.0)), Some(Length::mm(6.0))],
                    travel: true,
                    speed: Feedrate(0.0),
                    length: Length::ZERO,
                    volume: Volume::ZERO,
                    filament: Length::ZERO,
                    width: None,
                    height: None,
                    kind: "dwell".to_string(),
                    centre: None,
                    clockwise: false,
                    temperature: None,
                    fan: None,
                    flow: None,
                    tool: None,
                    dwell_s: Some(1.5),
                    orientation: None,
                },
            ],
        };

        // Binary streaming roundtrip
        let bytes = encode(&tp);
        let (version, meta, iter) = decode_streaming(&bytes).unwrap();
        assert_eq!(version, 3);
        assert_eq!(meta, None);
        let decoded_segs: Vec<Segment> = iter.map(|r| r.unwrap()).collect();
        assert_eq!(decoded_segs, tp.segments);

        // JSON streaming roundtrip
        let json_str = tp.to_json();
        let json_iter = JsonSegmentsIterator::new(json_str.as_bytes());
        let json_segs: Vec<Segment> = json_iter.map(|r| r.unwrap()).collect();
        assert_eq!(json_segs, tp.segments);
    }
}

use std::io::{BufReader, Read};

pub struct JsonSegmentsIterator<R: Read> {
    reader: BufReader<R>,
    started: bool,
    done: bool,
}

impl<R: Read> Iterator for JsonSegmentsIterator<R> {
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        if !self.started {
            self.started = true;
            if let Err(e) = self.skip_to_segments() {
                self.done = true;
                return Some(Err(e));
            }
        }

        match self.skip_whitespace_and_comma() {
            Ok(true) => {
                self.done = true;
                None
            }
            Ok(false) => {
                let mut de = serde_json::Deserializer::from_reader(&mut self.reader);
                match serde::Deserialize::deserialize(&mut de) {
                    Ok(seg) => Some(Ok(seg)),
                    Err(e) => {
                        self.done = true;
                        Some(Err(CodecError::Other(e.to_string())))
                    }
                }
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

impl<R: Read> JsonSegmentsIterator<R> {
    pub fn new(reader: R) -> Self {
        JsonSegmentsIterator {
            reader: BufReader::new(reader),
            started: false,
            done: false,
        }
    }

    fn skip_to_segments(&mut self) -> Result<(), CodecError> {
        let pattern = b"\"segments\"";
        let mut matched = 0;
        let mut buf = [0u8; 1];
        loop {
            self.reader.read_exact(&mut buf).map_err(|e| CodecError::Other(e.to_string()))?;
            if buf[0] == pattern[matched] {
                matched += 1;
                if matched == pattern.len() {
                    break;
                }
            } else {
                matched = 0;
                if buf[0] == pattern[0] {
                    matched = 1;
                }
            }
        }
        loop {
            self.reader.read_exact(&mut buf).map_err(|e| CodecError::Other(e.to_string()))?;
            if buf[0] == b'[' {
                break;
            }
        }
        Ok(())
    }

    fn skip_whitespace_and_comma(&mut self) -> Result<bool, CodecError> {
        use std::io::BufRead;
        loop {
            let available = self.reader.fill_buf().map_err(|e| CodecError::Other(e.to_string()))?;
            if available.is_empty() {
                return Err(CodecError::Truncated);
            }
            let c = available[0];
            if c.is_ascii_whitespace() || c == b',' {
                self.reader.consume(1);
            } else if c == b']' {
                self.reader.consume(1);
                return Ok(true);
            } else {
                return Ok(false);
            }
        }
    }
}
