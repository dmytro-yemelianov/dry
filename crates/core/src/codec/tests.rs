use super::chunked::{encode_chunked_with_block_size, try_encode_chunked_with_block_size};
use super::*;
use crate::SegmentKind;
use std::io::Cursor;

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
fn trailing_binary_bytes_are_rejected() {
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    let mut columnar = encode(&tp);
    columnar.push(0xff);
    assert_eq!(decode(&columnar), Err(CodecError::BadCompression));

    let mut chunked = encode_chunked(&tp);
    chunked.push(0xff);
    assert!(
        matches!(decode(&chunked), Err(CodecError::Other(message)) if message.contains("trailing bytes"))
    );
}

#[test]
fn columnar_decoder_rejects_untrusted_header_lengths_before_allocation() {
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    let encoded = encode(&tp);
    let limits = DecodeLimits {
        max_input_bytes: encoded.len(),
        max_segments: 10,
        max_columnar_body_bytes: 16,
        ..DecodeLimits::default()
    };

    let mut too_many_segments = encoded.clone();
    too_many_segments[9..13].copy_from_slice(&11u32.to_le_bytes());
    assert_eq!(
        decode_with_limits(&too_many_segments, &limits),
        Err(CodecError::LimitExceeded {
            field: "segment count",
            limit: 10,
            actual: 11,
        })
    );

    let mut oversized_body = encoded;
    oversized_body[13..17].copy_from_slice(&17u32.to_le_bytes());
    assert_eq!(
        decode_with_limits(&oversized_body, &limits),
        Err(CodecError::LimitExceeded {
            field: "columnar body bytes",
            limit: 16,
            actual: 17,
        })
    );
}

#[test]
fn chunked_decoder_rejects_untrusted_block_lengths_before_reading_them() {
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    let mut encoded = encode_chunked(&tp);
    encoded[9..13].copy_from_slice(&1u32.to_le_bytes());
    encoded[13..17].copy_from_slice(&1u32.to_le_bytes());
    encoded.extend_from_slice(&1u32.to_le_bytes());
    encoded.extend_from_slice(&65u32.to_le_bytes());
    encoded.extend_from_slice(&0u32.to_le_bytes());

    let limits = DecodeLimits {
        max_block_bytes: 64,
        ..DecodeLimits::default()
    };
    let (_, _, mut iter) =
        decode_chunked_streaming_with_limits(Cursor::new(encoded), &limits).unwrap();
    assert_eq!(
        iter.next(),
        Some(Err(CodecError::LimitExceeded {
            field: "chunk body bytes",
            limit: 64,
            actual: 65,
        }))
    );
}

#[test]
fn chunked_streaming_decoder_caps_total_declared_input() {
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    let mut encoded = encode_chunked(&tp);
    encoded[9..13].copy_from_slice(&1u32.to_le_bytes());
    encoded[13..17].copy_from_slice(&1u32.to_le_bytes());
    encoded.extend_from_slice(&1u32.to_le_bytes());
    encoded.extend_from_slice(&0u32.to_le_bytes());
    encoded.extend_from_slice(&0u32.to_le_bytes());

    let limits = DecodeLimits {
        max_input_bytes: 29,
        ..DecodeLimits::default()
    };
    let (_, _, mut iter) =
        decode_chunked_streaming_with_limits(Cursor::new(encoded), &limits).unwrap();
    assert_eq!(
        iter.next(),
        Some(Err(CodecError::LimitExceeded {
            field: "input bytes",
            limit: 29,
            actual: 30,
        }))
    );
}

#[test]
fn decoder_rejects_oversized_input() {
    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    let encoded = encode(&tp);
    let limits = DecodeLimits {
        max_input_bytes: encoded.len() - 1,
        ..DecodeLimits::default()
    };
    assert_eq!(
        decode_with_limits(&encoded, &limits),
        Err(CodecError::LimitExceeded {
            field: "input bytes",
            limit: encoded.len() - 1,
            actual: encoded.len(),
        })
    );
}

#[test]
#[cfg(target_pointer_width = "64")]
fn checked_u32_lengths_reject_oversize_values() {
    let too_large = u32::MAX as usize + 1;
    assert_eq!(
        super::util::checked_u32_len(too_large, "test"),
        Err(CodecError::TooLarge {
            field: "test",
            len: too_large
        })
    );

    let tp = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    assert_eq!(
        try_encode_chunked_with_block_size(&tp, too_large),
        Err(CodecError::TooLarge {
            field: "chunk block size",
            len: too_large
        })
    );
}

