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
//!
//! `DRY1` is the streaming archive. It keeps the same values, but stores compressed row chunks:
//!
//! ```text
//! header (uncompressed):
//!   magic       "DRY1" (4 bytes)
//!   enc_ver     u8     (the streaming encoding version; 1)
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
mod error;
mod json;
mod util;

#[cfg(test)]
mod tests;

use crate::ir::{Meta, Segment, Toolpath};
use std::io::{BufReader, Cursor, Read};

pub use self::chunked::{decode_chunked_streaming, encode_chunked, ChunkedSegmentsIterator};
pub use self::columnar::{decode_streaming, encode, BinarySegmentsIterator};
pub use self::error::CodecError;
pub use self::json::JsonSegmentsIterator;

pub(super) const MAGIC: [u8; 4] = *b"DRY0";
pub(super) const ENC_VER: u8 = 0;
pub(super) const CHUNKED_MAGIC: [u8; 4] = *b"DRY1";
pub(super) const CHUNKED_ENC_VER: u8 = 1;
pub(super) const DEFAULT_CHUNK_SIZE: usize = 512;

pub type SegmentStream = Box<dyn Iterator<Item = Result<Segment, CodecError>>>;
pub type StreamingDecode = (u32, Option<Meta>, SegmentStream);

/// Decode either Dry binary form into a segment stream.
///
/// `DRY1` is decoded chunk-by-chunk. `DRY0` is accepted for compatibility, but it must still inflate
/// its full legacy columnar body before yielding segments.
pub fn decode_any_streaming<R: Read + 'static>(reader: R) -> Result<StreamingDecode, CodecError> {
    let mut reader = BufReader::new(reader);
    let magic = util::read_array::<4, _>(&mut reader)?;
    match magic {
        MAGIC => {
            let mut chained = Cursor::new(magic).chain(reader);
            let mut buf = Vec::new();
            chained.read_to_end(&mut buf).map_err(util::read_error)?;
            let (version, meta, iter) = decode_streaming(&buf)?;
            Ok((version, meta, Box::new(iter)))
        }
        CHUNKED_MAGIC => {
            let chained = Cursor::new(magic).chain(reader);
            let (version, meta, iter) = decode_chunked_streaming(chained)?;
            Ok((version, meta, Box::new(iter)))
        }
        _ => Err(CodecError::BadMagic),
    }
}

/// Decode a toolpath from either binary form.
pub fn decode(buf: &[u8]) -> Result<Toolpath, CodecError> {
    if buf.starts_with(&CHUNKED_MAGIC) {
        let (version, meta, iter) = decode_chunked_streaming(Cursor::new(buf))?;
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
