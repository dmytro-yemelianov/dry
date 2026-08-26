use super::util::{
    checked_u32_len, decompress_exact, read_array, read_u32, read_u8, read_vec, Reader,
};
use super::{
    CodecError, DecodeLimits, CHUNKED_ENC_VER, CHUNKED_MAGIC, DEFAULT_CHUNK_SIZE,
    LEGACY_CHUNKED_ENC_VER,
};
use crate::ir::{Meta, Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};
use std::io::{BufReader, Read};

const FLAG_TRAVEL: u32 = 1 << 0;
const FLAG_CLOCKWISE: u32 = 1 << 1;
const FLAG_START_X: u32 = 1 << 2;
const FLAG_START_Y: u32 = 1 << 3;
const FLAG_START_Z: u32 = 1 << 4;
const FLAG_END_X: u32 = 1 << 5;
const FLAG_END_Y: u32 = 1 << 6;
const FLAG_END_Z: u32 = 1 << 7;
const FLAG_WIDTH: u32 = 1 << 8;
const FLAG_HEIGHT: u32 = 1 << 9;
const FLAG_CENTRE: u32 = 1 << 10;
const FLAG_TEMPERATURE: u32 = 1 << 11;
const FLAG_FAN: u32 = 1 << 12;
const FLAG_FLOW: u32 = 1 << 13;
const FLAG_DWELL: u32 = 1 << 14;
const FLAG_TOOL: u32 = 1 << 15;
const FLAG_ORIENTATION: u32 = 1 << 16;
const FLAG_CONTROL_POINTS: u32 = 1 << 17;
const FLAG_MANUAL_GCODE: u32 = 1 << 18;
const FLAG_POWER: u32 = 1 << 19;
const LEGACY_KNOWN_SEGMENT_FLAGS: u32 = (1 << 18) - 1;
// `power` claims a flag bit rather than an `enc_ver`: a `DRY1` row already describes itself through
// this word, so the field costs nothing on a row that lacks it — every power-free stream stays
// byte-identical — and a reader built before the bit existed refuses it as an unknown flag instead
// of misreading the row. `DRY0`, whose columns are dense, has no such escape and does bump.
const KNOWN_SEGMENT_FLAGS: u32 = (1 << 20) - 1;

fn segment_kind_tag(kind: SegmentKind) -> u8 {
    match kind {
        SegmentKind::Line => 0,
        SegmentKind::Arc => 1,
        SegmentKind::Spline => 2,
        SegmentKind::Dwell => 3,
        SegmentKind::Retract => 4,
        SegmentKind::Unretract => 5,
        SegmentKind::Deposit => 6,
        SegmentKind::ManualGcode => 7,
    }
}

fn segment_kind_from_tag(tag: u8) -> Result<SegmentKind, CodecError> {
    match tag {
        0 => Ok(SegmentKind::Line),
        1 => Ok(SegmentKind::Arc),
        2 => Ok(SegmentKind::Spline),
        3 => Ok(SegmentKind::Dwell),
        4 => Ok(SegmentKind::Retract),
        5 => Ok(SegmentKind::Unretract),
        6 => Ok(SegmentKind::Deposit),
        7 => Ok(SegmentKind::ManualGcode),
        _ => Err(CodecError::BadKind(format!("tag {tag}"))),
    }
}

fn push_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_opt_length(out: &mut Vec<u8>, value: Option<Length>) {
    if let Some(value) = value {
        push_f64(out, value.value());
    }
}

fn push_opt_f64(out: &mut Vec<u8>, value: Option<f64>) {
    if let Some(value) = value {
        push_f64(out, value);
    }
}

fn push_opt_string(out: &mut Vec<u8>, value: Option<&str>) -> Result<(), CodecError> {
    if let Some(value) = value {
        push_u32(out, checked_u32_len(value.len(), "manual gcode string")?);
        out.extend_from_slice(value.as_bytes());
    }
    Ok(())
}

