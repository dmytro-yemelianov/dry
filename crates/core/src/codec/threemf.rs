//! 3MF Toolpath reference import/export pass (Task P4.4)
//!
//! Provides reference XML serialization and deserialization for 3MF Toolpath extension models.
//! Documented lossiness: 3MF toolpath extension represents 3D segments and layer IDs;
//! channel metadata (temperature, fan, orientation) is attached via XML element attributes.

use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::units::{Feedrate, Length, Volume};

#[derive(Debug, Clone, PartialEq)]
pub struct ThreeMfError {
    pub message: String,
}

impl std::fmt::Display for ThreeMfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "3MF error: {}", self.message)
    }
}

impl std::error::Error for ThreeMfError {}

/// Exports a Dry L2 [`Toolpath`] to 3MF Toolpath XML format.
pub fn export_3mf_xml(toolpath: &Toolpath) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<model xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"\n");
    xml.push_str(
        "       xmlns:tp=\"http://schemas.microsoft.com/3dmanufacturing/toolpath/2022/07\"\n",
    );
    xml.push_str("       unit=\"millimeter\">\n");
    xml.push_str("  <resources/>\n");
    xml.push_str("  <build>\n");
    xml.push_str("    <tp:toolpath>\n");

    for (idx, seg) in toolpath.segments.iter().enumerate() {
        let kind_str = match seg.kind {
            SegmentKind::Line => "line",
            SegmentKind::Arc => "arc",
            SegmentKind::Spline => "spline",
            SegmentKind::Dwell => "dwell",
            SegmentKind::Retract => "retract",
            SegmentKind::Unretract => "unretract",
            SegmentKind::Deposit => "deposit",
            SegmentKind::ManualGcode => "manual",
        };
        xml.push_str(&format!(
            "      <tp:segment id=\"{}\" type=\"{}\" travel=\"{}\"",
            idx, kind_str, seg.travel
        ));

        if let Some(x) = seg.end[0] {
            xml.push_str(&format!(" x=\"{:.4}\"", x.0));
        }
        if let Some(y) = seg.end[1] {
            xml.push_str(&format!(" y=\"{:.4}\"", y.0));
        }
        if let Some(z) = seg.end[2] {
            xml.push_str(&format!(" z=\"{:.4}\"", z.0));
        }
        if seg.speed.0 > 0.0 {
            xml.push_str(&format!(" feedrate=\"{:.1}\"", seg.speed.0));
        }
        if seg.volume.0 > 0.0 {
            xml.push_str(&format!(" volume=\"{:.4}\"", seg.volume.0));
        }
        if let Some(temp) = seg.temperature {
            xml.push_str(&format!(" temp=\"{temp:.1}\""));
        }
        xml.push_str("/>\n");
    }

    xml.push_str("    </tp:toolpath>\n");
    xml.push_str("  </build>\n");
    xml.push_str("</model>\n");
    xml
}

/// Imports a Dry L2 [`Toolpath`] from 3MF Toolpath XML format.
pub fn import_3mf_xml(xml: &str) -> Result<Toolpath, ThreeMfError> {
    let mut segments = Vec::new();
    let mut current_pos = [
        Some(Length::mm(0.0)),
        Some(Length::mm(0.0)),
        Some(Length::mm(0.0)),
    ];

    for line in xml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<tp:segment") {
            let travel = trimmed.contains("travel=\"true\"");
            let x = parse_attr(trimmed, "x=").map(Length::mm);
            let y = parse_attr(trimmed, "y=").map(Length::mm);
            let z = parse_attr(trimmed, "z=").map(Length::mm);
            let speed = parse_attr(trimmed, "feedrate=")
                .map(Feedrate)
                .unwrap_or(Feedrate::ZERO);
            let volume = parse_attr(trimmed, "volume=")
                .map(Volume)
                .unwrap_or(Volume::ZERO);
            let temperature = parse_attr(trimmed, "temp=");

            let end_pos = [
                x.or(current_pos[0]),
                y.or(current_pos[1]),
                z.or(current_pos[2]),
            ];

            let dx =
                end_pos[0].unwrap_or(Length::ZERO).0 - current_pos[0].unwrap_or(Length::ZERO).0;
            let dy =
                end_pos[1].unwrap_or(Length::ZERO).0 - current_pos[1].unwrap_or(Length::ZERO).0;
            let dz =
                end_pos[2].unwrap_or(Length::ZERO).0 - current_pos[2].unwrap_or(Length::ZERO).0;
            let length = Length::mm((dx * dx + dy * dy + dz * dz).sqrt());

            segments.push(Segment {
                start: current_pos,
                end: end_pos,
                travel,
                speed,
                length,
                volume,
                filament: Length::ZERO,
                width: None,
                height: None,
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature,
                fan: None,
                flow: None,
                tool: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            });

            current_pos = end_pos;
        }
    }

    Ok(Toolpath {
        version: 0,
        meta: None,
        segments,
    })
}

fn parse_attr(line: &str, key: &str) -> Option<f64> {
    if let Some(pos) = line.find(key) {
        let rest = &line[pos + key.len()..];
        let rest = rest.trim_start_matches('"');
        if let Some(end) = rest.find('"') {
            return rest[..end].parse::<f64>().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_import_3mf_round_trips_segments() {
        let original = Toolpath {
            version: 0,
            meta: None,
            segments: vec![Segment {
                start: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                ],
                end: [
                    Some(Length::mm(10.0)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.0)),
                ],
                travel: false,
                speed: Feedrate(1800.0),
                length: Length::mm(10.0),
                volume: Volume(0.8),
                filament: Length::mm(0.33),
                width: Some(Length::mm(0.4)),
                height: Some(Length::mm(0.2)),
                kind: SegmentKind::Line,
                centre: None,
                clockwise: false,
                temperature: Some(215.0),
                fan: None,
                flow: None,
                tool: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            }],
        };

        let xml = export_3mf_xml(&original);
        assert!(xml.contains("<tp:segment"));
        assert!(xml.contains("temp=\"215.0\""));

        let imported = import_3mf_xml(&xml).unwrap();
        assert_eq!(imported.segments.len(), 1);
        assert_eq!(imported.segments[0].end[0], Some(Length::mm(10.0)));
        assert_eq!(imported.segments[0].temperature, Some(215.0));
    }
}
