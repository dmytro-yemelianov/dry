//! The binary encodings of the Dry IR (P0.3) — compact, lossless, self-describing alternatives to
//! JSON for the L2 [`Toolpath`].
//!
//! `DRY0` is the original columnar archive. Its layout (little-endian): a small plaintext header,
//! then a DEFLATE-compressed Arrow-style struct-of-arrays body (each field a contiguous column — so
//! the many repeated values, e.g. a constant feedrate or bead width, collapse under compression):
//!
//! ```text
//! header (uncompressed):
//!   magic    "DRY0"   (4 bytes)
//!   enc_ver  u8       (the encoding version; 0 = no manual_gcode, 1, 2 = + power)
//!   ir_ver   u32      (Toolpath.version — the IR schema version)
//!   n        u32      (segment count)
//!   body_len u32      (uncompressed body length, the inflate bound)
//! body (DEFLATE-compressed):
//!   bool columns  (bitmaps, ceil(n/8) bytes each):  travel, clockwise
//!   nullable f64 columns  (validity bitmap + n×f64):  start{x,y,z}, end{x,y,z}, width, height,
//!                                                     centre{x,y}
//!   dense   f64 columns  (n×f64):  speed, length, volume, filament
//!   channel columns  (nullable):  temperature, fan, flow, dwell_s (f64); manual_gcode (utf-8); tool (u32);
//!                                  orientation (validity bitmap + n×3 f64); power (f64, enc_ver 2)
//!   kind    dictionary:  dict_len u32, [str_len u32, bytes…]…, then n×u32 indices
//!   meta    trailer:  present u8 (0/1); if 1, a u32 length + the UTF-8 bytes of the IR header
//!                     ([`crate::ir::Meta`]) as JSON — the reserved provenance/invariants slot.
//! ```
//!
//! `enc_ver` is the **minimum reader version required**, not a monotonic stamp: a `DRY0` column costs
//! `ceil(n/8) + 8n` bytes whether or not anything fills it, so the encoder emits the lowest layout
//! that can carry the toolpath. A power-free toolpath is therefore still written at `enc_ver 1`, and
//! every archive produced before the column existed is byte-for-byte reproducible.
//!
//! Columns store the *typed* IR quantities ([`crate::units`]) as their raw `f64` bits
//! (`to_le_bytes`/`from_le_bytes`), so the round-trip is exact. `None` is recorded in the validity
//! bitmap (the value slot holds a `0.0` placeholder), so `decode(encode(ir)) == ir` for any toolpath.
//! The IR header ([`crate::ir::Meta`] — provenance + declared invariants) rides in the meta trailer.
//!
//! `DRY1` is the streaming archive. It keeps the same values, but stores compressed row chunks:
//!
//! ```text
//! header (uncompressed):
//!   magic       "DRY1" (4 bytes)
//!   enc_ver     u8     (the streaming encoding version; 2; version 1 is accepted without manual_gcode)
//!                      (`power` needs no bump here: a `DRY1` row is self-describing through its flag
//!                       word, so the field costs nothing when unset and an older reader refuses the
//!                       unknown bit rather than misreading the row)
//!   ir_ver      u32
//!   n           u32
//!   block_size  u32
//!   meta        present u8, then optional length-prefixed JSON
//! blocks:
//!   block_n     u32
//!   body_len    u32    (uncompressed chunk length, the inflate bound)
//!   deflate_len u32
//!   body        DEFLATE-compressed row records
//! ```
//!
//! `DRY1` is less column-compression-friendly than `DRY0`, but it lets CLI analyses and emitters
//! decode one bounded chunk at a time instead of inflating the full toolpath body up front.

mod chunked;
mod columnar;
mod dry2;
mod error;
mod json;
pub mod threemf;
mod util;

pub use threemf::{export_3mf_xml, import_3mf_xml, ThreeMfError};

#[cfg(test)]
mod tests;

use crate::ir::{Meta, Segment, Toolpath};
use std::io::{BufReader, Cursor, Read};

pub use self::chunked::{
    decode_chunked_streaming, decode_chunked_streaming_with_limits, encode_chunked,
    try_encode_chunked, ChunkedSegmentsIterator,
};
pub use self::columnar::{
    decode_streaming, decode_streaming_with_limits, encode, try_encode, BinarySegmentsIterator,
};
pub use self::dry2::{decode_dry2, encode_dry2, DRY2_MAGIC};
pub use self::error::CodecError;
pub use self::json::JsonSegmentsIterator;

