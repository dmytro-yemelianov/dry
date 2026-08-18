use dry_core::{decode_dry2, encode_dry2, resolve, Design, Op, ResolveParams, DRY2_MAGIC};

#[test]
fn test_dry2_delta_codec_round_trip() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(10.0),
        y: Some(20.0),
        z: Some(0.2),
    });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Speed { print: 1800.0 });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(20.0),
        z: Some(0.2),
    });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(60.0),
        z: Some(0.4),
    });

    let toolpath = resolve(&design, &ResolveParams::default());

    let encoded = encode_dry2(&toolpath);
    assert_eq!(&encoded[0..4], DRY2_MAGIC);
    assert!(encoded.len() > 12);

    let decoded = decode_dry2(&encoded).expect("must decode DRY2");
    assert_eq!(decoded.segments.len(), toolpath.segments.len());

    for (orig, dec) in toolpath.segments.iter().zip(decoded.segments.iter()) {
        assert_eq!(orig.travel, dec.travel);
        assert!((orig.end[0].unwrap().value() - dec.end[0].unwrap().value()).abs() < 1e-3);
        assert!((orig.end[1].unwrap().value() - dec.end[1].unwrap().value()).abs() < 1e-3);
        assert!((orig.end[2].unwrap().value() - dec.end[2].unwrap().value()).abs() < 1e-3);
        assert!((orig.speed.value() - dec.speed.value()).abs() < 1e-1);
    }
}
