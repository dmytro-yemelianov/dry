//! Bounded-memory scale gate (`docs/13-performance-and-scale.md`).
//!
//! A counting global allocator records the peak heap *above a baseline* during a measured operation.
//! Streaming a large `DRY1` archive through `simulate_stream` and `verify_stream` must keep that
//! working set **bounded** (independent of segment count), whereas materializing the whole toolpath
//! (`from_bytes` → `simulate`) grows linearly. Asserting the ratio across two sizes is deterministic
//! and catches a regression that accidentally buffers a streaming path.
//!
//! All measurement happens in one `#[test]` so the process-wide allocator counters are not raced by
//! parallel tests.

use dry_core::{
    decode_any_streaming, emit_stream_to_writer, simulate, simulate_stream, verify_stream,
    CodecError, Contracts, EmitParams, Feedrate, Length, Segment, SegmentKind, Toolpath, Volume,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Cursor;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static BASELINE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            let c = CURRENT.fetch_add(l.size(), Relaxed) + l.size();
            PEAK.fetch_max(c, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l);
        CURRENT.fetch_sub(l.size(), Relaxed);
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// Reset the peak high-water mark to the current live bytes (the baseline for the next measurement).
fn reset_peak() {
    let c = CURRENT.load(Relaxed);
    BASELINE.store(c, Relaxed);
    PEAK.store(c, Relaxed);
}

/// Peak heap allocated above the baseline since the last `reset_peak`.
fn peak_delta() -> usize {
    PEAK.load(Relaxed).saturating_sub(BASELINE.load(Relaxed))
}

/// X coordinate at the *start* of segment `i`: a 1 mm-per-step triangle wave over `[0, 200]`.
///
/// These fixtures measure peak working set, so the coordinates only ever needed to stay bounded —
/// which they did with `i % 200`. But that wrapped 200 → 0 between consecutive segments, so the path
/// teleported 200 mm every 200 segments (100 times in the 20k case). Since the emitter writes
/// endpoints only, each wrap is a 200 mm cut straight across the part, and H1.3's `continuity` rule
/// now says so. A serpentine keeps the coordinates just as bounded while describing a path a machine
/// could actually run, and every segment is still exactly 1 mm long.
fn serpentine_x(i: usize) -> f64 {
    let phase = i % 400;
    if phase <= 200 {
        phase as f64
    } else {
        (400 - phase) as f64
    }
}

fn big_toolpath(n: usize) -> Toolpath {
    let mut segments = Vec::with_capacity(n);
    for i in 0..n {
        segments.push(Segment {
            start: [
                Some(Length::mm(serpentine_x(i))),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(serpentine_x(i + 1))),
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

/// Peak working set when streaming a DRY1 archive through `simulate_stream` (input bytes are baseline).
fn streaming_peak(dry1: &[u8]) -> usize {
    let cursor = Cursor::new(dry1.to_vec());
    reset_peak();
    let (_v, _m, iter) = decode_any_streaming(cursor).unwrap();
    let _metrics = simulate_stream(iter).unwrap();
    peak_delta()
}

/// Peak working set when streaming a DRY1 archive through `verify_stream` (input bytes are baseline).
fn verify_streaming_peak(dry1: &[u8], expected_segments: usize) -> usize {
    let cursor = Cursor::new(dry1.to_vec());
    reset_peak();
    let (_v, _m, iter) = decode_any_streaming(cursor).unwrap();
    let report = verify_stream(iter, &Contracts::default()).unwrap();
    assert!(report.ok(), "findings: {:?}", report.findings);
    // The subject of this test is peak working set, not soundness — but `ok()` alone is also true of
    // a pass that inspected nothing, so a decoder that silently yielded zero segments would clear the
    // memory bar trivially. Pin the coverage the `ok()` above is claiming.
    assert_eq!(
        report.segments_inspected, expected_segments,
        "verify_stream did not see every segment"
    );
    peak_delta()
}

/// Peak working set when materializing the whole toolpath, then simulating.
fn materialize_peak(dry1: &[u8]) -> usize {
    let bytes = dry1.to_vec();
    reset_peak();
    let tp = Toolpath::from_bytes(&bytes).unwrap();
    let _metrics = simulate(&tp);
    peak_delta()
}

struct StreamedSegmentSource {
    remaining: usize,
    phase: usize,
}

impl StreamedSegmentSource {
    fn new(segments: usize) -> Self {
        Self {
            remaining: segments,
            phase: 0,
        }
    }
}

impl Iterator for StreamedSegmentSource {
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        let i = self.phase;
        self.phase += 1;
        Some(Ok(Segment {
            start: [
                Some(Length::mm(serpentine_x(i))),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(serpentine_x(i + 1))),
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
        }))
    }
}

#[test]
fn dry1_streaming_is_bounded_memory() {
    let n = 10_000;
    let dry1_n = big_toolpath(n).to_streaming_bytes();
    let dry1_2n = big_toolpath(2 * n).to_streaming_bytes();

    let stream_n = streaming_peak(&dry1_n);
    let stream_2n = streaming_peak(&dry1_2n);
    let verify_n = verify_streaming_peak(&dry1_n, n);
    let verify_2n = verify_streaming_peak(&dry1_2n, 2 * n);
    let mat_n = materialize_peak(&dry1_n);
    let mat_2n = materialize_peak(&dry1_2n);

    eprintln!(
        "simulate stream: {stream_n} -> {stream_2n} bytes; verify stream: {verify_n} -> \
         {verify_2n} bytes; materialize: {mat_n} -> {mat_2n} bytes (n={n})"
    );

    // Streaming working set must not scale with N: doubling segments keeps it within 1.5x.
    assert!(
        stream_2n < stream_n * 3 / 2 + 64 * 1024,
        "DRY1 streaming peak grew with N ({stream_n} -> {stream_2n}); a streaming path is buffering"
    );
    assert!(
        verify_2n < verify_n * 3 / 2 + 64 * 1024,
        "DRY1 verify peak grew with N ({verify_n} -> {verify_2n}); verify_stream is buffering"
    );

    // Materializing must scale roughly linearly: doubling segments grows the peak by >=1.7x.
    assert!(
        mat_2n > mat_n * 17 / 10,
        "materialize peak did not scale with N ({mat_n} -> {mat_2n}); test assumptions are wrong"
    );

    // And streaming must be dramatically smaller than materializing at the larger size.
    assert!(
        stream_2n * 5 < mat_2n,
        "DRY1 streaming ({stream_2n}) is not bounded well below materialization ({mat_2n})"
    );
}

fn simulate_peak_from_stream(segments: usize) -> (dry_core::engine::Metrics, usize) {
    reset_peak();
    let metrics = simulate_stream(StreamedSegmentSource::new(segments)).unwrap();
    (metrics, peak_delta())
}

fn verify_peak_from_stream(segments: usize) -> (bool, usize) {
    reset_peak();
    let report =
        verify_stream(StreamedSegmentSource::new(segments), &Contracts::default()).unwrap();
    (report.ok(), peak_delta())
}

#[derive(Default)]
struct LineCountingWriter {
    lines: usize,
}

impl Write for LineCountingWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.lines += buf.iter().filter(|&&b| b == b'\n').count();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn emit_peak_from_stream(segments: usize) -> (usize, usize) {
    reset_peak();
    let mut writer = LineCountingWriter::default();
    emit_stream_to_writer(
        StreamedSegmentSource::new(segments),
        &EmitParams::default(),
        &mut writer,
    )
    .unwrap();
    // writer.lines counts emitted '\n' separators, so the number of lines is lines + 1.
    (writer.lines + 1, peak_delta())
}

#[test]
fn dry1_streaming_scales_to_one_million_segments() {
    let n = 1_000_000;

    let (metrics, simulate_peak) = simulate_peak_from_stream(n);
    assert_eq!(metrics.segment_count as usize, n);
    assert!(
        (metrics.total_time_s.0 - n as f64 * 0.04).abs() < 1e-6,
        "unexpected total time: {}",
        metrics.total_time_s.0
    );

    // Keep this threshold conservative but stable: it catches regressions that accidentally
    // materialize large intermediate buffers while still allowing small allocator growth.
    assert!(
        simulate_peak < 16 * 1024 * 1024,
        "simulate_stream peak exceeded 16 MiB ({simulate_peak} B)"
    );

    let (report_ok, verify_peak) = verify_peak_from_stream(n);
    assert!(report_ok, "verify_stream reported errors");
    assert!(
        verify_peak < 32 * 1024 * 1024,
        "verify_stream peak exceeded 32 MiB ({verify_peak} B)"
    );

    let (emitted_lines, emit_peak) = emit_peak_from_stream(n);
    assert_eq!(
        emitted_lines, n,
        "emit_stream should preserve segment cardinality"
    );
    assert!(
        emit_peak < 16 * 1024 * 1024,
        "emit_stream peak exceeded 16 MiB ({emit_peak} B)"
    );
}
