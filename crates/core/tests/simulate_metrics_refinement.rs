//! Native refinement checks for the exact-rational simulate-metrics fixture model.

use dry_core::{simulate, Feedrate, Length, Segment, SegmentKind, Toolpath, Volume};
use serde::Deserialize;

const FIXTURES: &str = include_str!("../../../proofs/fixtures/simulate-metrics-refinement-v0.json");

#[derive(Debug, Deserialize)]
struct FixtureDocument {
    schema_version: u32,
    model: String,
    model_checks: bool,
    cases: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
struct FixtureCase {
    id: String,
    segments: Vec<FixtureSegment>,
    expected: MetricsExpectation,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NumberOrRational {
    Float(f64),
    Rational { numerator: i64, denominator: u64 },
}

impl NumberOrRational {
    fn to_f64(&self) -> f64 {
        match *self {
            Self::Float(value) => value,
            Self::Rational {
                numerator,
                denominator,
            } => numerator as f64 / denominator as f64,
        }
    }
}

#[derive(Debug, Deserialize)]
struct FixtureSegment {
    travel: bool,
    length: NumberOrRational,
    speed: NumberOrRational,
    volume: NumberOrRational,
    filament: NumberOrRational,
    dwell_seconds: Option<NumberOrRational>,
}

#[derive(Debug, Deserialize)]
struct MetricsExpectation {
    total_time: NumberOrRational,
    print_time: NumberOrRational,
    travel_time: NumberOrRational,
    extruding_distance: NumberOrRational,
    travel_distance: NumberOrRational,
    extruded_volume: NumberOrRational,
    filament_length: NumberOrRational,
    segment_count: u64,
    max_flow_rate: NumberOrRational,
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-12 * expected.abs().max(1.0)
}

fn to_segment(fixture: &FixtureSegment) -> Segment {
    Segment {
        start: [None, None, None],
        end: [None, None, None],
        travel: fixture.travel,
        speed: Feedrate(fixture.speed.to_f64()),
        length: Length(fixture.length.to_f64()),
        volume: Volume(fixture.volume.to_f64()),
        filament: Length(fixture.filament.to_f64()),
        width: None,
        height: None,
        kind: SegmentKind::Line,
        centre: None,
        clockwise: false,
        temperature: None,
        fan: None,
        flow: None,
        tool: None,
        dwell_s: fixture.dwell_seconds.as_ref().map(NumberOrRational::to_f64),
        manual_gcode: None,
        orientation: None,
        control_points: None,
    }
}

#[test]
fn native_simulate_refines_generated_lean_simulate_corpus() {
    let document: FixtureDocument =
        serde_json::from_str(FIXTURES).expect("valid simulate-metrics fixture JSON");
    assert_eq!(document.schema_version, 1);
    assert_eq!(document.model, "simulate-metrics-refinement-v0");
    assert!(document.model_checks);
    assert_eq!(document.cases.len(), 7);

    for fixture in &document.cases {
        let segments: Vec<_> = fixture.segments.iter().map(to_segment).collect();
        let toolpath = Toolpath {
            version: 0,
            meta: None,
            segments,
        };
        let observed = simulate(&toolpath);

        assert!(
            close(observed.total_time_s.0, fixture.expected.total_time.to_f64()),
            "case {} total_time_s mismatch: {} != {}",
            fixture.id,
            observed.total_time_s.0,
            fixture.expected.total_time.to_f64()
        );
        assert!(
            close(observed.print_time_s.0, fixture.expected.print_time.to_f64()),
            "case {} print_time_s mismatch: {} != {}",
            fixture.id,
            observed.print_time_s.0,
            fixture.expected.print_time.to_f64()
        );
        assert!(
            close(observed.travel_time_s.0, fixture.expected.travel_time.to_f64()),
            "case {} travel_time_s mismatch: {} != {}",
            fixture.id,
            observed.travel_time_s.0,
            fixture.expected.travel_time.to_f64()
        );
        assert!(
            close(observed.extruding_distance.0, fixture.expected.extruding_distance.to_f64()),
            "case {} extruding_distance mismatch: {} != {}",
            fixture.id,
            observed.extruding_distance.0,
            fixture.expected.extruding_distance.to_f64()
        );
        assert!(
            close(observed.travel_distance.0, fixture.expected.travel_distance.to_f64()),
            "case {} travel_distance mismatch: {} != {}",
            fixture.id,
            observed.travel_distance.0,
            fixture.expected.travel_distance.to_f64()
        );
        assert!(
            close(observed.extruded_volume.0, fixture.expected.extruded_volume.to_f64()),
            "case {} extruded_volume mismatch: {} != {}",
            fixture.id,
            observed.extruded_volume.0,
            fixture.expected.extruded_volume.to_f64()
        );
        assert!(
            close(observed.filament_length.0, fixture.expected.filament_length.to_f64()),
            "case {} filament_length mismatch: {} != {}",
            fixture.id,
            observed.filament_length.0,
            fixture.expected.filament_length.to_f64()
        );
        assert_eq!(
            observed.segment_count,
            fixture.expected.segment_count,
            "case {} segment_count mismatch: {} != {}",
            fixture.id,
            observed.segment_count,
            fixture.expected.segment_count
        );
        assert!(
            close(observed.max_flow_rate.0, fixture.expected.max_flow_rate.to_f64()),
            "case {} max_flow_rate mismatch: {} != {}",
            fixture.id,
            observed.max_flow_rate.0,
            fixture.expected.max_flow_rate.to_f64()
        );
    }
}
