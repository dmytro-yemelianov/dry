//! Criterion benchmark for the verification pass. Run locally with `cargo bench -p drymachina-verify`;
//! the CI `bench` job builds it to keep it from bit-rotting (`docs/13-performance-and-scale.md`).
//!
//! Split out of `dry-core`'s `engine_codec` bench with the code it measures (plan Task 7): that
//! bench moved to `drymachina-kernel`, which cannot call the verifier — layer 2 depends on layer 1 and
//! never the other way round. The fixture is the same 5 000-segment toolpath, repeated rather than
//! shared, because after graduation no crate spans both layers to hold one copy of it.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use drymachina_kernel::{Feedrate, Length, Segment, SegmentKind, Toolpath, Volume};
use drymachina_verify::{verify, Contracts};

fn toolpath(n: usize) -> Toolpath {
    let mut segments = Vec::with_capacity(n);
    for i in 0..n {
        let x = (i % 200) as f64;
        segments.push(Segment {
            start: [
                Some(Length::mm(x)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(x + 1.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            travel: false,
            speed: Feedrate(1500.0),
            length: Length::mm(1.0),
            volume: Volume(0.08),
            filament: Length::mm(0.033),
            width: Some(Length::mm(0.4)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            temperature: None,
            fan: None,
            flow: None,
            tool: None,
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        });
    }
    Toolpath {
        version: 0,
        meta: None,
        segments,
    }
}

fn benches(c: &mut Criterion) {
    let tp = toolpath(5_000);
    // Bounds and a flow ceiling on purpose: the contract-driven rules are the ones whose cost scales
    // with segment count, and a default `Contracts` would leave them all switched off.
    let contracts = Contracts {
        bounds: Some([[0.0, 250.0], [0.0, 250.0], [0.0, 250.0]]),
        max_flow: Some(25.0),
        ..Contracts::default()
    };

    c.bench_function("verify", |b| b.iter(|| black_box(verify(&tp, &contracts))));
}

criterion_group!(group, benches);
criterion_main!(group);
