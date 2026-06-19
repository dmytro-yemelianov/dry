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

use crate::ir::{Segment, Toolpath};
use crate::units::{Feedrate, Length, Volume};

const MAGIC: [u8; 4] = *b"DRY0";
const ENC_VER: u8 = 0;

/// A decode error — the bytes are not a valid Dry IR v0 binary.
#[derive(Debug, PartialEq, Eq)]
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

/// Decode a toolpath from the columnar binary form.
pub fn decode(buf: &[u8]) -> Result<Toolpath, CodecError> {
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
    let opt_len = |c: &[Option<f64>], i: usize| c[i].map(Length::mm);
    let mut segments = Vec::with_capacity(n);
    for i in 0..n {
        let kind_idx = r.u32()? as usize;
        let kind = dict.get(kind_idx).cloned().ok_or(CodecError::Truncated)?;
        let centre = match (cx[i], cy[i]) {
            (Some(a), Some(b)) => Some([Length::mm(a), Length::mm(b)]),
            _ => None,
        };
        segments.push(Segment {
            start: [opt_len(&sx, i), opt_len(&sy, i), opt_len(&sz, i)],
            end: [opt_len(&ex, i), opt_len(&ey, i), opt_len(&ez, i)],
            travel: travel[i],
            speed: Feedrate(speed[i]),
            length: Length::mm(length[i]),
            volume: Volume(volume[i]),
            filament: Length::mm(filament[i]),
            width: opt_len(&width, i),
            height: opt_len(&height, i),
            kind,
            centre,
            clockwise: clockwise[i],
            temperature: temperature[i],
            fan: fan[i],
            flow: flow[i],
            tool: tool[i],
            dwell_s: dwell_s[i],
            orientation: orientation[i],
        });
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
}