pub(super) const MAGIC: [u8; 4] = *b"DRY0";
pub(super) const LEGACY_ENC_VER: u8 = 0;
pub(super) const ENC_VER: u8 = 1;
/// `DRY0` layout carrying the `power` column. See [`columnar::try_encode`]: `enc_ver` is the
/// *minimum reader version required*, so a toolpath with no power stays at [`ENC_VER`] and its bytes
/// do not move.
pub(super) const POWER_ENC_VER: u8 = 2;
pub(super) const CHUNKED_MAGIC: [u8; 4] = *b"DRY1";
pub(super) const LEGACY_CHUNKED_ENC_VER: u8 = 1;
pub(super) const CHUNKED_ENC_VER: u8 = 2;
pub(super) const DEFAULT_CHUNK_SIZE: usize = 512;

/// Resource budgets applied before binary decoding allocates or decompresses attacker-controlled
/// lengths. Callers handling unusually large trusted archives can opt in to higher limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_input_bytes: usize,
    pub max_segments: usize,
    pub max_columnar_body_bytes: usize,
    pub max_block_segments: usize,
    pub max_block_bytes: usize,
    pub max_metadata_bytes: usize,
    pub max_string_bytes: usize,
    pub max_control_points_per_segment: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 512 * 1024 * 1024,
            max_segments: 2_000_000,
            max_columnar_body_bytes: 512 * 1024 * 1024,
            max_block_segments: 65_536,
            max_block_bytes: 64 * 1024 * 1024,
            max_metadata_bytes: 1024 * 1024,
            max_string_bytes: 1024 * 1024,
            max_control_points_per_segment: 1_000_000,
        }
    }
}

impl DecodeLimits {
    pub(super) fn ensure(
        &self,
        field: &'static str,
        actual: usize,
        limit: usize,
    ) -> Result<(), CodecError> {
        if actual > limit {
            Err(CodecError::LimitExceeded {
                field,
                limit,
                actual,
            })
        } else {
            Ok(())
        }
    }
}

pub type SegmentStream = Box<dyn Iterator<Item = Result<Segment, CodecError>>>;
pub type StreamingDecode = (u32, Option<Meta>, SegmentStream);

/// Decode either Dry binary form into a segment stream.
///
/// `DRY1` is decoded chunk-by-chunk. `DRY0` is accepted for compatibility, but it must still inflate
/// its full legacy columnar body before yielding segments.
pub fn decode_any_streaming<R: Read + 'static>(reader: R) -> Result<StreamingDecode, CodecError> {
    decode_any_streaming_with_limits(reader, &DecodeLimits::default())
}

/// Decode either Dry binary form using explicit resource budgets.
pub fn decode_any_streaming_with_limits<R: Read + 'static>(
    reader: R,
    limits: &DecodeLimits,
) -> Result<StreamingDecode, CodecError> {
    let mut reader = BufReader::new(reader);
    let magic = util::read_array::<4, _>(&mut reader)?;
    match magic {
        MAGIC => {
            let chained = Cursor::new(magic).chain(reader);
            let mut buf = Vec::new();
            let read_limit = limits.max_input_bytes.saturating_add(1) as u64;
            chained
                .take(read_limit)
                .read_to_end(&mut buf)
                .map_err(util::read_error)?;
            limits.ensure("input bytes", buf.len(), limits.max_input_bytes)?;
            let (version, meta, iter) = decode_streaming_with_limits(&buf, limits)?;
            Ok((version, meta, Box::new(iter)))
        }
        CHUNKED_MAGIC => {
            let chained = Cursor::new(magic).chain(reader);
            let (version, meta, iter) = decode_chunked_streaming_with_limits(chained, limits)?;
            Ok((version, meta, Box::new(iter)))
        }
        _ => Err(CodecError::BadMagic),
    }
}

/// Decode a toolpath from either binary form.
pub fn decode(buf: &[u8]) -> Result<Toolpath, CodecError> {
    decode_with_limits(buf, &DecodeLimits::default())
}

/// Decode a toolpath using explicit resource budgets.
pub fn decode_with_limits(buf: &[u8], limits: &DecodeLimits) -> Result<Toolpath, CodecError> {
    limits.ensure("input bytes", buf.len(), limits.max_input_bytes)?;
    if buf.starts_with(&CHUNKED_MAGIC) {
        let (version, meta, iter) = decode_chunked_streaming_with_limits(Cursor::new(buf), limits)?;
        let mut segments = Vec::new();
        for res in iter {
            segments.push(res?);
        }
        return Ok(Toolpath {
            version,
            meta,
            segments,
        });
    }

    let (version, meta, iter) = decode_streaming_with_limits(buf, limits)?;
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