fn encode_segment_row(out: &mut Vec<u8>, s: &Segment) -> Result<(), CodecError> {
    let mut flags = 0u32;
    if s.travel {
        flags |= FLAG_TRAVEL;
    }
    if s.clockwise {
        flags |= FLAG_CLOCKWISE;
    }
    if s.start[0].is_some() {
        flags |= FLAG_START_X;
    }
    if s.start[1].is_some() {
        flags |= FLAG_START_Y;
    }
    if s.start[2].is_some() {
        flags |= FLAG_START_Z;
    }
    if s.end[0].is_some() {
        flags |= FLAG_END_X;
    }
    if s.end[1].is_some() {
        flags |= FLAG_END_Y;
    }
    if s.end[2].is_some() {
        flags |= FLAG_END_Z;
    }
    if s.width.is_some() {
        flags |= FLAG_WIDTH;
    }
    if s.height.is_some() {
        flags |= FLAG_HEIGHT;
    }
    if s.centre.is_some() {
        flags |= FLAG_CENTRE;
    }
    if s.temperature.is_some() {
        flags |= FLAG_TEMPERATURE;
    }
    if s.fan.is_some() {
        flags |= FLAG_FAN;
    }
    if s.flow.is_some() {
        flags |= FLAG_FLOW;
    }
    if s.dwell_s.is_some() {
        flags |= FLAG_DWELL;
    }
    if s.tool.is_some() {
        flags |= FLAG_TOOL;
    }
    if s.orientation.is_some() {
        flags |= FLAG_ORIENTATION;
    }
    if s.control_points.is_some() {
        flags |= FLAG_CONTROL_POINTS;
    }
    if s.manual_gcode.is_some() {
        flags |= FLAG_MANUAL_GCODE;
    }
    if s.power.is_some() {
        flags |= FLAG_POWER;
    }

    push_u32(out, flags);
    out.push(segment_kind_tag(s.kind));

    push_opt_length(out, s.start[0]);
    push_opt_length(out, s.start[1]);
    push_opt_length(out, s.start[2]);
    push_opt_length(out, s.end[0]);
    push_opt_length(out, s.end[1]);
    push_opt_length(out, s.end[2]);
    push_opt_length(out, s.width);
    push_opt_length(out, s.height);
    if let Some(centre) = s.centre {
        push_f64(out, centre[0].value());
        push_f64(out, centre[1].value());
    }

    push_f64(out, s.speed.value());
    push_f64(out, s.length.value());
    push_f64(out, s.volume.value());
    push_f64(out, s.filament.value());

    push_opt_f64(out, s.temperature);
    push_opt_f64(out, s.fan);
    push_opt_f64(out, s.flow);
    push_opt_f64(out, s.dwell_s);
    push_opt_string(out, s.manual_gcode.as_deref())?;
    if let Some(tool) = s.tool {
        push_u32(out, tool);
    }
    if let Some(orientation) = s.orientation {
        for axis in orientation {
            push_f64(out, axis);
        }
    }
    if let Some(points) = &s.control_points {
        push_u32(out, checked_u32_len(points.len(), "control point count")?);
        for point in points {
            push_f64(out, point[0].value());
            push_f64(out, point[1].value());
            push_f64(out, point[2].value());
        }
    }
    // Newest field last, so an older row layout is a prefix of a newer one.
    push_opt_f64(out, s.power);
    Ok(())
}

