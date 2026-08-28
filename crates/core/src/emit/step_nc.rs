//! STEP-NC intent export (prototype).
//!
//! This is a lightweight, deterministic XML sketch of the resolved toolpath. It is intentionally
//! conservative and schema-light: the goal is to provide an explicit, machine-independent intent
//! artifact alongside G-code emission while the CNC/STEP-NC backend is being standardised.

use crate::emit::format_number_checked as checked;
use crate::ir::{SegmentKind, Toolpath};

/// Emit a small STEP-NC-inspired XML intent document from a resolved toolpath.
///
/// # Errors
///
/// Refuses a toolpath carrying a non-finite quantity, for the same reason the g-code emitter does:
/// `format_number` is `format!("{v:.6}")` plus trimming, so NaN/inf leave as the well-formed
/// attribute values `NaN`/`inf` and the document is a machining program a downstream consumer will
/// read as intent. The whole document is refused rather than written with a nonsense value in it.
///
/// This gate is wider than the g-code one: `<toolframe>` is written for every oriented segment,
/// not only under a five-axis model, so a value the g-code path drops still reaches this file.
pub fn emit_step_nc(
    tp: &Toolpath,
    _params: &crate::emit::EmitParams,
) -> Result<String, crate::codec::CodecError> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(
        "<stepnc xmlns=\"urn:iso:std:iso-10303-14649\"\n  xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    out.push_str("  <program name=\"dry-program\">\n");
    out.push_str("    <header schema=\"ISO-10303-238:2020\" standard=\"ISO 14649-10/11\" standard_name=\"STEP-NC AP238\"/>\n");
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
                    checked(secs, "dwell")?
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
                    "      <workingstep id=\"ws-{index}\" type=\"{kind}\">\n"
                ));
                out.push_str(&format!(
                    "        <motion kind=\"{}\" speed=\"{}\">\n",
                    motion,
                    checked(seg.speed.value(), "speed")?
                ));
                out.push_str(&format!(
                    "          <start x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                    checked(start[0], "start x")?,
                    checked(start[1], "start y")?,
                    checked(start[2], "start z")?
                ));
                out.push_str(&format!(
                    "          <end x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                    checked(end[0], "end x")?,
                    checked(end[1], "end y")?,
                    checked(end[2], "end z")?
                ));

                if let Some([i, j, k]) = seg.orientation {
                    out.push_str(&format!(
                        "          <toolframe i=\"{}\" j=\"{}\" k=\"{}\"/>\n",
                        checked(i, "toolframe i")?,
                        checked(j, "toolframe j")?,
                        checked(k, "toolframe k")?
                    ));
                }
                if let Some([cx, cy]) = seg.centre {
                    out.push_str(&format!(
                        "          <arc centre_x=\"{}\" centre_y=\"{}\"/>\n",
                        checked(cx.value(), "arc centre_x")?,
                        checked(cy.value(), "arc centre_y")?
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
    Ok(out)
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
