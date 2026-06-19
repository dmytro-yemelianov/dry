//! `verify` — check a resolved [`Toolpath`] against machine-safety **contracts** and structural
//! invariants, returning a located [`Report`] (`docs/01-architecture.md` §7). This is where Dry stops
//! merely *compiling* a toolpath and starts *catching* unsafe ones.
//!
//! The contracts are Dry's own, clean-room (each is a well-specified property of a safe toolpath, not a
//! reproduction of any oracle's wording):
//!  - **structural** (always checked): every quantity is finite; a travel deposits no material; an
//!    extruding move has a positive bead (`width`,`height` > 0).
//!  - **contract-driven** (checked when the contract supplies a limit): the move stays inside the build
//!    **bounds**; the volumetric **flow** stays under a ceiling; the feedrate stays within a **speed**
//!    range; **Z is monotonic** (non-decreasing) when required (e.g. vase mode).

use crate::ir::{Segment, Toolpath};
use crate::units::Length;
use serde::Serialize;

/// The limits a toolpath is checked against. An unset (`None`/`false`) field disables that check.
#[derive(Debug, Clone, Default)]
pub struct Contracts {
    /// Build volume as `[[x_lo, x_hi], [y_lo, y_hi], [z_lo, z_hi]]` (mm).
    pub bounds: Option<[[f64; 2]; 3]>,
    /// Maximum volumetric flow rate (mm³/s).
    pub max_flow: Option<f64>,
    /// Allowed feedrate range `[min, max]` (mm/min) for extruding moves.
    pub speed_range: Option<[f64; 2]>,
    /// Require Z never to decrease along the path.
    pub monotonic_z: bool,
    /// Minimum nozzle temperature (°C) required to extrude (cold-extrusion guard).
    pub min_temp: Option<f64>,
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The toolpath is unsafe / invalid.
    Error,
    /// Suspicious but not necessarily fatal.
    Warning,
}

/// One located issue found by [`verify`].
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// A stable kebab-case rule id (`bounds`, `max-flow`, `speed`, `monotonic-z`, `cold-extrusion`,
    /// `finite`, `travel-extrudes`, `bead`, `orientation-not-unit`).
    pub rule: String,
    pub severity: Severity,
    /// The offending segment index, if the finding is local to one move.
    pub segment: Option<usize>,
    /// A human-readable description.
    pub message: String,
}

/// The result of verifying a toolpath.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
}

impl Report {
    /// True when there are no `Error`-severity findings.
    pub fn ok(&self) -> bool {
        !self.findings.iter().any(|f| f.severity == Severity::Error)
    }
    /// The number of `Error`-severity findings.
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }
}

/// Per-segment volumetric flow (mm³/s), or `None` for a zero-length or zero-speed move.
fn flow(s: &Segment) -> Option<f64> {
    if s.length > Length::ZERO && s.speed.value() != 0.0 {
        Some((s.volume / (s.length / s.speed)).value())
    } else {
        None
    }
}

/// Verify a toolpath against the contracts, returning all findings (structural + contract-driven).
pub fn verify(tp: &Toolpath, c: &Contracts) -> Report {
    let mut r = Report::default();
    let mut push = |rule: &str, severity, segment, message: String| {
        r.findings.push(Finding {
            rule: rule.to_string(),
            severity,
            segment,
            message,
        });
    };
    let axis = ['X', 'Y', 'Z'];

    for (i, s) in tp.segments.iter().enumerate() {
        // --- structural invariants (always on) ---
        let mut nums = vec![
            s.speed.value(),
            s.length.value(),
            s.volume.value(),
            s.filament.value(),
        ];
        nums.extend(s.end.iter().flatten().map(|v| v.value()));
        if nums.iter().any(|v| !v.is_finite()) {
            push(
                "finite",
                Severity::Error,
                Some(i),
                "segment carries a non-finite value".into(),
            );
        }
        if s.travel && s.volume.value() > 0.0 {
            push(
                "travel-extrudes",
                Severity::Error,
                Some(i),
                format!(
                    "travel move deposits {:.4} mm³ (should be 0)",
                    s.volume.value()
                ),
            );
        }
        if !s.travel && s.length > Length::ZERO {
            let w = s.width.map(|l| l.value()).unwrap_or(0.0);
            let h = s.height.map(|l| l.value()).unwrap_or(0.0);
            if w <= 0.0 || h <= 0.0 {
                push(
                    "bead",
                    Severity::Error,
                    Some(i),
                    format!("extruding move has a non-positive bead (width {w}, height {h})"),
                );
            }
        }
        if let Some([x, y, z]) = s.orientation {
            // the toolframe orientation must be a unit direction vector.
            let mag = (x * x + y * y + z * z).sqrt();
            if (mag - 1.0).abs() > 1e-6 {
                push(
                    "orientation-not-unit",
                    Severity::Error,
                    Some(i),
                    format!(
                        "toolframe orientation [{x}, {y}, {z}] has magnitude {mag} (must be 1)"
                    ),
                );
            }
        }

        // --- contract-driven checks ---
        if let Some(b) = c.bounds {
            for (k, end) in s.end.iter().enumerate() {
                if let Some(v) = end {
                    let v = v.value();
                    if v < b[k][0] || v > b[k][1] {
                        push(
                            "bounds",
                            Severity::Error,
                            Some(i),
                            format!(
                                "{} = {v} is outside the build volume [{}, {}]",
                                axis[k], b[k][0], b[k][1]
                            ),
                        );
                        break; // one bounds finding per segment
                    }
                }
            }
        }
        if let (Some(max), Some(f)) = (c.max_flow, flow(s)) {
            if f > max {
                push(
                    "max-flow",
                    Severity::Error,
                    Some(i),
                    format!("flow {f:.3} mm³/s exceeds the ceiling {max:.3}"),
                );
            }
        }
        if let Some([lo, hi]) = c.speed_range {
            if !s.travel {
                let v = s.speed.value();
                if v < lo || v > hi {
                    push(
                        "speed",
                        Severity::Error,
                        Some(i),
                        format!("feedrate {v} is outside [{lo}, {hi}] mm/min"),
                    );
                }
            }
        }
        if c.monotonic_z {
            if let (Some(z0), Some(z1)) = (s.start[2], s.end[2]) {
                if z1 < z0 {
                    push(
                        "monotonic-z",
                        Severity::Error,
                        Some(i),
                        format!("Z decreases from {} to {}", z0.value(), z1.value()),
                    );
                }
            }
        }
        if let Some(min) = c.min_temp {
            // an extruding move below the minimum nozzle temperature (or with none set) is cold extrusion.
            if !s.travel && s.volume.value() > 0.0 && s.temperature.map(|t| t < min).unwrap_or(true)
            {
                let got = s
                    .temperature
                    .map(|t| format!("{t}"))
                    .unwrap_or_else(|| "unset".into());
                push(
                    "cold-extrusion",
                    Severity::Error,
                    Some(i),
                    format!("extruding at nozzle temperature {got} (< {min} °C)"),
                );
            }
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toolpath_is_ok() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        assert!(verify(&tp, &Contracts::default()).ok());
    }
}