pub(super) fn try_encode_chunked_with_block_size(
    tp: &Toolpath,
    block_size: usize,
) -> Result<Vec<u8>, CodecError> {
    let block_size = block_size.max(1);
    let segment_count = checked_u32_len(tp.segments.len(), "segment count")?;
    let block_size_u32 = checked_u32_len(block_size, "chunk block size")?;
    let mut out = Vec::new();
    out.extend_from_slice(&CHUNKED_MAGIC);
    out.push(CHUNKED_ENC_VER);
    push_u32(&mut out, tp.version);
    push_u32(&mut out, segment_count);
    push_u32(&mut out, block_size_u32);
    match &tp.meta {
        None => out.push(0),
        Some(meta) => {
            out.push(1);
            let json = serde_json::to_string(meta).expect("Meta serialises");
            push_u32(&mut out, checked_u32_len(json.len(), "meta JSON")?);
            out.extend_from_slice(json.as_bytes());
        }
    }

    for chunk in tp.segments.chunks(block_size) {
        let mut body = Vec::new();
        for segment in chunk {
            encode_segment_row(&mut body, segment)?;
        }
        let compressed = miniz_oxide::deflate::compress_to_vec(&body, 8);
        push_u32(
            &mut out,
            checked_u32_len(chunk.len(), "chunk segment count")?,
        );
        push_u32(&mut out, checked_u32_len(body.len(), "chunk body length")?);
        push_u32(
            &mut out,
            checked_u32_len(compressed.len(), "compressed chunk length")?,
        );
        out.extend_from_slice(&compressed);
    }

    Ok(out)
}

#[cfg(test)]
pub(super) fn encode_chunked_with_block_size(tp: &Toolpath, block_size: usize) -> Vec<u8> {
    try_encode_chunked_with_block_size(tp, block_size).expect("Dry chunked binary encode failed")
}

/// Encode a toolpath to the chunked streaming binary form.
pub fn try_encode_chunked(tp: &Toolpath) -> Result<Vec<u8>, CodecError> {
    try_encode_chunked_with_block_size(tp, DEFAULT_CHUNK_SIZE)
}

/// Encode a toolpath to the chunked streaming binary form.
pub fn encode_chunked(tp: &Toolpath) -> Vec<u8> {
    try_encode_chunked(tp).expect("Dry chunked binary encode failed")
}

fn opt_length_from_flag(
    r: &mut Reader<'_>,
    flags: u32,
    flag: u32,
) -> Result<Option<Length>, CodecError> {
    if flags & flag == 0 {
        Ok(None)
    } else {
        Ok(Some(Length::mm(r.f64()?)))
    }
}

fn opt_f64_from_flag(r: &mut Reader<'_>, flags: u32, flag: u32) -> Result<Option<f64>, CodecError> {
    if flags & flag == 0 {
        Ok(None)
    } else {
        Ok(Some(r.f64()?))
    }
}

fn opt_string_from_flag(
    r: &mut Reader<'_>,
    flags: u32,
    flag: u32,
    limits: &DecodeLimits,
) -> Result<Option<String>, CodecError> {
    if flags & flag == 0 {
        Ok(None)
    } else {
        let len = r.u32()? as usize;
        limits.ensure("manual gcode bytes", len, limits.max_string_bytes)?;
        let bytes = r.take(len)?;
        let value = std::str::from_utf8(bytes).map_err(|_| CodecError::BadUtf8)?;
        Ok(Some(value.to_string()))
    }
}

