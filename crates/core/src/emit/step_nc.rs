//! STEP-NC intent export (prototype).
//!
//! This is a lightweight, deterministic XML sketch of the resolved toolpath. It is intentionally
//! conservative and schema-light: the goal is to provide an explicit, machine-independent intent
//! artifact alongside G-code emission while the CNC/STEP-NC backend is being standardised.

use crate::ir::{SegmentKind, Toolpath};

/// Emit a small STEP-NC-inspired XML intent document from a resolved toolpath.
pub fn emit_step_nc(tp: &Toolpath, _params: &crate::emit::EmitParams) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<stepnc xmlns=\"urn:iso:std:iso-10303-14649\"\n  xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    out.push_str("  <program name=\"dry-program\">\n");
    out.push_str("    <workpiece unit=\"mm\"/>");

    let mut cursor = [0.0; 3];
    out.push_str("\n    <workingsteps>\n");
    for (index, seg) in tp.segments.iter().enumerate() {
        let start = axis_values(seg.start, cursor);
        let end = axis_values(seg.end, start);
        cursor = end;

        match seg.kind {
            SegmentKind::Dwell if seg.dwell_s.is_some() => {
                let secs = seg.dwell_s.unwrap_or_default();
                out.push_str(&format!(
                    "      <workingstep id=\"ws-{index}\" type=\"pause\" duration_s=\"{}\"/>\n",
                    super::format_number(secs)
                ));
            }
            SegmentKind::ManualGcode if seg.manual_gcode.is_some() => {
                out.push_str(&format!(
                    "      <workingstep id=\"ws-{index}\" type=\"manual\">\n        <comment>{}</comment>\n      </workingstep>\n",
                    escape(seg.manual_gcode.as_deref().unwrap_or("").replace('\n', "\\n").as_str())
                ));
            }
            SegmentKind::Line | SegmentKind::Arc => {
                let kind = if seg.travel { "rapid" } else { "motion" };
                let motion = match (seg.kind, seg.centre) {
                    (SegmentKind::Arc, Some(_)) => "arc",
                    _ => "line",
                };

                out.push_str(&format!(
                    "      <workingstep id=\"ws-{index}\" type=\"{}\">\n",
                    kind
                ));
                out.push_str(&format!(
                    "        <motion kind=\"{}\" speed=\"{}\">\n",
                    motion,
                    super::format_number(seg.speed.value())
                ));
                out.push_str(&format!(
                    "          <start x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                    super::format_number(start[0]),
                    super::format_number(start[1]),
                    super::format_number(start[2])
                ));
                out.push_str(&format!(
                    "          <end x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                    super::format_number(end[0]),
                    super::format_number(end[1]),
                    super::format_number(end[2])
                ));

                if let Some([i, j, k]) = seg.orientation {
                    out.push_str(&format!(
                        "          <toolframe i=\"{}\" j=\"{}\" k=\"{}\"/>\n",
                        super::format_number(i),
                        super::format_number(j),
                        super::format_number(k)
                    ));
                }
                if let Some([cx, cy]) = seg.centre {
                    out.push_str(&format!(
                        "          <arc centre_x=\"{}\" centre_y=\"{}\"/>\n",
                        super::format_number(cx.value()),
                        super::format_number(cy.value())
                    ));
                }
                out.push_str("        </motion>\n");
                out.push_str("      </workingstep>\n");
            }
            SegmentKind::Dwell => {
                out.push_str(&format!(
                    "      <workingstep id=\"ws-{index}\" type=\"pause\" duration_s=\"0\"/>\n"
                ));
            }
            _ => {
                out.push_str(&format!(
                    "      <workingstep id=\"ws-{index}\" type=\"unsupported\"/>\n"
                ));
            }
        }
    }
    out.push_str("    </workingsteps>\n");
    out.push_str("  </program>\n");
    out.push_str("</stepnc>\n");
    out
}

fn axis_values(values: [Option<crate::units::Length>; 3], previous: [f64; 3]) -> [f64; 3] {
    let mut current = previous;
    for (i, value) in values.iter().enumerate() {
        if let Some(value) = value {
            current[i] = value.value();
        }
    }
    current
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
