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
/// Serialise a toolpath as a 3MF Toolpath-extension document.
///
/// Refuses IR carrying a non-finite quantity. Every attribute was written through `{:.6}`-style
/// formatting, which renders `NaN` and `inf` as literal attribute text — producing a document that
/// is well-formed XML, passes a schema check on shape, and describes a motion no machine can
/// execute. `import_3mf_xml` already refuses non-finite attributes on the way in (H1.2); this is the
/// same rule applied on the way out, so the two halves of the round trip agree about what a valid
/// document is.
pub fn export_3mf_xml(toolpath: &Toolpath) -> Result<String, ThreeMfError> {
    for (idx, seg) in toolpath.segments.iter().enumerate() {
        let check = |name: &str, v: f64| -> Result<(), ThreeMfError> {
            if v.is_finite() {
                Ok(())
            } else {
                Err(ThreeMfError {
                    message: format!("segments[{idx}].{name} is {v}; a 3MF document cannot carry a non-finite value"),
                })
            }
        };
        for (axis, name) in [(0, "end.x"), (1, "end.y"), (2, "end.z")] {
            if let Some(v) = seg.end[axis] {
                check(name, v.0)?;
            }
        }
        check("speed", seg.speed.0)?;
        if let Some(w) = seg.width {
            check("width", w.0)?;
        }
        if let Some(h) = seg.height {
            check("height", h.0)?;
        }
        if let Some(t) = seg.temperature {
            check("temperature", t)?;
        }
        if let Some(f) = seg.fan {
            check("fan", f)?;
        }
        if let Some(o) = seg.orientation {
            for (i, v) in o.iter().enumerate() {
                check(&format!("orientation[{i}]"), *v)?;
            }
        }
    }
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<model xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"\n");
    xml.push_str(
        "       xmlns:tp=\"http://schemas.microsoft.com/3dmanufacturing/toolpath/2022/07\"\n",
    );
    xml.push_str("       unit=\"millimeter\">\n");
    xml.push_str("  <resources/>\n");
    xml.push_str("  <build>\n");
    xml.push_str("    <tp:toolpath>\n");

    // `import_3mf_xml` decides whether a segment moves from the coordinate delta against a running
    // position that starts at the origin, *not* from any length it was told. Mirror that cursor
    // exactly, so the exporter's "does this segment move?" is the same question the importer asks.
    // Keying on `Segment.length` instead disagrees on the first segment of a G-code import, whose
    // start is undefined and whose IR length is therefore zero even though it moves.
    let mut cursor = [0.0_f64; 3];

    for (idx, seg) in toolpath.segments.iter().enumerate() {
        let mut moves = false;
        for (axis, held) in cursor.iter_mut().enumerate() {
            if let Some(end) = seg.end[axis] {
                if end.0 != *held {
                    moves = true;
                }
                *held = end.0;
            }
        }
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
        if let Some(w) = seg.width {
            xml.push_str(&format!(" width=\"{:.4}\"", w.0));
        }
        if let Some(h) = seg.height {
            xml.push_str(&format!(" height=\"{:.4}\"", h.0));
        }
        // A moving segment must carry its feedrate even when that feedrate is zero: the importer
        // refuses motion with no `feedrate` attribute, and a zero-speed moving segment is exactly
        // what the G-code importer preserves for motion before the first `F` (see `gcode/lift.rs`).
        // Writing nothing there would make dry's own export un-importable.
        if seg.speed.0 > 0.0 || moves {
            xml.push_str(&format!(" feedrate=\"{:.1}\"", seg.speed.0));
        }
        if seg.volume.0 > 0.0 {
            xml.push_str(&format!(" volume=\"{:.4}\"", seg.volume.0));
        }
        if let Some(temp) = seg.temperature {
            xml.push_str(&format!(" temp=\"{temp:.1}\""));
        }
        if let Some(fan) = seg.fan {
            xml.push_str(&format!(" fan=\"{fan:.1}\""));
        }
        if let Some([i, j, k]) = seg.orientation {
            xml.push_str(&format!(" i=\"{i:.6}\" j=\"{j:.6}\" k=\"{k:.6}\""));
        }
        if let Some(c) = seg.centre {
            xml.push_str(&format!(
                " cx=\"{:.4}\" cy=\"{:.4}\" cw=\"{}\"",
                c[0].0, c[1].0, seg.clockwise
            ));
        }
        if let Some(dwell) = seg.dwell_s {
            xml.push_str(&format!(" dwell=\"{dwell:.3}\""));
        }
        xml.push_str("/>\n");
    }

    xml.push_str("    </tp:toolpath>\n");
    xml.push_str("  </build>\n");
    xml.push_str("</model>\n");
    Ok(xml)
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
            let x = parse_length_attr(trimmed, "x=")?;
            let y = parse_length_attr(trimmed, "y=")?;
            let z = parse_length_attr(trimmed, "z=")?;
            let width = parse_length_attr(trimmed, "width=")?;
            let height = parse_length_attr(trimmed, "height=")?;
            let feedrate = parse_attr(trimmed, "feedrate=")?;
            if feedrate.is_some_and(|f| f < 0.0) {
                return Err(ThreeMfError {
                    message: "segment feedrate must not be negative".into(),
                });
            }
            let volume = parse_attr(trimmed, "volume=")?
                .map(Volume)
                .unwrap_or(Volume::ZERO);
            let temperature = parse_attr(trimmed, "temp=")?;
            let fan = parse_attr(trimmed, "fan=")?;

            let orientation = match (
                parse_attr(trimmed, "i=")?,
                parse_attr(trimmed, "j=")?,
                parse_attr(trimmed, "k=")?,
            ) {
                (Some(i), Some(j), Some(k)) => Some([i, j, k]),
                _ => None,
            };

            let cx = parse_length_attr(trimmed, "cx=")?;
            let cy = parse_length_attr(trimmed, "cy=")?;
            let centre = match (cx, cy) {
                (Some(c0), Some(c1)) => Some([c0, c1]),
                _ => None,
            };
            let clockwise = trimmed.contains("cw=\"true\"");
            let dwell_s = parse_attr(trimmed, "dwell=")?;

            let kind = if dwell_s.is_some() || trimmed.contains("type=\"dwell\"") {
                SegmentKind::Dwell
            } else if centre.is_some() || trimmed.contains("type=\"arc\"") {
                SegmentKind::Arc
            } else if trimmed.contains("type=\"retract\"") {
                SegmentKind::Retract
            } else if trimmed.contains("type=\"unretract\"") {
                SegmentKind::Unretract
            } else if trimmed.contains("type=\"deposit\"") {
                SegmentKind::Deposit
            } else if trimmed.contains("type=\"manual\"") {
                SegmentKind::ManualGcode
            } else if trimmed.contains("type=\"spline\"") {
                SegmentKind::Spline
            } else {
                SegmentKind::Line
            };

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
            // `parse_length_attr` accepts only finite text, but the squared deltas can still
            // overflow it (`x="1e308"` against an origin of zero), so the distance goes through the
            // checked constructor too — `Length::mm` would only assert, and only in debug builds.
            let squared = dx * dx + dy * dy + dz * dz;
            let Some(length) = Length::try_mm(squared.sqrt()) else {
                return Err(ThreeMfError {
                    message: "segment length is not finite".into(),
                });
            };

            // A segment that moves must say how fast: `export_3mf_xml` writes `feedrate` for one
            // (including `feedrate="0.0"` when the source program never stated a feedrate), and
            // defaulting a *missing* attribute to zero produced a move that contributed no time, no
            // distance and no segment count to `simulate` — an invisible move.
            let speed = match feedrate {
                Some(f) => Feedrate(f),
                None if length > Length::ZERO => {
                    return Err(ThreeMfError {
                        message: "segment has motion but no feedrate attribute".into(),
                    })
                }
                None => Feedrate::ZERO,
            };

            segments.push(Segment {
                start: current_pos,
                end: end_pos,
                travel,
                speed,
                length,
                volume,
                filament: Length::ZERO,
                width,
                height,
                kind,
                centre,
                clockwise,
                temperature,
                fan,
                flow: None,
                tool: None,
                power: None,
                dwell_s,
                manual_gcode: None,
                orientation,
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

fn attr_text<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let pos = line.find(key)?;
    let rest = line[pos + key.len()..].trim_start_matches('"');
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// Read one numeric attribute: `Ok(None)` when it is absent or unparseable, an error when it is
/// present but not finite. `"nan"` and `"inf"` parse as `f64` happily, and the value went straight
/// into the IR.
fn parse_attr(line: &str, key: &str) -> Result<Option<f64>, ThreeMfError> {
    let Some(text) = attr_text(line, key) else {
        return Ok(None);
    };
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(Some(value)),
        Ok(_) => Err(ThreeMfError {
            message: format!("attribute {key}\"{text}\" is not finite"),
        }),
        Err(_) => Ok(None),
    }
}

/// Read one coordinate attribute through the checked [`Length`] constructor — this is the ingress
/// boundary for untrusted XML, so the invariant is enforced where the quantity is *built*.
fn parse_length_attr(line: &str, key: &str) -> Result<Option<Length>, ThreeMfError> {
    let Some(text) = attr_text(line, key) else {
        return Ok(None);
    };
    let Ok(value) = text.parse::<f64>() else {
        return Ok(None);
    };
    match Length::try_mm(value) {
        Some(length) => Ok(Some(length)),
        None => Err(ThreeMfError {
            message: format!("attribute {key}\"{text}\" is not a finite length"),
        }),
    }
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
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            }],
        };

        let xml = export_3mf_xml(&original).unwrap();
        assert!(xml.contains("<tp:segment"));
        assert!(xml.contains("temp=\"215.0\""));

        let imported = import_3mf_xml(&xml).unwrap();
        assert_eq!(imported.segments.len(), 1);
        assert_eq!(imported.segments[0].end[0], Some(Length::mm(10.0)));
        assert_eq!(imported.segments[0].temperature, Some(215.0));
    }
}
