use super::chunked::encode_chunked_with_block_size;
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
