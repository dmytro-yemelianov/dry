//! DRY2 — Delta-compressed binary toolpath streaming format (D1.7, `docs/20-dry-ir-ecosystem-implementation-plan.md` §5).
//!
//! Layout:
//! - Magic: `b"DRY2"` (4 bytes)
//! - Version: `u32` (IR schema version)
//! - Segment Count: `u32`
//! - Segments: Row-wise delta-encoded $X, Y, Z$ integer coordinates (microns = mm $\times 1000$) with bit-flags.

use super::CodecError;
use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};

pub const DRY2_MAGIC: &[u8; 4] = b"DRY2";

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
            // Write placeholder if coordinates undefined
            buf.push(0);
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
            flags |= 1 << 0;
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

        let dx = i32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let dy = i32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let dz = i32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;

        let speed_raw = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;

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
            start: [Some(Length(prev_x)), Some(Length(prev_y)), Some(Length(prev_z))],
            end: [Some(Length(end_x)), Some(Length(end_y)), Some(Length(end_z))],
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
