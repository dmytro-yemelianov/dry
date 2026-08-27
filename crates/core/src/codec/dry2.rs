//! DRY2 — Delta-compressed binary toolpath streaming format (D1.7, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Layout:
//! - Magic: `b"DRY2"` (4 bytes)
//! - Version: `u32` (IR schema version)
//! - Segment Count: `u32`
//! - Segments: one row each, led by a `u8` flag byte. A full row carries delta-encoded $X, Y, Z$
//!   integer coordinates (microns = mm $\times 1000$) and a speed word, 17 bytes in total. A row
//!   whose [`FLAG_NO_COORDS`] bit is set is the flag byte alone, and stands for a segment whose end
//!   coordinates were never defined.
//!
//! The short row is not an optimisation. 2D work — a laser or a router that never emits a Z word —
//! leaves `end[2]` as `None` for every segment, and the encoder has always written a one-byte
//! placeholder for those. It was not in this layout, and the decoder did not know about it: it read
//! a flag byte and then unconditionally consumed sixteen more, so the first short row desynchronised
//! the cursor and the read ran off the end of the buffer. The bit makes the row self-describing.

use super::CodecError;
use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};

pub const DRY2_MAGIC: &[u8; 4] = b"DRY2";

/// Row is a travel move rather than an extruding one.
const FLAG_TRAVEL: u8 = 1 << 0;
/// The segment's end coordinates were never defined; this row is the flag byte alone.
///
/// Bit 7 rather than the next free low bit: a full row's flag byte can legitimately be `0`, so the
/// old zero-byte placeholder was indistinguishable from the start of an ordinary row. Any payload
/// written before this bit existed that contains a placeholder is undecodable by construction, so
/// nothing readable is invalidated by claiming the bit.
pub const FLAG_NO_COORDS: u8 = 1 << 7;

/// Encode a [`Toolpath`] into the DRY2 delta format.
pub fn encode_dry2(toolpath: &Toolpath) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(DRY2_MAGIC);
    buf.extend_from_slice(&toolpath.version.to_le_bytes());
    buf.extend_from_slice(&(toolpath.segments.len() as u32).to_le_bytes());

    let mut last_x_um: i32 = 0;
    let mut last_y_um: i32 = 0;
    let mut last_z_um: i32 = 0;

    for seg in &toolpath.segments {
        let (Some(ex), Some(ey), Some(ez)) = (seg.end[0], seg.end[1], seg.end[2]) else {
            // Short row: the flag byte alone. Marked so the decoder knows not to read a body.
            buf.push(FLAG_NO_COORDS);
            continue;
        };

        let curr_x_um = (ex.value() * 1000.0).round() as i32;
        let curr_y_um = (ey.value() * 1000.0).round() as i32;
        let curr_z_um = (ez.value() * 1000.0).round() as i32;

        let dx = curr_x_um - last_x_um;
        let dy = curr_y_um - last_y_um;
        let dz = curr_z_um - last_z_um;

        let mut flags: u8 = 0;
        if seg.travel {
            flags |= FLAG_TRAVEL;
        }
        if seg.speed.value() > 0.0 {
            flags |= 1 << 1;
        }
        if seg.volume.value() > 0.0 {
            flags |= 1 << 2;
        }

        buf.push(flags);
        buf.extend_from_slice(&dx.to_le_bytes());
        buf.extend_from_slice(&dy.to_le_bytes());
        buf.extend_from_slice(&dz.to_le_bytes());

        let speed_val = (seg.speed.value() * 10.0).round() as u32;
        buf.extend_from_slice(&speed_val.to_le_bytes());

        last_x_um = curr_x_um;
        last_y_um = curr_y_um;
        last_z_um = curr_z_um;
    }

    buf
}

/// Read four little-endian bytes, or report the payload as truncated.
fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, CodecError> {
    let end = cursor.checked_add(4).ok_or(CodecError::Truncated)?;
    let slice = bytes.get(*cursor..end).ok_or(CodecError::Truncated)?;
    *cursor = end;
    Ok(u32::from_le_bytes(
        slice.try_into().map_err(|_| CodecError::Truncated)?,
    ))
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Result<i32, CodecError> {
    read_u32(bytes, cursor).map(|value| value as i32)
}

/// The segment a short row stands for: no end coordinates, and no geometry derived from them.
fn undefined_segment(flags: u8) -> Segment {
    Segment {
        start: [None; 3],
        end: [None; 3],
        travel: (flags & FLAG_TRAVEL) != 0,
        speed: Feedrate(0.0),
        length: Length(0.0),
        volume: Volume(0.0),
        filament: Length(0.0),
        width: None,
        height: None,
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        control_points: None,
        orientation: None,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        power: None,
        dwell_s: None,
        manual_gcode: None,
    }
}

/// Decode a DRY2 binary payload into a [`Toolpath`].
pub fn decode_dry2(bytes: &[u8]) -> Result<Toolpath, CodecError> {
    if bytes.len() < 12 {
        return Err(CodecError::Truncated);
    }
    if &bytes[0..4] != DRY2_MAGIC {
        return Err(CodecError::BadMagic);
    }

    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let seg_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;

    let mut segments = Vec::with_capacity(seg_count);
    let mut cursor = 12;

    let mut curr_x_um: i32 = 0;
    let mut curr_y_um: i32 = 0;
    let mut curr_z_um: i32 = 0;

    for _ in 0..seg_count {
        if cursor >= bytes.len() {
            break;
        }
        let flags = bytes[cursor];
        cursor += 1;

        // A short row stands for a segment whose end was never defined — 2D work that emits no Z
        // word produces one per segment. Carry that through rather than inventing coordinates for
        // it, which is what makes the round trip faithful instead of merely non-crashing.
        if flags & FLAG_NO_COORDS != 0 {
            segments.push(undefined_segment(flags));
            continue;
        }

        // Checked reads: this decodes bytes from outside the process — the wasm binding exports it
        // straight to JavaScript — so a truncated or corrupt payload must be a CodecError, not an
        // out-of-bounds slice panic, which in wasm is an abort the caller cannot catch.
        let dx = read_i32(bytes, &mut cursor)?;
        let dy = read_i32(bytes, &mut cursor)?;
        let dz = read_i32(bytes, &mut cursor)?;
        let speed_raw = read_u32(bytes, &mut cursor)?;

        let prev_x = (curr_x_um as f64) / 1000.0;
        let prev_y = (curr_y_um as f64) / 1000.0;
        let prev_z = (curr_z_um as f64) / 1000.0;

        curr_x_um += dx;
        curr_y_um += dy;
        curr_z_um += dz;

        let end_x = (curr_x_um as f64) / 1000.0;
        let end_y = (curr_y_um as f64) / 1000.0;
        let end_z = (curr_z_um as f64) / 1000.0;

        let travel = (flags & (1 << 0)) != 0;
        let speed = (speed_raw as f64) / 10.0;

        let seg = Segment {
            start: [
                Some(Length(prev_x)),
                Some(Length(prev_y)),
                Some(Length(prev_z)),
            ],
            end: [
                Some(Length(end_x)),
                Some(Length(end_y)),
                Some(Length(end_z)),
            ],
            travel,
            speed: Feedrate(speed),
            length: Length(libm::hypot(end_x - prev_x, end_y - prev_y)),
            volume: Volume(0.0),
            filament: Length(0.0),
            width: None,
            height: None,
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            control_points: None,
            temperature: None,
            fan: None,
            flow: None,
            tool: None,
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
        };

        segments.push(seg);
    }

    Ok(Toolpath {
        version,
        segments,
        meta: None,
    })
}
