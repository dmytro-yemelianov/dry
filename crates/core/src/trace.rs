//! Windowed motion/time-series summaries over Dry IR.

use crate::engine::segment_motion_time;
use crate::ir::{Segment, Toolpath};
use serde::{Deserialize, Serialize};

/// A compact time-series summary of a toolpath.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceSummary {
    /// Requested fixed window duration in seconds.
    pub window_s: f64,
    /// Total number of segments in the source toolpath.
    pub segment_count: usize,
    /// Number of segments that actually move.
    pub moving_segment_count: usize,
    pub total_time_s: f64,
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub dwell_time_s: f64,
    pub extruding_distance_mm: f64,
    pub travel_distance_mm: f64,
    pub extruded_volume_mm3: f64,
    pub filament_mm: f64,
    pub max_feedrate_mm_min: f64,
    pub max_flow_mm3_s: f64,
    pub windows: Vec<TraceWindow>,
}

/// One fixed-duration trace window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceWindow {
    pub index: usize,
    pub start_time_s: f64,
    pub end_time_s: f64,
    /// First segment index touching this window.
    pub segment_start: Option<usize>,
    /// Exclusive segment end index touching this window.
    pub segment_end: Option<usize>,
    /// First original source line touching this window, when the trace came from imported G-code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_start: Option<usize>,
    /// Last original source line touching this window, when the trace came from imported G-code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_line_end: Option<usize>,
    pub duration_s: f64,
    pub print_time_s: f64,
    pub travel_time_s: f64,
    pub dwell_time_s: f64,
    pub extruding_distance_mm: f64,
    pub travel_distance_mm: f64,
    pub extruded_volume_mm3: f64,
    pub filament_mm: f64,
    pub max_feedrate_mm_min: f64,
    pub max_flow_mm3_s: f64,
}

/// A trace configuration error.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceError {
    message: String,
}