#[test]
fn test_streaming_decoders() {
    use crate::units::{Feedrate, Length, Volume};
    let tp = Toolpath {
        version: 3,
        meta: None,
        segments: vec![
            Segment {
                start: [
                    Some(Length::mm(1.0)),
                    Some(Length::mm(2.0)),
                    Some(Length::mm(3.0)),
                ],
                end: [
                    Some(Length::mm(4.0)),
                    Some(Length::mm(5.0)),
                    Some(Length::mm(6.0)),
                ],
                travel: false,
                speed: Feedrate(1200.0),
                length: Length::mm(5.196),
                volume: Volume(0.62),
                filament: Length::mm(0.2),
                width: Some(Length::mm(0.6)),
                height: Some(Length::mm(0.2)),
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: Some(210.0),
                fan: Some(0.5),
                flow: Some(1.0),
                tool: Some(0),
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            },
            Segment {
                start: [
                    Some(Length::mm(4.0)),
                    Some(Length::mm(5.0)),
                    Some(Length::mm(6.0)),
                ],
                end: [
                    Some(Length::mm(4.0)),
                    Some(Length::mm(5.0)),
                    Some(Length::mm(6.0)),
                ],
                travel: true,
                speed: Feedrate(0.0),
                length: Length::ZERO,
                volume: Volume::ZERO,
                filament: Length::ZERO,
                width: None,
                height: None,
                kind: SegmentKind::Dwell,
                centre: None,
                clockwise: false,
                temperature: None,
                fan: None,
                flow: None,
                tool: None,
                dwell_s: Some(1.5),
                manual_gcode: None,
                orientation: None,
                control_points: None,
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
    let (_version, _meta, iter) = decode_any_streaming(Cursor::new(bytes.clone())).unwrap();
    let decoded_segs: Vec<Segment> = iter.map(|r| r.unwrap()).collect();
    assert_eq!(decoded_segs, tp.segments);

    // Chunked binary streaming roundtrip, forced to two one-segment blocks.
    let chunked = encode_chunked_with_block_size(&tp, 1);
    assert_eq!(&chunked[..4], b"DRY1");
    assert_eq!(decode(&chunked).unwrap(), tp);
    let (version, meta, iter) = decode_chunked_streaming(Cursor::new(chunked.clone())).unwrap();
    assert_eq!(version, 3);
    assert_eq!(meta, None);
    let decoded_segs: Vec<Segment> = iter.map(|r| r.unwrap()).collect();
    assert_eq!(decoded_segs, tp.segments);
    let (_version, _meta, iter) = decode_any_streaming(Cursor::new(chunked)).unwrap();
    let decoded_segs: Vec<Segment> = iter.map(|r| r.unwrap()).collect();
    assert_eq!(decoded_segs, tp.segments);

    // JSON streaming roundtrip
    let json_str = tp.to_json();
    let json_iter = JsonSegmentsIterator::new(json_str.as_bytes());
    let json_segs: Vec<Segment> = json_iter.map(|r| r.unwrap()).collect();
    assert_eq!(json_segs, tp.segments);
}

#[test]
fn json_segment_iterator_uses_the_structural_segments_key() {
    use crate::units::{Feedrate, Length, Volume};

    let segment = Segment {
        start: [
            Some(Length::mm(0.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        end: [
            Some(Length::mm(10.0)),
            Some(Length::mm(0.0)),
            Some(Length::mm(0.2)),
        ],
        travel: false,
        speed: Feedrate(600.0),
        length: Length::mm(10.0),
        volume: Volume(1.0),
        filament: Length::mm(1.0),
        width: Some(Length::mm(0.5)),
        height: Some(Length::mm(0.2)),
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        dwell_s: None,
        manual_gcode: None,
        orientation: None,
        control_points: None,
    };
    let json = serde_json::json!({
        "version": 0,
        "meta": {
            "generator": "segments",
            "units": "mm",
            "invariants": []
        },
        "segments": [segment.clone()]
    })
    .to_string();

    let got: Vec<Segment> = JsonSegmentsIterator::new(json.as_bytes())
        .map(|result| result.unwrap())
        .collect();
    assert_eq!(got, vec![segment]);
}
