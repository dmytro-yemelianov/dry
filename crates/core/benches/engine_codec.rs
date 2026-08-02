//! Criterion benchmarks for the hot paths: the three codecs (JSON / `DRY0` / `DRY1`) and the passes
//! (`simulate` / `verify` / `emit` / `trace`). Run locally with `cargo bench -p dry-core`; the CI
//! `bench` job builds these to keep them from bit-rotting (`docs/13-performance-and-scale.md`).

// These exercise the deprecated infallible `emit()` on purpose: it is still the entry point the
// in-tree call sites use, and refusing the whole program is part of what is under test here.
#![allow(deprecated)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use dry_core::{
    emit, simulate, trace_summary, verify, Contracts, EmitParams, Feedrate, Length, Segment,
    SegmentKind, Toolpath, Volume,
};

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
    let json = tp.to_json();
    let dry0 = tp.to_bytes();
    let dry1 = tp.to_streaming_bytes();
    let emit_params = EmitParams::default();
    let contracts = Contracts {
        bounds: Some([[0.0, 250.0], [0.0, 250.0], [0.0, 250.0]]),
        max_flow: Some(25.0),
        ..Contracts::default()
    };

    c.bench_function("encode_json", |b| b.iter(|| black_box(tp.to_json())));
    c.bench_function("encode_dry0", |b| b.iter(|| black_box(tp.to_bytes())));
    c.bench_function("encode_dry1", |b| {
        b.iter(|| black_box(tp.to_streaming_bytes()))
    });
    c.bench_function("decode_json", |b| {
        b.iter(|| black_box(Toolpath::from_json(&json).unwrap()))
    });
    c.bench_function("decode_dry0", |b| {
        b.iter(|| black_box(Toolpath::from_bytes(&dry0).unwrap()))
    });
    c.bench_function("decode_dry1", |b| {
        b.iter(|| black_box(Toolpath::from_bytes(&dry1).unwrap()))
    });
    c.bench_function("simulate", |b| b.iter(|| black_box(simulate(&tp))));
    c.bench_function("verify", |b| b.iter(|| black_box(verify(&tp, &contracts))));
    c.bench_function("emit", |b| b.iter(|| black_box(emit(&tp, &emit_params))));
    c.bench_function("trace", |b| {
        b.iter(|| black_box(trace_summary(&tp, 5.0).unwrap()))
    });
}

criterion_group!(group, benches);
criterion_main!(group);
