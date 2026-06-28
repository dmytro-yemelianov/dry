use crate::ir::{Segment, SegmentKind};
use crate::resolve::{catmull_rom, dist, SAMPLES};
use crate::units::Length;
use crate::CodecError;
use std::collections::VecDeque;

pub struct SplineFlatteningIterator<I>
where
    I: Iterator<Item = Result<Segment, CodecError>>,
{
    source: I,
    buffer: VecDeque<Segment>,
}

impl<I> SplineFlatteningIterator<I>
where
    I: Iterator<Item = Result<Segment, CodecError>>,
{
    pub fn new(source: I) -> Self {
        Self {
            source,
            buffer: VecDeque::new(),
        }
    }
}

impl<I> Iterator for SplineFlatteningIterator<I>
where
    I: Iterator<Item = Result<Segment, CodecError>>,
{
    type Item = Result<Segment, CodecError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(seg) = self.buffer.pop_front() {
            return Some(Ok(seg));
        }

        match self.source.next()? {
            Err(e) => Some(Err(e)),
            Ok(s) => {
                if s.kind == SegmentKind::Spline {
                    if let Some(ref control_points) = s.control_points {
                        let cur = [
                            s.start[0].unwrap_or(Length::ZERO).value(),
                            s.start[1].unwrap_or(Length::ZERO).value(),
                            s.start[2].unwrap_or(Length::ZERO).value(),
                        ];
                        let mut through: Vec<[f64; 3]> =
                            Vec::with_capacity(control_points.len() + 1);
                        through.push(cur);
                        for pt in control_points {
                            through.push([pt[0].value(), pt[1].value(), pt[2].value()]);
                        }
                        let n = through.len();
                        let mut temp_pos = s.start;
                        for i in 0..n - 1 {
                            let p0 = through[i.saturating_sub(1)];
                            let p1 = through[i];
                            let p2 = through[i + 1];
                            let p3 = through[(i + 2).min(n - 1)];
                            for step in 1..=SAMPLES {
                                let pt = if step == SAMPLES {
                                    p2
                                } else {
                                    catmull_rom(p0, p1, p2, p3, step as f64 / SAMPLES as f64)
                                };
                                let end = [
                                    Some(Length::mm(pt[0])),
                                    Some(Length::mm(pt[1])),
                                    Some(Length::mm(pt[2])),
                                ];
                                let sub_length = dist(temp_pos, end);
                                let ratio = if s.length.value() > 0.0 {
                                    sub_length.value() / s.length.value()
                                } else {
                                    0.0
                                };
                                let sub_volume = s.volume * ratio;
                                let sub_filament = s.filament * ratio;
                                self.buffer.push_back(Segment {
                                    start: temp_pos,
                                    end,
                                    travel: s.travel,
                                    speed: s.speed,
                                    length: sub_length,
                                    volume: sub_volume,
                                    filament: sub_filament,
                                    width: s.width,
                                    height: s.height,
                                    kind: SegmentKind::Line,
                                    centre: None,
                                    clockwise: false,
                                    temperature: s.temperature,
                                    fan: s.fan,
                                    flow: s.flow,
                                    tool: s.tool,
                                    dwell_s: None,
                                    manual_gcode: None,
                                    orientation: s.orientation,
                                    control_points: None,
                                });
                                temp_pos = end;
                            }
                        }
                    }
                    if let Some(seg) = self.buffer.pop_front() {
                        Some(Ok(seg))
                    } else {
                        self.next()
                    }
                } else {
                    Some(Ok(s))
                }
            }
        }
    }
}
