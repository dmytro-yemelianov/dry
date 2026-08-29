use dry_core::reverse::reverse;
use dry_core::{resolve_checked, Design, Op, ResolveParams};

#[test]
fn test_reverse_engineering_travel_and_print_sequences() {
    let original = Design {
        ops: vec![
            Op::Speed { print: 1800.0 },
            Op::Extruder { on: false },
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(5.0),
            },
            Op::Move {
                x: Some(10.0),
                y: Some(10.0),
                z: Some(0.2),
            },
            Op::Extruder { on: true },
            Op::Speed { print: 900.0 },
            Op::Move {
                x: Some(30.0),
                y: Some(10.0),
                z: Some(0.2),
            },
            Op::Move {
                x: Some(30.0),
                y: Some(30.0),
                z: Some(0.2),
            },
            Op::Extruder { on: false },
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(10.0),
            },
        ],
    };

    let toolpath = resolve_checked(&original, &ResolveParams::default()).unwrap();
    let reversed_design = reverse(&toolpath).expect("Reverse engineering must succeed");

    assert!(!reversed_design.ops.is_empty());

    let re_resolved = resolve_checked(&reversed_design, &ResolveParams::default()).unwrap();
    assert_eq!(toolpath.segments.len(), re_resolved.segments.len());

    for (s1, s2) in toolpath.segments.iter().zip(&re_resolved.segments) {
        assert_eq!(s1.end, s2.end);
        assert_eq!(s1.travel, s2.travel);
        assert_eq!(s1.speed, s2.speed);
    }
}

#[test]
fn test_reverse_engineering_arcs_and_channels() {
    let original = Design {
        ops: vec![
            Op::Temperature { nozzle: 215.0 },
            Op::Fan { speed: 0.8 },
            Op::Flow { ratio: 1.05 },
            Op::Power { level: 850.0 },
            Op::Orient {
                i: 0.0,
                j: 0.0,
                k: 1.0,
            },
            Op::Move {
                x: Some(0.0),
                y: Some(0.0),
                z: Some(0.0),
            },
            Op::Arc {
                cx: 10.0,
                cy: 0.0,
                x: Some(20.0),
                y: Some(0.0),
                z: Some(0.0),
                clockwise: false,
            },
            Op::Dwell { seconds: 1.5 },
        ],
    };

    let toolpath = resolve_checked(&original, &ResolveParams::default()).unwrap();
    let reversed = reverse(&toolpath).expect("Failed reversing arc & channel design");
    let re_resolved = resolve_checked(&reversed, &ResolveParams::default()).unwrap();

    assert_eq!(toolpath.segments.len(), re_resolved.segments.len());
    for (s1, s2) in toolpath.segments.iter().zip(&re_resolved.segments) {
        assert_eq!(s1.kind, s2.kind);
        assert_eq!(s1.end, s2.end);
        assert_eq!(s1.temperature, s2.temperature);
        assert_eq!(s1.fan, s2.fan);
        assert_eq!(s1.power, s2.power);
        assert_eq!(s1.orientation, s2.orientation);
    }
}
