use super::util::{checked_u32_len, decompress_exact, Reader};
use super::{CodecError, DecodeLimits, ENC_VER, LEGACY_ENC_VER, MAGIC, POWER_ENC_VER};
use crate::ir::{Meta, Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};

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

fn push_opt_string_col(
    out: &mut Vec<u8>,
    segs: &[Segment],
    get: impl Fn(&Segment) -> Option<&str>,
) -> Result<(), CodecError> {
    push_bits(out, segs.len(), |i| get(&segs[i]).is_some());
    for s in segs {
        if let Some(value) = get(s) {
            body_push_string(out, value)?;
        }
    }
    Ok(())
}

fn body_push_string(out: &mut Vec<u8>, value: &str) -> Result<(), CodecError> {
    out.extend_from_slice(&checked_u32_len(value.len(), "string")?.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

/// Encode a toolpath to the compact columnar binary form.
///
/// The header's `enc_ver` records the *minimum reader version* the body needs, so it is chosen from
/// the content rather than fixed. A `DRY0` column is dense — `ceil(n/8)` validity bits plus `n×f64`
/// of payload whether or not a single segment fills it — so unconditionally writing the `power`
/// column would move the bytes of every archive ever produced. Emitting [`ENC_VER`] when no segment
/// carries power keeps those archives reproducible; [`POWER_ENC_VER`] appears only once the column
/// has something to say.
pub fn try_encode(tp: &Toolpath) -> Result<Vec<u8>, CodecError> {
    let segs = &tp.segments;
    let n = segs.len();
    let n_u32 = checked_u32_len(n, "segment count")?;
    let enc_ver = if segs.iter().any(|s| s.power.is_some()) {
        POWER_ENC_VER
    } else {
        ENC_VER
    };

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

    // Encode control points
    push_bits(&mut body, n, |i| segs[i].control_points.is_some());
    for s in segs {
        if let Some(ref points) = s.control_points {
            body.extend_from_slice(
                &checked_u32_len(points.len(), "control point count")?.to_le_bytes(),
            );
            for pt in points {
                body.extend_from_slice(&pt[0].value().to_le_bytes());
                body.extend_from_slice(&pt[1].value().to_le_bytes());
                body.extend_from_slice(&pt[2].value().to_le_bytes());
            }
        }
    }
    push_opt_string_col(&mut body, segs, |s| s.manual_gcode.as_deref())?;
    // New columns append after the existing ones, so an older layout is a prefix of a newer one.
    if enc_ver >= POWER_ENC_VER {
        push_opt_col(&mut body, segs, |s| s.power);
    }

    // kind dictionary (line/arc repeat, so this is tiny) + per-segment u32 index.
    let mut dict: Vec<SegmentKind> = Vec::new();
    let mut idx: Vec<u32> = Vec::with_capacity(n);
    for s in segs {
        let pos = dict.iter().position(|k| *k == s.kind).unwrap_or_else(|| {
            dict.push(s.kind);
            dict.len() - 1
        });
        idx.push(checked_u32_len(pos, "kind dictionary index")?);
    }
    body.extend_from_slice(&checked_u32_len(dict.len(), "kind dictionary length")?.to_le_bytes());
    for k in &dict {
        let k = k.as_str();
        body.extend_from_slice(&checked_u32_len(k.len(), "kind string")?.to_le_bytes());
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
            body.extend_from_slice(&checked_u32_len(json.len(), "meta JSON")?.to_le_bytes());
            body.extend_from_slice(json.as_bytes());
        }
    }

    let compressed = miniz_oxide::deflate::compress_to_vec(&body, 8);
    let mut out = Vec::with_capacity(17 + compressed.len());
    out.extend_from_slice(&MAGIC);
    out.push(enc_ver);
    out.extend_from_slice(&tp.version.to_le_bytes());
    out.extend_from_slice(&n_u32.to_le_bytes());
    out.extend_from_slice(&checked_u32_len(body.len(), "body length")?.to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// Encode a toolpath to the compact columnar binary form.
pub fn encode(tp: &Toolpath) -> Vec<u8> {
    try_encode(tp).expect("Dry columnar binary encode failed")
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
    pub manual_gcode: Vec<Option<String>>,
    pub power: Vec<Option<f64>>,
    pub tool: Vec<Option<u32>>,
    pub orientation: Vec<Option<[f64; 3]>>,
    pub control_points: Vec<Option<Vec<[Length; 3]>>>,
    pub dict: Vec<SegmentKind>,
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
            Some(k) => *k,
            None => return Some(Err(CodecError::Truncated)),
        };

        let opt_len = |c: &[Option<f64>], idx: usize| c[idx].map(Length::mm);
        let centre = match (self.cx[i], self.cy[i]) {
            (Some(a), Some(b)) => Some([Length::mm(a), Length::mm(b)]),
            _ => None,
        };

        Some(Ok(Segment {
            start: [
                opt_len(&self.sx, i),
                opt_len(&self.sy, i),
                opt_len(&self.sz, i),
            ],
            end: [
                opt_len(&self.ex, i),
                opt_len(&self.ey, i),
                opt_len(&self.ez, i),
            ],
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
            power: self.power[i],
            dwell_s: self.dwell_s[i],
            manual_gcode: self.manual_gcode[i].clone(),
            orientation: self.orientation[i],
            control_points: self.control_points[i].clone(),
        }))
    }
}

/// Decode a toolpath from the columnar binary form streamingly.
pub fn decode_streaming(
    buf: &[u8],
) -> Result<(u32, Option<Meta>, BinarySegmentsIterator), CodecError> {
    decode_streaming_with_limits(buf, &DecodeLimits::default())
}

/// Decode a columnar toolpath using explicit resource budgets.
pub fn decode_streaming_with_limits(
    buf: &[u8],
    limits: &DecodeLimits,
) -> Result<(u32, Option<Meta>, BinarySegmentsIterator), CodecError> {
    limits.ensure("input bytes", buf.len(), limits.max_input_bytes)?;
    let mut h = Reader::new(buf);
    if h.take(4)? != MAGIC {
        return Err(CodecError::BadMagic);
    }
    let enc = h.u8()?;
    if enc != ENC_VER && enc != LEGACY_ENC_VER && enc != POWER_ENC_VER {
        return Err(CodecError::UnsupportedVersion(enc));
    }
    let version = h.u32()?;
    let n = h.u32()? as usize;
    let body_len = h.u32()? as usize;
    limits.ensure("segment count", n, limits.max_segments)?;
    limits.ensure(
        "columnar body bytes",
        body_len,
        limits.max_columnar_body_bytes,
    )?;

    // Fixed bytes per segment and validity bitmaps, per layout: enc 0 has no manual_gcode bitmap,
    // enc 2 adds the power column (one bitmap + one f64 per segment).
    let (fixed_per_segment, bitmap_columns) = match enc {
        LEGACY_ENC_VER => (176, 19),
        ENC_VER => (176, 20),
        // POWER_ENC_VER — every other value was rejected above.
        _ => (184, 21),
    };
    let minimum_body_len = n
        .checked_mul(fixed_per_segment)
        .and_then(|fixed| {
            n.div_ceil(8)
                .checked_mul(bitmap_columns)
                .and_then(|bitmaps| fixed.checked_add(bitmaps))
        })
        .and_then(|size| size.checked_add(5))
        .ok_or(CodecError::LimitExceeded {
            field: "minimum columnar body bytes",
            limit: limits.max_columnar_body_bytes,
            actual: usize::MAX,
        })?;
    if body_len < minimum_body_len {
        return Err(CodecError::Truncated);
    }

    let body = decompress_exact(&buf[h.at..], body_len)?;
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

    let control_points_valid = r.bits(n)?;
    let mut control_points = Vec::with_capacity(n);
    for v in control_points_valid {
        if v {
            let len = r.u32()? as usize;
            limits.ensure(
                "control point count",
                len,
                limits.max_control_points_per_segment,
            )?;
            let mut points = Vec::with_capacity(len);
            for _ in 0..len {
                let pt = [
                    Length::mm(r.f64()?),
                    Length::mm(r.f64()?),
                    Length::mm(r.f64()?),
                ];
                points.push(pt);
            }
            control_points.push(Some(points));
        } else {
            control_points.push(None);
        }
    }
    let manual_gcode = if enc == LEGACY_ENC_VER {
        vec![None; n]
    } else {
        let valid = r.bits(n)?;
        let mut values = Vec::with_capacity(n);
        for v in valid {
            if v {
                let len = r.u32()? as usize;
                limits.ensure("manual gcode bytes", len, limits.max_string_bytes)?;
                let value = std::str::from_utf8(r.take(len)?).map_err(|_| CodecError::BadUtf8)?;
                values.push(Some(value.to_string()));
            } else {
                values.push(None);
            }
        }
        values
    };
    let power = if enc == POWER_ENC_VER {
        r.opt_col(n)?
    } else {
        vec![None; n]
    };

    let dict_len = r.u32()? as usize;
    limits.ensure("kind dictionary entries", dict_len, 32)?;
    let mut dict: Vec<SegmentKind> = Vec::with_capacity(dict_len);
    for _ in 0..dict_len {
        let len = r.u32()? as usize;
        limits.ensure("kind string bytes", len, limits.max_string_bytes)?;
        let s = std::str::from_utf8(r.take(len)?).map_err(|_| CodecError::BadUtf8)?;
        let kind = SegmentKind::from_wire(s).ok_or_else(|| CodecError::BadKind(s.to_string()))?;
        dict.push(kind);
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
            limits.ensure("metadata bytes", len, limits.max_metadata_bytes)?;
            let json = std::str::from_utf8(r.take(len)?).map_err(|_| CodecError::BadUtf8)?;
            Some(serde_json::from_str(json).map_err(|_| CodecError::BadMeta)?)
        }
    };
    if r.at != body.len() {
        return Err(CodecError::Other(
            "trailing bytes in columnar Dry IR body".to_string(),
        ));
    }

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
        manual_gcode,
        power,
        tool,
        orientation,
        control_points,
        dict,
        kind_indices,
    };

    Ok((version, meta, iter))
}