fn decode_segment_row(
    r: &mut Reader<'_>,
    enc: u8,
    limits: &DecodeLimits,
) -> Result<Segment, CodecError> {
    let flags = r.u32()?;
    let known_flags = if enc == LEGACY_CHUNKED_ENC_VER {
        LEGACY_KNOWN_SEGMENT_FLAGS
    } else {
        KNOWN_SEGMENT_FLAGS
    };
    if flags & !known_flags != 0 {
        return Err(CodecError::Other(format!(
            "unsupported segment flags 0x{:08x}",
            flags & !known_flags
        )));
    }
    let kind = segment_kind_from_tag(r.u8()?)?;

    let start = [
        opt_length_from_flag(r, flags, FLAG_START_X)?,
        opt_length_from_flag(r, flags, FLAG_START_Y)?,
        opt_length_from_flag(r, flags, FLAG_START_Z)?,
    ];
    let end = [
        opt_length_from_flag(r, flags, FLAG_END_X)?,
        opt_length_from_flag(r, flags, FLAG_END_Y)?,
        opt_length_from_flag(r, flags, FLAG_END_Z)?,
    ];
    let width = opt_length_from_flag(r, flags, FLAG_WIDTH)?;
    let height = opt_length_from_flag(r, flags, FLAG_HEIGHT)?;
    let centre = if flags & FLAG_CENTRE == 0 {
        None
    } else {
        Some([Length::mm(r.f64()?), Length::mm(r.f64()?)])
    };

    let speed = Feedrate(r.f64()?);
    let length = Length::mm(r.f64()?);
    let volume = Volume(r.f64()?);
    let filament = Length::mm(r.f64()?);

    let temperature = opt_f64_from_flag(r, flags, FLAG_TEMPERATURE)?;
    let fan = opt_f64_from_flag(r, flags, FLAG_FAN)?;
    let flow = opt_f64_from_flag(r, flags, FLAG_FLOW)?;
    let dwell_s = opt_f64_from_flag(r, flags, FLAG_DWELL)?;
    let manual_gcode = if enc == LEGACY_CHUNKED_ENC_VER {
        None
    } else {
        opt_string_from_flag(r, flags, FLAG_MANUAL_GCODE, limits)?
    };
    let tool = if flags & FLAG_TOOL == 0 {
        None
    } else {
        Some(r.u32()?)
    };
    let orientation = if flags & FLAG_ORIENTATION == 0 {
        None
    } else {
        Some([r.f64()?, r.f64()?, r.f64()?])
    };
    let control_points = if flags & FLAG_CONTROL_POINTS == 0 {
        None
    } else {
        let len = r.u32()? as usize;
        limits.ensure(
            "control point count",
            len,
            limits.max_control_points_per_segment,
        )?;
        let mut points = Vec::with_capacity(len);
        for _ in 0..len {
            points.push([
                Length::mm(r.f64()?),
                Length::mm(r.f64()?),
                Length::mm(r.f64()?),
            ]);
        }
        Some(points)
    };
    let power = opt_f64_from_flag(r, flags, FLAG_POWER)?;

    Ok(Segment {
        start,
        end,
        travel: flags & FLAG_TRAVEL != 0,
        speed,
        length,
        volume,
        filament,
        width,
        height,
        kind,
        centre,
        clockwise: flags & FLAG_CLOCKWISE != 0,
        temperature,
        fan,
        flow,
        tool,
        power,
        dwell_s,
        manual_gcode,
        orientation,
        control_points,
    })
}

pub struct ChunkedSegmentsIterator<R: Read> {
    reader: BufReader<R>,
    remaining: usize,
    block: std::vec::IntoIter<Segment>,
    enc: u8,
    declared_block_size: usize,
    declared_input_bytes: usize,
    checked_eof: bool,
    limits: DecodeLimits,
}

impl<R: Read> ChunkedSegmentsIterator<R> {
    fn read_next_block(&mut self) -> Result<(), CodecError> {
        let block_n = read_u32(&mut self.reader)? as usize;
        if block_n == 0 || block_n > self.remaining {
            return Err(CodecError::Truncated);
        }
        self.limits.ensure(
            "chunk segment count",
            block_n,
            self.declared_block_size.min(self.limits.max_block_segments),
        )?;
        let body_len = read_u32(&mut self.reader)? as usize;
        let compressed_len = read_u32(&mut self.reader)? as usize;
        let declared_input_bytes = self
            .declared_input_bytes
            .checked_add(12)
            .and_then(|bytes| bytes.checked_add(compressed_len))
            .ok_or(CodecError::LimitExceeded {
                field: "input bytes",
                limit: self.limits.max_input_bytes,
                actual: usize::MAX,
            })?;
        self.limits.ensure(
            "input bytes",
            declared_input_bytes,
            self.limits.max_input_bytes,
        )?;
        self.limits
            .ensure("chunk body bytes", body_len, self.limits.max_block_bytes)?;
        self.limits.ensure(
            "compressed chunk bytes",
            compressed_len,
            self.limits.max_block_bytes,
        )?;
        let compressed = read_vec(&mut self.reader, compressed_len)?;
        let body = decompress_exact(&compressed, body_len)?;

        let mut r = Reader::new(&body);
        let mut segments = Vec::with_capacity(block_n);
        for _ in 0..block_n {
            segments.push(decode_segment_row(&mut r, self.enc, &self.limits)?);
        }
        if r.at != body.len() {
            return Err(CodecError::Other(
                "trailing bytes in chunked Dry IR block".to_string(),
            ));
        }

        self.remaining -= block_n;
        self.declared_input_bytes = declared_input_bytes;
        self.block = segments.into_iter();
        Ok(())
    }
}

