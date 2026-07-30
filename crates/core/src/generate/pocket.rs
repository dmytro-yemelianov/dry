//! `pocket` — contour-parallel CNC pocket/profile generator (P5.3, spec
//! `docs/superpowers/specs/2026-07-30-cnc-pocket-profile-design.md`).
//!
//! Pure L1 sugar like [`super::tpms`]: validated options → `Vec<Op>`; resolve/verify/
//! simulate/emit are inherited unchanged.

use crate::resolve::{Design, Op};

#[derive(Debug, Clone, PartialEq)]
pub enum PocketShape {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    Circle {
        cx: f64,
        cy: f64,
        radius: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CutMode {
    #[default]
    Pocket,
    Profile,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PocketOptions {
    pub shape: PocketShape,
    pub mode: CutMode,
    pub tool_diameter: f64,
    pub stepover: Option<f64>,
    pub depth: f64,
    pub depth_per_pass: Option<f64>,
    pub z_top: Option<f64>,
    pub safe_z: Option<f64>,
    pub cut_feed: Option<f64>,
    pub plunge_feed: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PocketError {
    message: String,
}

impl PocketError {
    fn new(message: impl Into<String>) -> Self {
        PocketError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PocketError {}

#[allow(dead_code)]
struct Resolved {
    tool_r: f64,
    step: f64,
    depth: f64,
    depth_per_pass: f64,
    z_top: f64,
    safe_z: f64,
    cut_feed: f64,
    plunge_feed: f64,
}

fn positive(name: &str, v: f64) -> Result<f64, PocketError> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(PocketError::new(format!(
            "{name} must be finite and > 0, got {v}"
        )))
    }
}

fn finite(name: &str, v: f64) -> Result<f64, PocketError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(PocketError::new(format!("{name} must be finite")))
    }
}

fn validate(o: &PocketOptions) -> Result<Resolved, PocketError> {
    let d = positive("tool_diameter", o.tool_diameter)?;
    let stepover = o.stepover.unwrap_or(0.5);
    if !(stepover.is_finite() && stepover > 0.0 && stepover <= 1.0) {
        return Err(PocketError::new(format!(
            "stepover must be in (0, 1] (fraction of tool_diameter), got {stepover}"
        )));
    }
    let depth = positive("depth", o.depth)?;
    let depth_per_pass = positive("depth_per_pass", o.depth_per_pass.unwrap_or(depth))?;
    let z_top = finite("z_top", o.z_top.unwrap_or(0.0))?;
    let safe_z = finite("safe_z", o.safe_z.unwrap_or(z_top + 5.0))?;
    if safe_z <= z_top {
        return Err(PocketError::new(format!(
            "safe_z ({safe_z}) must be above z_top ({z_top})"
        )));
    }
    let cut_feed = positive("cut_feed", o.cut_feed.unwrap_or(300.0))?;
    let plunge_feed = positive("plunge_feed", o.plunge_feed.unwrap_or(cut_feed / 3.0))?;
    match o.shape {
        PocketShape::Rect {
            x,
            y,
            width,
            height,
        } => {
            finite("x", x)?;
            finite("y", y)?;
            positive("width", width)?;
            positive("height", height)?;
            if d > width || d > height {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) does not fit the {width}x{height} rectangle"
                )));
            }
        }
        PocketShape::Circle { cx, cy, radius } => {
            finite("cx", cx)?;
            finite("cy", cy)?;
            positive("radius", radius)?;
            if d > 2.0 * radius {
                return Err(PocketError::new(format!(
                    "tool_diameter ({d}) does not fit the radius-{radius} circle"
                )));
            }
        }
    }
    Ok(Resolved {
        tool_r: d / 2.0,
        step: stepover * d,
        depth,
        depth_per_pass,
        z_top,
        safe_z,
        cut_feed,
        plunge_feed,
    })
}

/// Generate the L1 ops. Structured failure on invalid options, never a panic.
pub fn try_pocket_ops(o: &PocketOptions) -> Result<Vec<Op>, PocketError> {
    let r = validate(o)?;
    let mut ops = vec![
        Op::Geometry {
            width: Some(o.tool_diameter),
            height: Some(r.depth_per_pass),
        },
        Op::Extruder { on: false },
        Op::Speed { print: r.cut_feed },
    ];
    ops.extend(passes(o, &r)?);
    Ok(ops)
}

// Filled in by the geometry tasks; keeping it separate keeps try_pocket_ops final.
fn passes(_o: &PocketOptions, _r: &Resolved) -> Result<Vec<Op>, PocketError> {
    Ok(Vec::new())
}

/// Panicking convenience over [`try_pocket_ops`]; precondition: valid Dry pocket options.
pub fn pocket_ops(o: &PocketOptions) -> Vec<Op> {
    try_pocket_ops(o).expect("valid Dry pocket options")
}

pub fn try_pocket_design(o: &PocketOptions) -> Result<Design, PocketError> {
    Ok(Design {
        ops: try_pocket_ops(o)?,
    })
}

pub fn pocket_design(o: &PocketOptions) -> Design {
    Design { ops: pocket_ops(o) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect_opts() -> PocketOptions {
        PocketOptions {
            shape: PocketShape::Rect {
                x: 0.0,
                y: 0.0,
                width: 60.0,
                height: 40.0,
            },
            mode: CutMode::Pocket,
            tool_diameter: 6.0,
            stepover: None,
            depth: 5.0,
            depth_per_pass: None,
            z_top: None,
            safe_z: None,
            cut_feed: None,
            plunge_feed: None,
        }
    }

    #[test]
    fn defaults_resolve() {
        let r = validate(&rect_opts()).unwrap();
        assert_eq!(r.tool_r, 3.0);
        assert_eq!(r.step, 3.0); // 0.5 * tool_diameter
        assert_eq!(r.depth_per_pass, 5.0); // defaults to depth (single pass)
        assert_eq!(r.z_top, 0.0);
        assert_eq!(r.safe_z, 5.0); // z_top + 5
        assert_eq!(r.cut_feed, 300.0);
        assert_eq!(r.plunge_feed, 100.0); // cut_feed / 3
    }

    #[test]
    fn tool_larger_than_pocket_is_rejected() {
        let mut o = rect_opts();
        o.tool_diameter = 41.0; // > height
        let err = try_pocket_ops(&o).unwrap_err();
        assert!(err.to_string().contains("tool_diameter"), "{err}");
    }

    #[test]
    fn stepover_out_of_range_is_rejected() {
        let mut o = rect_opts();
        o.stepover = Some(1.5);
        assert!(validate(&o).is_err());
        o.stepover = Some(0.0);
        assert!(validate(&o).is_err());
    }

    #[test]
    fn non_finite_and_non_positive_inputs_are_rejected() {
        for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            let mut o = rect_opts();
            o.depth = bad;
            assert!(validate(&o).is_err(), "depth {bad} must be rejected");
            let mut o = rect_opts();
            o.tool_diameter = bad;
            assert!(
                validate(&o).is_err(),
                "tool_diameter {bad} must be rejected"
            );
        }
    }

    #[test]
    fn safe_z_below_z_top_is_rejected() {
        let mut o = rect_opts();
        o.safe_z = Some(-1.0);
        assert!(validate(&o).is_err());
    }
}