impl TraceError {
    fn new(message: impl Into<String>) -> Self {
        TraceError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TraceError {}

#[derive(Debug, Clone, Copy)]
struct SegmentTiming {
    motion_s: f64,
    dwell_s: f64,
    flow_mm3_s: f64,
}

impl TraceSummary {
    fn new(window_s: f64, segment_count: usize) -> Self {
        TraceSummary {
            window_s,
            segment_count,
            moving_segment_count: 0,
            total_time_s: 0.0,
            print_time_s: 0.0,
            travel_time_s: 0.0,
            dwell_time_s: 0.0,
            extruding_distance_mm: 0.0,
            travel_distance_mm: 0.0,
            extruded_volume_mm3: 0.0,
            filament_mm: 0.0,
            max_feedrate_mm_min: 0.0,
            max_flow_mm3_s: 0.0,
            windows: Vec::new(),
        }
    }
}

impl TraceWindow {
    fn new(index: usize, window_s: f64) -> Self {
        TraceWindow {
            index,
            start_time_s: index as f64 * window_s,
            end_time_s: (index + 1) as f64 * window_s,
            segment_start: None,
            segment_end: None,
            source_line_start: None,
            source_line_end: None,
            duration_s: 0.0,
            print_time_s: 0.0,
            travel_time_s: 0.0,
            dwell_time_s: 0.0,
            extruding_distance_mm: 0.0,
            travel_distance_mm: 0.0,
            extruded_volume_mm3: 0.0,
            filament_mm: 0.0,
            max_feedrate_mm_min: 0.0,
            max_flow_mm3_s: 0.0,
        }
    }

    fn touch_segment(&mut self, segment: usize, source_line: Option<usize>) {
        self.segment_start = Some(
            self.segment_start
                .map_or(segment, |current| current.min(segment)),
        );
        self.segment_end = Some(
            self.segment_end
                .map_or(segment + 1, |current| current.max(segment + 1)),
        );
        if let Some(source_line) = source_line {
            self.source_line_start = Some(
                self.source_line_start
                    .map_or(source_line, |current| current.min(source_line)),
            );
            self.source_line_end = Some(
                self.source_line_end
                    .map_or(source_line, |current| current.max(source_line)),
            );
        }
    }
}

fn validate_window(window_s: f64) -> Result<(), TraceError> {
    if !window_s.is_finite() || window_s <= 0.0 {
        return Err(TraceError::new(
            "trace window must be a positive finite number of seconds",
        ));
    }
    Ok(())
}

fn timing(segment: &Segment) -> SegmentTiming {
    let motion_s = segment_motion_time(segment)
        .map(|time| time.value())
        .unwrap_or(0.0);
    let dwell_s = segment.dwell_s.unwrap_or(0.0).max(0.0);
    let flow_mm3_s = if motion_s > 0.0 {
        segment.volume.value() / motion_s
    } else {
        0.0
    };
    SegmentTiming {
        motion_s,
        dwell_s,
        flow_mm3_s,
    }
}

fn ensure_window(windows: &mut Vec<TraceWindow>, index: usize, window_s: f64) -> &mut TraceWindow {
    while windows.len() <= index {
        let next = windows.len();
        windows.push(TraceWindow::new(next, window_s));
    }
    &mut windows[index]
}

fn add_zero_duration_segment(
    summary: &mut TraceSummary,
    segment_index: usize,
    source_line: Option<usize>,
    cursor_s: f64,
) {
    let boundary = (cursor_s / summary.window_s).round();
    let on_boundary = cursor_s > 0.0 && (cursor_s - boundary * summary.window_s).abs() < 1e-12;
    let index = if on_boundary {
        (boundary as usize).saturating_sub(1)
    } else {
        (cursor_s / summary.window_s).floor() as usize
    };
    let window = ensure_window(&mut summary.windows, index, summary.window_s);
    window.touch_segment(segment_index, source_line);
}

fn add_motion_component(
    summary: &mut TraceSummary,
    segment_index: usize,
    source_line: Option<usize>,
    segment: &Segment,
    cursor_s: f64,
    duration_s: f64,
    flow_mm3_s: f64,
) {
    let end_s = cursor_s + duration_s;
    let mut t = cursor_s;
    while t < end_s - 1e-12 {
        let index = (t / summary.window_s).floor() as usize;
        let window_end = ((index + 1) as f64 * summary.window_s).min(end_s);
        let overlap_s = (window_end - t).max(0.0);
        let fraction = overlap_s / duration_s;
        let window = ensure_window(&mut summary.windows, index, summary.window_s);
        window.touch_segment(segment_index, source_line);
        window.duration_s += overlap_s;
        if segment.travel {
            window.travel_time_s += overlap_s;
            window.travel_distance_mm += segment.length.value() * fraction;
        } else {
            window.print_time_s += overlap_s;
            window.extruding_distance_mm += segment.length.value() * fraction;
        }
        window.extruded_volume_mm3 += segment.volume.value() * fraction;
        window.filament_mm += segment.filament.value() * fraction;
        window.max_feedrate_mm_min = window.max_feedrate_mm_min.max(segment.speed.value());
        window.max_flow_mm3_s = window.max_flow_mm3_s.max(flow_mm3_s);
        t = window_end;
    }
}

fn add_dwell_component(
    summary: &mut TraceSummary,
    segment_index: usize,
    source_line: Option<usize>,
    cursor_s: f64,
    duration_s: f64,
) {
    let end_s = cursor_s + duration_s;
    let mut t = cursor_s;
    while t < end_s - 1e-12 {
        let index = (t / summary.window_s).floor() as usize;
        let window_end = ((index + 1) as f64 * summary.window_s).min(end_s);
        let overlap_s = (window_end - t).max(0.0);
        let window = ensure_window(&mut summary.windows, index, summary.window_s);
        window.touch_segment(segment_index, source_line);
        window.duration_s += overlap_s;
        window.dwell_time_s += overlap_s;
        t = window_end;
    }
}

/// Summarize a toolpath into fixed-duration windows.
pub fn trace_summary(tp: &Toolpath, window_s: f64) -> Result<TraceSummary, TraceError> {
    trace_summary_with_sources(tp, window_s, &[])
}

/// Summarize a toolpath into fixed-duration windows, carrying optional source-line numbers per segment.
pub fn trace_summary_with_sources(
    tp: &Toolpath,
    window_s: f64,
    source_lines: &[Option<usize>],
) -> Result<TraceSummary, TraceError> {
    validate_window(window_s)?;
    let mut summary = TraceSummary::new(window_s, tp.segments.len());
    let mut cursor_s = 0.0;

    for (segment_index, segment) in tp.segments.iter().enumerate() {
        let source_line = source_lines.get(segment_index).copied().flatten();
        let timing = timing(segment);
        let duration_s = timing.motion_s + timing.dwell_s;
        if duration_s == 0.0 {
            add_zero_duration_segment(&mut summary, segment_index, source_line, cursor_s);
            continue;
        }

        if timing.motion_s > 0.0 {
            summary.moving_segment_count += 1;
            summary.total_time_s += timing.motion_s;
            if segment.travel {
                summary.travel_time_s += timing.motion_s;
                summary.travel_distance_mm += segment.length.value();
            } else {
                summary.print_time_s += timing.motion_s;
                summary.extruding_distance_mm += segment.length.value();
            }
            summary.extruded_volume_mm3 += segment.volume.value();
            summary.filament_mm += segment.filament.value();
            summary.max_feedrate_mm_min = summary.max_feedrate_mm_min.max(segment.speed.value());
            summary.max_flow_mm3_s = summary.max_flow_mm3_s.max(timing.flow_mm3_s);
            add_motion_component(
                &mut summary,
                segment_index,
                source_line,
                segment,
                cursor_s,
                timing.motion_s,
                timing.flow_mm3_s,
            );
            cursor_s += timing.motion_s;
        }

        if timing.dwell_s > 0.0 {
            summary.total_time_s += timing.dwell_s;
            summary.dwell_time_s += timing.dwell_s;
            add_dwell_component(
                &mut summary,
                segment_index,
                source_line,
                cursor_s,
                timing.dwell_s,
            );
            cursor_s += timing.dwell_s;
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Segment, SegmentKind};
    use crate::units::{Feedrate, Length, Volume};

    fn segment(length: f64, speed: f64, travel: bool, volume: f64) -> Segment {
        Segment {
            start: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(length)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            travel,
            speed: Feedrate(speed),
            length: Length::mm(length),
            volume: Volume(volume),
            filament: Length::mm(volume / 2.4),
            width: Some(Length::mm(0.45)),
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
        }
    }

    #[test]
    fn splits_long_segments_across_windows() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![segment(100.0, 600.0, false, 12.0)],
        };
        let summary = trace_summary_with_sources(&tp, 5.0, &[Some(42)]).unwrap();
        assert_eq!(summary.windows.len(), 2);
        assert!((summary.total_time_s - 10.0).abs() < 1e-12);
        assert!((summary.extruded_volume_mm3 - 12.0).abs() < 1e-12);
        assert!((summary.windows[0].extruded_volume_mm3 - 6.0).abs() < 1e-12);
        assert_eq!(summary.windows[0].source_line_start, Some(42));
        assert_eq!(summary.windows[1].source_line_end, Some(42));
    }

    #[test]
    fn rejects_bad_window_duration() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert!(trace_summary(&tp, 0.0).is_err());
    }

    #[test]
    fn zero_duration_segment_on_boundary_does_not_create_empty_trailing_window() {
        let mut zero = segment(0.0, 600.0, true, 0.0);
        zero.end = zero.start;
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![segment(100.0, 600.0, false, 12.0), zero],
        };

        let summary = trace_summary_with_sources(&tp, 5.0, &[Some(10), Some(11)]).unwrap();
        assert_eq!(summary.windows.len(), 2);
        assert_eq!(summary.windows[1].source_line_end, Some(11));
        assert_eq!(summary.windows[1].segment_end, Some(2));
    }
}