impl<R: Read> Iterator for ChunkedSegmentsIterator<R> {
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(segment) = self.block.next() {
            return Some(Ok(segment));
        }
        if self.remaining == 0 {
            if self.checked_eof {
                return None;
            }
            self.checked_eof = true;
            let mut trailing = [0u8; 1];
            return match self.reader.read(&mut trailing) {
                Ok(0) => None,
                Ok(_) => Some(Err(CodecError::Other(
                    "trailing bytes after chunked Dry IR blocks".into(),
                ))),
                Err(error) => Some(Err(super::util::read_error(error))),
            };
        }
        match self.read_next_block() {
            Ok(()) => self.block.next().map(Ok),
            Err(error) => {
                self.remaining = 0;
                Some(Err(error))
            }
        }
    }
}

/// Decode a toolpath from the chunked streaming binary form.
pub fn decode_chunked_streaming<R: Read>(
    reader: R,
) -> Result<(u32, Option<Meta>, ChunkedSegmentsIterator<R>), CodecError> {
    decode_chunked_streaming_with_limits(reader, &DecodeLimits::default())
}

/// Decode a chunked toolpath using explicit resource budgets.
pub fn decode_chunked_streaming_with_limits<R: Read>(
    reader: R,
    limits: &DecodeLimits,
) -> Result<(u32, Option<Meta>, ChunkedSegmentsIterator<R>), CodecError> {
    let mut reader = BufReader::new(reader);
    if read_array::<4, _>(&mut reader)? != CHUNKED_MAGIC {
        return Err(CodecError::BadMagic);
    }
    let enc = read_u8(&mut reader)?;
    if enc != CHUNKED_ENC_VER && enc != LEGACY_CHUNKED_ENC_VER {
        return Err(CodecError::UnsupportedVersion(enc));
    }
    let version = read_u32(&mut reader)?;
    let n = read_u32(&mut reader)? as usize;
    limits.ensure("segment count", n, limits.max_segments)?;
    let block_size = read_u32(&mut reader)? as usize;
    if block_size == 0 {
        return Err(CodecError::Other(
            "chunked Dry IR block size cannot be zero".to_string(),
        ));
    }
    limits.ensure(
        "declared chunk segment count",
        block_size,
        limits.max_block_segments,
    )?;
    let (meta, declared_input_bytes) = match read_u8(&mut reader)? {
        0 => (None, 18usize),
        _ => {
            let len = read_u32(&mut reader)? as usize;
            limits.ensure("metadata bytes", len, limits.max_metadata_bytes)?;
            let declared_input_bytes =
                22usize.checked_add(len).ok_or(CodecError::LimitExceeded {
                    field: "input bytes",
                    limit: limits.max_input_bytes,
                    actual: usize::MAX,
                })?;
            limits.ensure("input bytes", declared_input_bytes, limits.max_input_bytes)?;
            let bytes = read_vec(&mut reader, len)?;
            let json = std::str::from_utf8(&bytes).map_err(|_| CodecError::BadUtf8)?;
            (
                Some(serde_json::from_str(json).map_err(|_| CodecError::BadMeta)?),
                declared_input_bytes,
            )
        }
    };
    limits.ensure("input bytes", declared_input_bytes, limits.max_input_bytes)?;

    Ok((
        version,
        meta,
        ChunkedSegmentsIterator {
            reader,
            remaining: n,
            block: Vec::new().into_iter(),
            enc,
            declared_block_size: block_size,
            declared_input_bytes,
            checked_eof: false,
            limits: *limits,
        },
    ))
}
