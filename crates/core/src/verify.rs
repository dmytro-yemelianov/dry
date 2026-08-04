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

use crate::emit::RotaryState;
use crate::engine::segment_motion_time;
use crate::ir::{Segment, SegmentKind, Toolpath};
use crate::optimize::{get_tangents, junction_cos_half_angle, junction_velocity_limit_mm_s};
use crate::resolve::{catmull_rom, SAMPLES};
use crate::units::Length;
use serde::{Deserialize, Serialize};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

#[path = "verify/collision.rs"]
pub mod collision;
pub use self::collision::{
    check_tool_holder_collision, CollisionFinding, ToolHolder, ToolHolderSection,
};

/// The limits a toolpath is checked against. An unset (`None`/`false`) field disables that check.
///
/// This is `Serialize` as well as `Deserialize` because [`Report`] echoes the contracts it ran under
/// (§3.5 of the H1.3 design): "clean" is not a claim until you can see what it was clean *against*.
/// `None` fields are skipped rather than written as `null`, so a default `Contracts` echoes as
/// `{"monotonic_z": false}` and the `conformance/reports/*/verify.json` byte-goldens stay compact.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Contracts {
    /// Build volume as `[[x_lo, x_hi], [y_lo, y_hi], [z_lo, z_hi]]` (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[[f64; 2]; 3]>,
    /// Maximum volumetric flow rate (mm³/s).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_flow: Option<f64>,
    /// Allowed feedrate range `[min, max]` (mm/min) for extruding moves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_range: Option<[f64; 2]>,
    /// Require Z never to decrease along the path.
    #[serde(default)]
    pub monotonic_z: bool,
    /// Minimum nozzle temperature (°C) required to extrude (cold-extrusion guard).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_temp: Option<f64>,
    /// Maximum retraction distance (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retraction_distance: Option<f64>,
    /// Maximum retraction speed (mm/min).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retraction_speed: Option<f64>,
    /// Maximum travel run distance without a retraction (mm).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_travel_without_retract: Option<f64>,
    /// Allowed Z height range `[min, max]` (mm) for the first layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_layer_height_range: Option<[f64; 2]>,
    /// Allowed speed range `[min, max]` (mm/min) for the first layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_layer_speed_range: Option<[f64; 2]>,
    /// Relative tolerance for the `bead-volume` rule (`volume ≈ length·width·height·flow`).
    ///
    /// Contract-gated rather than always-on because two `optimize` passes violate the identity by
    /// design: `coasting` zeroes `volume` on the tail of an extrusion run while keeping the bead, and
    /// `arc_fit` sets `length` to the arc while summing chord volumes. Imported IR takes `volume` from
    /// `E` while `width`/`height` come from a user-supplied constant, so it breaks in both directions
    /// on real slicer output too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bead_volume_tolerance: Option<f64>,
    /// Kinematic limits for the peak-acceleration / junction-velocity rules. `None` disables them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinematics: Option<KinematicContracts>,
    /// Rotary-axis limits and reachable workspace for the rotary-travel / rotary-feed /
    /// orientation-reachability rules. `None` disables all three.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotary: Option<RotaryContracts>,
    /// Require the spindle/laser power channel to read `0` on every travel segment
    /// (`laser-power-during-travel`).
    ///
    /// Contract-gated rather than always-on, by the same test `RotaryContracts` states: it is a
    /// property of the **process**, not of the IR. `Op::Power` is one channel with two meanings —
    /// "the target controller's `S` word value: RPM for a spindle, PWM counts for a laser" — and the
    /// two disagree about travel. A lit beam crossing a travel burns a line the design never asked
    /// for; a spindle turning through a rapid is not a hazard but mandatory practice, and stopping it
    /// between passes would be wrong. An always-on rule cannot tell them apart from `Segment.power`
    /// alone, so it errored on both: on an ordinary milling rapid at `S8000`, and on Dry's own
    /// resolved laser output, since the channel is sticky and no in-tree producer forces travels
    /// dark. Only a profile knows which process it is describing, so only a profile may turn this on.
    ///
    /// Whether `resolve` should itself force travels dark is a separate, still-open question
    /// (`docs/04-tasks.md`); gating the rule does not decide it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub travel_must_be_dark: Option<bool>,
}

/// Kinematic limits checked by the `peak-acceleration` (arc centripetal) and `junction-velocity`
/// (cornering) rules. An unset field disables its check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KinematicContracts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_acceleration_mm_s2: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_junction_velocity_mm_s: Option<f64>,
}

/// The rotary model used when a `rotary` contract names none.
///
/// The same machine `Profile::emit_params` falls back to, on purpose: the rotary rules check the words
/// the *emitter* would write, so a verify default that disagreed with the emit default would be
/// checking a program nobody is going to run.
fn reference_rotary_model() -> crate::emit::Kinematics {
    crate::emit::REFERENCE_FIVE_AXIS_MACHINE
}

/// Rotary-axis limits checked by the `rotary-travel`, `rotary-feed` and `orientation-reachability`
/// rules. An unset field disables its check.
///
/// All three are contract-gated rather than structural because each states a property of a **machine**,
/// not of the IR: a toolpath that tilts 180° or reaches 400 mm out is perfectly well-formed, and is
/// unreachable only on a machine that cannot do it. There is no Dry producer that can emit IR violating
/// these, which is exactly the H1.3 test for "not always-on".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotaryContracts {
    /// The rotary model the toolframe orientation is resolved through — the same mapping and the same
    /// two axis letters the emitter uses, so the angles checked are the angles that will be written.
    #[serde(default = "reference_rotary_model")]
    pub model: crate::emit::Kinematics,
    /// Per-axis rotary travel in degrees, keyed by the axis letter the model emits. An axis with no
    /// range is unconstrained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub travel_deg: Option<RotaryTravelRanges>,
    /// Maximum rate for **any** rotary axis, in deg/min.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rotary_feed_deg_min: Option<f64>,
    /// The reachable workspace as `[[x_lo, x_hi], [y_lo, y_hi], [z_lo, z_hi]]` (mm) in **machine**
    /// coordinates — i.e. after the orientation has been resolved into rotary motion. This is not the
    /// build volume: `bounds` checks the programmed (workpiece) coordinates, this checks where the
    /// rotation actually puts the tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_mm: Option<[[f64; 2]; 3]>,
}

impl Default for RotaryContracts {
    fn default() -> Self {
        RotaryContracts {
            model: reference_rotary_model(),
            travel_deg: None,
            max_rotary_feed_deg_min: None,
            envelope_mm: None,
        }
    }
}

/// Rotary travel ranges `[min, max]` in degrees, one per axis letter. An absent axis is unconstrained.
///
/// Keyed by letter rather than by position so a range cannot silently be read against the wrong axis
/// when the model changes: `Bc` emits `C` then `B`, `Ab` emits `A` then `B`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct RotaryTravelRanges {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub c: Option<[f64; 2]>,
}

impl RotaryTravelRanges {
    /// The range constraining `letter`, or `None` when that axis is unconstrained.
    pub fn range(&self, letter: char) -> Option<[f64; 2]> {
        match letter {
            'A' => self.a,
            'B' => self.b,
            'C' => self.c,
            _ => None,
        }
    }

    /// Whether any axis is constrained at all. An all-empty table checks nothing, so the rule it gates
    /// is not evaluated and does not appear in `rules_evaluated`.
    pub fn any_set(&self) -> bool {
        self.a.is_some() || self.b.is_some() || self.c.is_some()
    }
}

/// A user-facing contract configuration parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractParseError {
    message: String,
}

impl ContractParseError {
    fn new(message: impl Into<String>) -> Self {
        ContractParseError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ContractParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ContractParseError {}

fn parse_csv_f64s(name: &str, s: &str, expected: usize) -> Result<Vec<f64>, ContractParseError> {
    let values: Result<Vec<f64>, _> = s.split(',').map(|t| t.trim().parse::<f64>()).collect();
    let values = values.map_err(|e| ContractParseError::new(format!("bad {name} value: {e}")))?;
    if values.len() != expected {
        return Err(ContractParseError::new(format!(
            "{name} needs {expected} comma-separated numbers"
        )));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(ContractParseError::new(format!(
            "{name} values must all be finite"
        )));
    }
    Ok(values)
}

fn validate_range_order(name: &str, [lo, hi]: [f64; 2]) -> Result<(), ContractParseError> {
    if lo > hi {
        return Err(ContractParseError::new(format!(
            "{name} lower bound must be <= upper bound"
        )));
    }
    Ok(())
}

/// Parse `x0,x1,y0,y1,z0,z1` into build-volume bounds.
pub fn parse_bounds_csv(s: &str) -> Result<[[f64; 2]; 3], ContractParseError> {
    let v = parse_csv_f64s("bounds", s, 6)?;
    let bounds = [[v[0], v[1]], [v[2], v[3]], [v[4], v[5]]];
    for (axis, range) in ["x", "y", "z"].into_iter().zip(bounds) {
        validate_range_order(&format!("bounds {axis}"), range)?;
    }
    Ok(bounds)
}

/// Parse `min,max` into an extruding-move feedrate range.
pub fn parse_speed_range_csv(s: &str) -> Result<[f64; 2], ContractParseError> {
    let v = parse_csv_f64s("speed range", s, 2)?;
    let range = [v[0], v[1]];
    validate_range_order("speed range", range)?;
    Ok(range)
}

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The toolpath is unsafe / invalid.
    Error,
    /// Suspicious but not necessarily fatal.
    Warning,
}

/// The closed set of verification rule ids. This is the single source of truth for the rule vocabulary,
/// each rule's default [`Severity`], and its one-line summary — the rule catalog (`docs/11`) and the
/// report schema (`spec/dry-reports-v1.schema.json`) are derived from it. A rule is **error** unless it
/// is a process/quality advisory (stringing, first-layer adhesion, an IR classification the controller
/// does not act on) rather than a machine-safety or geometric-validity violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleId {
    /// A quantity is non-finite (NaN/inf).
    Finite,
    /// A travel move deposits material.
    TravelExtrudes,
    /// An extruding move has a non-positive bead (width/height).
    Bead,
    /// The toolframe orientation is not a unit vector.
    OrientationNotUnit,
    /// An arc's endpoint radius disagrees with its start radius (or the arc is malformed).
    ArcRadius,
    /// A move leaves the build volume.
    Bounds,
    /// Volumetric flow exceeds the ceiling.
    MaxFlow,
    /// An extruding feedrate is outside the allowed range.
    Speed,
    /// Z decreases where it must be non-decreasing.
    MonotonicZ,
    /// Extruding below the minimum nozzle temperature (or with none set).
    ColdExtrusion,
    /// A retraction distance exceeds the limit.
    RetractionDistance,
    /// A pure retraction/unretraction speed exceeds the limit.
    RetractionSpeed,
    /// A travel run exceeds the allowed distance without a retraction (stringing risk — advisory).
    TravelWithoutRetraction,
    /// First-layer height is outside the allowed range (adhesion — advisory).
    FirstLayerHeight,
    /// First-layer speed is outside the allowed range (adhesion — advisory).
    FirstLayerSpeed,
    /// An arc's centripetal acceleration exceeds the machine's max acceleration.
    PeakAcceleration,
    /// A junction is entered faster than its direction change allows.
    JunctionVelocity,
    /// Verbatim or imported G-code is preserved but not semantically verified.
    UnmodeledGcode,
    /// A segment starts somewhere other than where the previous one ended.
    Continuity,
    /// A quantity that cannot be negative is (length, volume, speed, power), or a bead dimension is ≤ 0.
    NegativeQuantity,
    /// A straight or stationary segment's declared length disagrees with its own endpoints.
    SegmentLength,
    /// An arc's declared length disagrees with its radius and swept angle.
    ArcLength,
    /// The volume-to-filament ratio changes within one tool.
    FilamentConsistency,
    /// Deposited volume disagrees with the bead geometry (`length·width·height·flow`).
    BeadVolume,
    /// A commanded rotary word is outside its axis's travel range.
    RotaryTravel,
    /// A rotary axis would have to turn faster than its rate limit to keep up with the move.
    RotaryFeed,
    /// The machine position an orientation implies is outside the machine's reachable envelope.
    OrientationReachability,
    /// The spindle or laser power is active (> 0) during a rapid traversal move (travel: true).
    LaserPowerDuringTravel,
}

/// One rule's catalog entry.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub id: RuleId,
    pub severity: Severity,
    pub summary: &'static str,
}

impl RuleId {
    /// Every rule, in catalog order.
    pub const ALL: [RuleId; 28] = [
        RuleId::Finite,
        RuleId::TravelExtrudes,
        RuleId::Bead,
        RuleId::OrientationNotUnit,
        RuleId::ArcRadius,
        RuleId::Bounds,
        RuleId::MaxFlow,
        RuleId::Speed,
        RuleId::MonotonicZ,
        RuleId::ColdExtrusion,
        RuleId::RetractionDistance,
        RuleId::RetractionSpeed,
        RuleId::TravelWithoutRetraction,
        RuleId::FirstLayerHeight,
        RuleId::FirstLayerSpeed,
        RuleId::PeakAcceleration,
        RuleId::JunctionVelocity,
        RuleId::UnmodeledGcode,
        RuleId::Continuity,
        RuleId::NegativeQuantity,
        RuleId::SegmentLength,
        RuleId::ArcLength,
        RuleId::FilamentConsistency,
        RuleId::BeadVolume,
        RuleId::RotaryTravel,
        RuleId::RotaryFeed,
        RuleId::OrientationReachability,
        RuleId::LaserPowerDuringTravel,
    ];

    /// The stable kebab-case wire id.
    pub fn as_str(self) -> &'static str {
        match self {
            RuleId::Finite => "finite",
            RuleId::TravelExtrudes => "travel-extrudes",
            RuleId::Bead => "bead",
            RuleId::OrientationNotUnit => "orientation-not-unit",
            RuleId::ArcRadius => "arc-radius",
            RuleId::Bounds => "bounds",
            RuleId::MaxFlow => "max-flow",
            RuleId::Speed => "speed",
            RuleId::MonotonicZ => "monotonic-z",
            RuleId::ColdExtrusion => "cold-extrusion",
            RuleId::RetractionDistance => "retraction-distance",
            RuleId::RetractionSpeed => "retraction-speed",
            RuleId::TravelWithoutRetraction => "travel-without-retraction",
            RuleId::FirstLayerHeight => "first-layer-height",
            RuleId::FirstLayerSpeed => "first-layer-speed",
            RuleId::PeakAcceleration => "peak-acceleration",
            RuleId::JunctionVelocity => "junction-velocity",
            RuleId::UnmodeledGcode => "unmodeled-gcode",
            RuleId::Continuity => "continuity",
            RuleId::NegativeQuantity => "negative-quantity",
            RuleId::SegmentLength => "segment-length",
            RuleId::ArcLength => "arc-length",
            RuleId::FilamentConsistency => "filament-consistency",
            RuleId::BeadVolume => "bead-volume",
            RuleId::RotaryTravel => "rotary-travel",
            RuleId::RotaryFeed => "rotary-feed",
            RuleId::OrientationReachability => "orientation-reachability",
            RuleId::LaserPowerDuringTravel => "laser-power-during-travel",
        }
    }

    /// Parse a wire id back into a [`RuleId`].
    pub fn from_wire(s: &str) -> Option<RuleId> {
        RuleId::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// The rule's default severity.
    pub fn default_severity(self) -> Severity {
        match self {
            RuleId::TravelWithoutRetraction
            | RuleId::FirstLayerHeight
            | RuleId::FirstLayerSpeed
            | RuleId::JunctionVelocity
            | RuleId::UnmodeledGcode
            // `travel` is a *classification*, and this rule states that the classification disagrees
            // with the deposited volume — not that anything unsafe happens. Three facts settle the
            // severity, all of them about who sets the flag:
            //
            //  - No in-tree producer can violate it. Every travel `resolve` and `optimize` emit
            //    carries `Volume::ZERO`, so `travel: true` from Dry is an *assertion*, and error
            //    severity never gated Dry-authored IR at all — only imported and hand-authored IR,
            //    where the flag is *inferred*.
            //  - For imported G-code the inference is `G0 || no E word` (`gcode::lift`), and `G0` is
            //    not a "do not extrude" command on the firmware these programs run: Marlin, Klipper
            //    and RepRapFirmware execute `G0` as an ordinary coordinated move and honour an `E`
            //    word in it. OrcaSlicer's stock start G-code relies on exactly that to write its
            //    purge/prime lines — in the Bambu X1C profile and the Prusa MK4 one alike — so stock
            //    output trips this rule 4-21 times per file while commanding precisely what its
            //    author intended.
            //  - What the finding *does* buy is real but advisory: a move counted as travel while
            //    depositing corrupts travel-derived accounting (travel time/distance in `simulate`,
            //    and `travel-without-retraction`, itself a warning). That is process/quality
            //    character, which is the criterion this doc comment already states for warning.
            //
            // Severity is deliberately *not* scoped to provenance, even though `Toolpath.meta`
            // records `imported-from-gcode`: `verify_stream` cannot see `meta` by construction, so
            // the same bytes would verify differently through `dry verify` than through
            // `dry review-gcode`; `Report` echoes `contracts` but not `meta`, so the difference
            // would be invisible in the report; and `meta` is producer-declared, so keying severity
            // off it would let an input choose the severity the verifier assigns it.
            | RuleId::TravelExtrudes
            // The controller does not refuse a rotary axis it cannot drive fast enough — it slows the
            // whole synchronised move down. The program still runs and still cuts the commanded path;
            // what is wrong is the plan, not the geometry. Same character as `junction-velocity`.
            | RuleId::RotaryFeed
            // Ships as a warning for one minor release before promotion to error (design §8):
            // multi-diameter / multi-material IR is unusual but not ill-formed, and no in-tree
            // producer makes any, so we have no evidence either way yet.
            | RuleId::FilamentConsistency => Severity::Warning,
            _ => Severity::Error,
        }
    }

    // The three rationales above were deleted wholesale by 02ed2dc and restored on 2026-08-30. They
    // are not commentary: `docs/11-profiles-and-reports.md` mirrors them, and a severity that states
    // no reason is exactly how `travel-extrudes` came to be argued about twice. Losing them cost
    // nothing at the time because the docs still carried the reasoning — which is precisely why the
    // deletion went unnoticed through review.

    /// A one-line human summary for the catalog/docs.
    pub fn summary(self) -> &'static str {
        match self {
            RuleId::Finite => "a quantity is non-finite (NaN or infinite)",
            RuleId::TravelExtrudes => "a travel (non-printing) move deposits material",
            RuleId::Bead => "an extruding move has a non-positive bead width or height",
            RuleId::OrientationNotUnit => "the toolframe orientation vector is not unit length",
            RuleId::ArcRadius => "an arc's endpoint radius disagrees with its start radius",
            RuleId::Bounds => "a move leaves the build volume",
            RuleId::MaxFlow => "volumetric flow exceeds the configured ceiling",
            RuleId::Speed => "an extruding feedrate is outside the allowed range",
            RuleId::MonotonicZ => "Z decreases where it is required to be non-decreasing",
            RuleId::ColdExtrusion => "extruding below the minimum nozzle temperature",
            RuleId::RetractionDistance => "a retraction distance exceeds the limit",
            RuleId::RetractionSpeed => "a retraction or unretraction speed exceeds the limit",
            RuleId::TravelWithoutRetraction => {
                "a travel run exceeds the allowed distance without a retraction"
            }
            RuleId::FirstLayerHeight => "the first-layer height is outside the allowed range",
            RuleId::FirstLayerSpeed => "the first-layer speed is outside the allowed range",
            RuleId::PeakAcceleration => {
                "an arc's centripetal acceleration exceeds the machine's max acceleration"
            }
            RuleId::JunctionVelocity => {
                "a junction is entered faster than its direction change allows"
            }
            RuleId::UnmodeledGcode => {
                "verbatim or imported G-code is preserved but not semantically verified"
            }
            RuleId::Continuity => {
                "a segment starts somewhere other than where the previous one ended"
            }
            RuleId::NegativeQuantity => {
                "a length, volume, speed or commanded spindle/laser power is negative, or a bead dimension is not positive"
            }
            RuleId::SegmentLength => {
                "a straight or stationary segment's length disagrees with its own endpoints"
            }
            RuleId::ArcLength => "an arc's length disagrees with its radius and swept angle",
            RuleId::FilamentConsistency => {
                "the volume-to-filament ratio changes within a single tool"
            }
            RuleId::BeadVolume => {
                "deposited volume disagrees with the bead geometry (length x width x height x flow)"
            }
            RuleId::RotaryTravel => "a rotary axis is commanded outside its travel range",
            RuleId::RotaryFeed => {
                "a rotary axis would have to turn faster than its rate limit to keep up with the move"
            }
            RuleId::OrientationReachability => {
                "an orientation puts the tool outside the machine's reachable envelope"
            }
            RuleId::LaserPowerDuringTravel => {
                "spindle or laser power is active (> 0) during a rapid traversal move"
            }
        }
    }

    /// Whether this rule is evaluated at all under `c` — i.e. whether it is structural (always on)
    /// or its gating contract supplies a limit.
    ///
    /// This is what makes a vacuous pass visible: [`Report::rules_evaluated`] is built from it, so a
    /// report clean under ten rules is distinguishable from one clean under twenty-four.
    pub fn is_evaluated(self, c: &Contracts) -> bool {
        match self {
            // Structural / well-formedness: no contract can make a violation acceptable.
            RuleId::Finite
            | RuleId::TravelExtrudes
            | RuleId::Bead
            | RuleId::OrientationNotUnit
            | RuleId::ArcRadius
            | RuleId::UnmodeledGcode
            | RuleId::Continuity
            | RuleId::NegativeQuantity
            | RuleId::SegmentLength
            | RuleId::ArcLength
            | RuleId::FilamentConsistency => true,
            // Process-gated: see `Contracts::travel_must_be_dark`. A spindle running through a rapid
            // and a laser burning through one are the same `Segment.power`; only a profile knows
            // which it is.
            RuleId::LaserPowerDuringTravel => c.travel_must_be_dark.is_some_and(|dark| dark),
            // A ceiling is only in force if it can actually decide anything. Every comparison
            // against a NaN ceiling is false, so a rule carrying one can never fire — and reporting
            // it as evaluated is exactly the vacuous pass `rules_evaluated` exists to rule out
            // (H1.3 design section 3.5). `Some(NaN)` is not a contract; it is a typo.
            RuleId::Bounds => c.bounds.is_some_and(usable_bounds),
            RuleId::MaxFlow => c.max_flow.is_some_and(usable),
            RuleId::Speed => c.speed_range.is_some_and(usable_range),
            RuleId::MonotonicZ => c.monotonic_z,
            RuleId::ColdExtrusion => c.min_temp.is_some_and(usable),
            RuleId::RetractionDistance => c.max_retraction_distance.is_some_and(usable),
            RuleId::RetractionSpeed => c.max_retraction_speed.is_some_and(usable),
            RuleId::TravelWithoutRetraction => c.max_travel_without_retract.is_some_and(usable),
            RuleId::FirstLayerHeight => c.first_layer_height_range.is_some_and(usable_range),
            RuleId::FirstLayerSpeed => c.first_layer_speed_range.is_some_and(usable_range),
            RuleId::BeadVolume => c.bead_volume_tolerance.is_some_and(usable),
            RuleId::PeakAcceleration => c
                .kinematics
                .as_ref()
                .is_some_and(|k| k.max_acceleration_mm_s2.is_some_and(usable)),
            RuleId::JunctionVelocity => c
                .kinematics
                .as_ref()
                .is_some_and(|k| k.max_junction_velocity_mm_s.is_some_and(usable)),
            RuleId::RotaryTravel => c.rotary.as_ref().is_some_and(|r| {
                r.travel_deg
                    .as_ref()
                    .is_some_and(RotaryTravelRanges::any_set)
            }),
            RuleId::RotaryFeed => c
                .rotary
                .as_ref()
                .is_some_and(|r| r.max_rotary_feed_deg_min.is_some()),
            RuleId::OrientationReachability => {
                c.rotary.as_ref().is_some_and(|r| r.envelope_mm.is_some())
            }
        }
    }
}

/// Whether a scalar contract value can decide anything at all.
///
/// Written as a positive test rather than `!v.is_nan()`: every ordering comparison against NaN is
/// false, so a rule holding one silently never fires, and an infinite ceiling is indistinguishable
/// from having set no ceiling at all.
fn usable(value: f64) -> bool {
    value.is_finite()
}

/// A range contract is in force only when both ends are usable.
fn usable_range(range: [f64; 2]) -> bool {
    usable(range[0]) && usable(range[1])
}

/// A build volume is in force only when every one of its six numbers is usable.
fn usable_bounds(bounds: [[f64; 2]; 3]) -> bool {
    bounds.iter().all(|axis| usable_range(*axis))
}

/// The full rule catalog (id, default severity, summary), in catalog order.
pub fn catalog() -> Vec<Rule> {
    RuleId::ALL
        .into_iter()
        .map(|id| Rule {
            id,
            severity: id.default_severity(),
            summary: id.summary(),
        })
        .collect()
}

/// One located issue found by [`verify`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// A stable kebab-case rule id from the [`RuleId`] catalog.
    pub rule: String,
    pub severity: Severity,
    /// The offending segment index, if the finding is local to one move.
    pub segment: Option<usize>,
    /// A human-readable description.
    pub message: String,
}

/// The result of verifying a toolpath.
///
/// The three fields beside `findings` exist so that a **vacuous** pass is not byte-identical to a real
/// one (H1.3 design §3.5). `ok()` alone cannot distinguish "clean" from "nothing was inspected" or
/// "clean against no limits at all", and eight in-tree call sites were reading it as an assurance
/// claim. All three are `#[serde(default)]`, so reports written by older Dry still deserialize.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    /// How many segments this pass actually looked at. Zero means the pass proved nothing.
    #[serde(default)]
    pub segments_inspected: usize,
    /// The wire ids of every rule that was in force, in catalog order.
    #[serde(default)]
    pub rules_evaluated: Vec<String>,
    /// The contracts the toolpath was checked against.
    #[serde(default)]
    pub contracts: Contracts,
    /// The licensing mode this report was produced under, when the caller stamped one
    /// (see [`crate::LicenseStamp`]) — never set by the engine.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub license: Option<crate::report::LicenseStamp>,
}

impl Report {
    /// True when there are no `Error`-severity findings.
    ///
    /// Note this says nothing about *coverage*: see [`Report::segments_inspected`] and
    /// [`Report::evaluated`] before treating it as an assurance claim.
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
    /// Whether `rule` was in force for this report.
    pub fn evaluated(&self, rule: RuleId) -> bool {
        self.rules_evaluated.iter().any(|r| r == rule.as_str())
    }
}

/// Relative tolerance for `arc-radius`: how far `|end − centre|` may differ from `|start − centre|`
/// before the two contradict each other.
///
/// The single definition in the tree. `resolve` applies the same epsilon at the L1 gate and imports
/// it from here rather than restating it — two literals could be retuned apart, and the published
/// boundary `FM1.F64.VERIFY.ARC_RADIUS` can only pin one of them.
pub(crate) const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;

/// Per-axis continuity tolerance (mm), applied by the hybrid rule below.
///
/// 1e-6 mm is the emitter's own print resolution (`num()` formats `{v:.6}`), so a gap smaller than
/// this is not representable in the output at all.
const CONTINUITY_TOLERANCE_MM: f64 = 1e-6;

/// Relative tolerance for `segment-length` and `arc-length`.
const LENGTH_TOLERANCE: f64 = 1e-6;

/// Relative tolerance for `filament-consistency`.
const FILAMENT_RATIO_TOLERANCE: f64 = 1e-6;

/// The tolerance idiom already used at `arc_radius_error` and `gcode/lift.rs:819`: absolute below
/// 1 mm, relative above. Keeps `verify` on one tolerance policy rather than three, and stays
/// satisfiable at large coordinates where `f64` spacing alone exceeds a fixed 1e-6 mm.
fn differs_beyond(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() > rel * a.abs().max(b.abs()).max(1.0)
}

/// Per-segment volumetric flow (mm³/s), or `None` for a move with no duration.
fn flow(s: &Segment) -> Option<f64> {
    segment_motion_time(s).map(|time| (s.volume / time).value())
}

/// True when the segment traverses a path of non-zero geometric length — as opposed to a **pure
/// filament move**, where the E axis turns while the tool stays where it is.
///
/// The distinction is what separates one physical act from another, and imported G-code is where it
/// becomes load-bearing. A retract or prime line carries an `E` word and no `X`/`Y`
/// (`G1 E1 F2400`, `G1 E-1 F3600`), so `gcode::lift` gives it `travel: false` — the flag is inferred
/// from "`G0`, or no `E` word" — a `volume` recovered from the `E` delta, and a geometric length of
/// zero. The filament is only moving through the feed path; nothing is laid on the part, no bead
/// exists, and the commanded feedrate is the *filament's* speed rather than the tool's.
///
/// Three families of rule turn on it:
///  - **deposition** (`max-flow`, `bead-volume`, the first-layer pair) measures a rate or a bead over
///    a path, and has nothing to measure without one. Scoring `G1 E1 F2400` as a deposition rate
///    reports `area × F/60` = 96.2 mm³/s for 1.75 mm filament — above every real print-flow ceiling,
///    on a move that deposits nothing (`docs/14`).
///  - **retraction** (`retraction-speed`, `retraction-distance`) limits the filament's own speed and
///    distance, so it applies exactly when this is false.
///  - **cornering** (`junction-velocity`) needs two tangents, and a zero-length move has none.
fn traverses_path(s: &Segment) -> bool {
    s.length.value() > 0.0
}

/// True when the segment lays material *along a path*: the domain of every deposition rule that also
/// needs the producer's own classification to agree ([`traverses_path`] states the geometric half).
fn deposits_along_path(s: &Segment) -> bool {
    !s.travel && traverses_path(s) && s.volume.value() > 0.0
}

fn segment_numbers(s: &Segment) -> Vec<f64> {
    let mut nums = vec![
        s.speed.value(),
        s.length.value(),
        s.volume.value(),
        s.filament.value(),
    ];
    nums.extend(s.start.iter().flatten().map(|v| v.value()));
    nums.extend(s.end.iter().flatten().map(|v| v.value()));
    if let Some(w) = s.width {
        nums.push(w.value());
    }
    if let Some(h) = s.height {
        nums.push(h.value());
    }
    if let Some([cx, cy]) = s.centre {
        nums.push(cx.value());
        nums.push(cy.value());
    }
    nums.extend(
        [s.temperature, s.fan, s.flow, s.power, s.dwell_s]
            .into_iter()
            .flatten(),
    );
    if let Some(o) = s.orientation {
        nums.extend(o);
    }
    if let Some(points) = &s.control_points {
        for p in points {
            nums.extend(p.iter().map(|v| v.value()));
        }
    }
    nums
}

fn normalised_angle(v: f64) -> f64 {
    let mut out = v % TAU;
    if out < 0.0 {
        out += TAU;
    }
    out
}

fn swept_delta(start: f64, end: f64, clockwise: bool) -> f64 {
    let delta = normalised_angle(if clockwise { start - end } else { end - start });
    if delta <= 1e-12 {
        TAU
    } else {
        delta
    }
}

fn delta_to_angle(start: f64, angle: f64, clockwise: bool) -> f64 {
    normalised_angle(if clockwise {
        start - angle
    } else {
        angle - start
    })
}

fn push_arc_bounds_points(s: &Segment, points: &mut Vec<[Option<Length>; 3]>) {
    let Some([cx, cy]) = s.centre else {
        return;
    };
    let (Some(sx), Some(sy), Some(ex), Some(ey)) = (s.start[0], s.start[1], s.end[0], s.end[1])
    else {
        return;
    };
    let radius = (sx - cx).hypot(sy - cy).value();
    if !radius.is_finite() {
        return;
    }
    let start_a = (sy - cy).atan2(sx - cx).value();
    let end_a = (ey - cy).atan2(ex - cx).value();
    let sweep = swept_delta(start_a, end_a, s.clockwise);

    for angle in [0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2] {
        let delta = delta_to_angle(start_a, angle, s.clockwise);
        if delta <= sweep + 1e-12 {
            let z = match (s.start[2], s.end[2]) {
                (Some(z0), Some(z1)) => Some(Length::mm(
                    z0.value() + (z1.value() - z0.value()) * (delta / sweep),
                )),
                _ => None,
            };
            points.push([
                Some(Length::mm(cx.value() + radius * libm::cos(angle))),
                Some(Length::mm(cy.value() + radius * libm::sin(angle))),
                z,
            ]);
        }
    }
}

fn push_spline_bounds_points(s: &Segment, points: &mut Vec<[Option<Length>; 3]>) {
    let Some(control_points) = &s.control_points else {
        return;
    };
    let start = [
        s.start[0].unwrap_or(Length::ZERO).value(),
        s.start[1].unwrap_or(Length::ZERO).value(),
        s.start[2].unwrap_or(Length::ZERO).value(),
    ];
    points.push([
        Some(Length::mm(start[0])),
        Some(Length::mm(start[1])),
        Some(Length::mm(start[2])),
    ]);
    let mut through = Vec::with_capacity(control_points.len() + 1);
    through.push(start);
    through.extend(
        control_points
            .iter()
            .map(|p| [p[0].value(), p[1].value(), p[2].value()]),
    );

    for i in 0..through.len().saturating_sub(1) {
        let p0 = through[i.saturating_sub(1)];
        let p1 = through[i];
        let p2 = through[i + 1];
        let p3 = through[(i + 2).min(through.len() - 1)];
        for step in 1..=SAMPLES {
            let pt = if step == SAMPLES {
                p2
            } else {
                catmull_rom(p0, p1, p2, p3, step as f64 / SAMPLES as f64)
            };
            points.push([
                Some(Length::mm(pt[0])),
                Some(Length::mm(pt[1])),
                Some(Length::mm(pt[2])),
            ]);
        }
    }
}

fn bounds_points(s: &Segment) -> Vec<[Option<Length>; 3]> {
    let mut points = vec![s.start, s.end];
    if s.kind == SegmentKind::Arc {
        push_arc_bounds_points(s, &mut points);
    } else if s.kind == SegmentKind::Spline {
        push_spline_bounds_points(s, &mut points);
    }
    points
}

/// Arc radius in mm from the segment's start point and centre; `None` if the segment is not a
/// well-formed arc (missing centre/start, or degenerate).
fn arc_radius_mm(s: &Segment) -> Option<f64> {
    let [cx, cy] = s.centre?;
    let (sx, sy) = (s.start[0]?, s.start[1]?);
    let r = (sx - cx).hypot(sy - cy).value();
    if r > 0.0 {
        Some(r)
    } else {
        None
    }
}

/// True when the previous printing segment's end ≈ this segment's start (within 0.1 mm in X/Y/Z).
fn junction_contiguous(
    prev_end: &Option<[Option<Length>; 3]>,
    start: &[Option<Length>; 3],
) -> bool {
    let Some(pe) = prev_end else {
        return false;
    };
    (0..3).all(|k| match (pe[k], start[k]) {
        (Some(a), Some(b)) => (a.value() - b.value()).abs() <= 0.1,
        (None, None) => true,
        _ => false,
    })
}

fn arc_radius_error(s: &Segment) -> Option<String> {
    if s.kind != SegmentKind::Arc {
        return None;
    }
    let Some([cx, cy]) = s.centre else {
        return Some("arc segment is missing centre".to_string());
    };
    let (Some(sx), Some(sy), Some(ex), Some(ey)) = (s.start[0], s.start[1], s.end[0], s.end[1])
    else {
        return Some("arc segment needs defined start and end X/Y".to_string());
    };
    let start_radius = (sx - cx).hypot(sy - cy).value();
    let end_radius = (ex - cx).hypot(ey - cy).value();
    if start_radius <= 0.0 || end_radius <= 0.0 {
        return Some("arc segment needs a non-zero radius".to_string());
    }
    let tolerance = ARC_RADIUS_TOLERANCE_MM * start_radius.max(end_radius).max(1.0);
    let delta = (start_radius - end_radius).abs();
    if delta > tolerance {
        Some(format!(
            "arc endpoint radius differs from start radius by {delta:.6} mm"
        ))
    } else {
        None
    }
}

/// The straight-line distance between a segment's own endpoints, or `None` when any axis is
/// undefined on either side (an undefined axis inherits, so no displacement is asserted).
fn endpoint_distance_mm(s: &Segment) -> Option<f64> {
    let (Some(sx), Some(sy), Some(sz), Some(ex), Some(ey), Some(ez)) = (
        s.start[0], s.start[1], s.start[2], s.end[0], s.end[1], s.end[2],
    ) else {
        return None;
    };
    let dx = ex.value() - sx.value();
    let dy = ey.value() - sy.value();
    let dz = ez.value() - sz.value();
    Some(libm::sqrt(dx * dx + dy * dy + dz * dz))
}

/// The arc length implied by a segment's own radius and swept angle: `hypot(r·sweep, Δz)`.
///
/// One formula across the tree — `resolve.rs:602-614`, `gcode/lift.rs:840` and `optimize/arc.rs:140`
/// all agree — which is what makes this checkable always-on. `None` for a malformed arc, which is
/// `arc-radius`'s business rather than this rule's.
fn arc_length_mm(s: &Segment) -> Option<f64> {
    let [cx, cy] = s.centre?;
    let (sx, sy, ex, ey) = (s.start[0]?, s.start[1]?, s.end[0]?, s.end[1]?);
    let radius = (sx - cx).hypot(sy - cy).value();
    // is_finite() first, so a NaN radius returns None rather than slipping past a `<= 0.0` test.
    if !radius.is_finite() || radius <= 0.0 {
        return None;
    }
    let start_a = (sy - cy).atan2(sx - cx).value();
    let end_a = (ey - cy).atan2(ex - cx).value();
    let sweep = swept_delta(start_a, end_a, s.clockwise);
    let dz = match (s.start[2], s.end[2]) {
        (Some(z0), Some(z1)) => z1.value() - z0.value(),
        _ => 0.0,
    };
    Some(libm::hypot(radius * sweep, dz))
}

/// `segment-length` applies to primitives whose length is the straight-line distance between their
/// endpoints. `Arc` belongs to `arc-length`; a `Spline`'s length is the sampled curve, not the chord;
/// `ManualGcode` is unmodeled by definition.
fn has_straight_length(kind: SegmentKind) -> bool {
    !matches!(
        kind,
        SegmentKind::Arc | SegmentKind::Spline | SegmentKind::ManualGcode
    )
}

fn push_finding(report: &mut Report, rule: RuleId, segment: Option<usize>, message: String) {
    report.findings.push(Finding {
        rule: rule.as_str().to_string(),
        severity: rule.default_severity(),
        segment,
        message,
    });
}

/// Verify a stream of segments against the contracts, returning all findings (structural + contract-driven).
pub fn verify_stream<I>(segments: I, c: &Contracts) -> Result<Report, crate::codec::CodecError>
where
    I: IntoIterator<Item = Result<Segment, crate::codec::CodecError>>,
{
    let mut r = Report {
        rules_evaluated: RuleId::ALL
            .into_iter()
            .filter(|rule| rule.is_evaluated(c))
            .map(|rule| rule.as_str().to_string())
            .collect(),
        contracts: c.clone(),
        ..Report::default()
    };
    let axis = ['X', 'Y', 'Z'];
    let mut first_layer_z: Option<f64> = None;

    let mut travel_run_length = 0.0;
    let mut retracted = true;
    let mut flagged_travel = false;
    // For junction-velocity: track the exit tangent and speed of the previous printing segment so we
    // can measure the direction change at contiguous junctions (reset on travel moves).
    let mut prev_print_end: Option<[Option<Length>; 3]> = None;
    let mut prev_speed_mm_s: Option<f64> = None;
    let mut prev_exit_tangent: Option<[f64; 3]> = None;
    // The C axis is history-dependent inside the singular cone, so the angles the emitter will
    // actually write depend on what came before. Threaded here exactly as `emit_stream` threads it —
    // advanced once per motion segment, untouched by dwells and manual g-code — because a rotary
    // limit checked against a *differently* resolved C would be judging a program nobody emits. That
    // is the `junction-velocity` mistake: two names, two quantities.
    let mut rotary_state = RotaryState::default();
    // For continuity: the machine position after the previous segment, per axis. An axis stays at its
    // last defined value ("inherit"), which is what both `resolve` and the emitter do.
    let mut tracked_pos: [Option<Length>; 3] = [None; 3];
    // For filament-consistency: the first volume/filament ratio observed for each tool.
    let mut tool_ratio: std::collections::BTreeMap<Option<u32>, f64> =
        std::collections::BTreeMap::new();
    // For rotary-feed: the rotary words the previous segment left the axes on, in the order the model
    // emits them. `None` before the first segment the model could resolve, and again after verbatim
    // G-code, which may drive the rotary axes to somewhere we cannot know.
    let mut prev_rotary: Option<[f64; 2]> = None;

    for (i, segment) in segments.into_iter().enumerate() {
        let s = segment?;
        r.segments_inspected += 1;
        // --- structural invariants (always on) ---
        if s.kind == SegmentKind::ManualGcode {
            push_finding(
                &mut r,
                RuleId::UnmodeledGcode,
                Some(i),
                "verbatim manual G-code is emitted without semantic verification".into(),
            );
        }
        let nums = segment_numbers(&s);
        if nums.iter().any(|v| !v.is_finite()) {
            push_finding(
                &mut r,
                RuleId::Finite,
                Some(i),
                "segment carries a non-finite value".into(),
            );
        }
        if s.travel && s.volume.value() > 0.0 {
            push_finding(
                &mut r,
                RuleId::TravelExtrudes,
                Some(i),
                format!(
                    "travel move deposits {:.4} mm³ (should be 0)",
                    s.volume.value()
                ),
            );
        }
        if c.travel_must_be_dark.is_some_and(|dark| dark)
            && s.travel
            && s.power.unwrap_or(0.0) > 0.0
        {
            push_finding(
                &mut r,
                RuleId::LaserPowerDuringTravel,
                Some(i),
                format!(
                    "travel move has active spindle/laser power (level {:.4})",
                    s.power.unwrap_or(0.0)
                ),
            );
        }
        if !s.travel && s.length > Length::ZERO {
            let w = s.width.map(|l| l.value()).unwrap_or(0.0);
            let h = s.height.map(|l| l.value()).unwrap_or(0.0);
            if w <= 0.0 || h <= 0.0 {
                push_finding(
                    &mut r,
                    RuleId::Bead,
                    Some(i),
                    format!("extruding move has a non-positive bead (width {w}, height {h})"),
                );
            }
        }
        if let Some([x, y, z]) = s.orientation {
            // the toolframe orientation must be a unit direction vector.
            let mag = libm::sqrt(x * x + y * y + z * z);
            if (mag - 1.0).abs() > 1e-6 {
                push_finding(
                    &mut r,
                    RuleId::OrientationNotUnit,
                    Some(i),
                    format!(
                        "toolframe orientation [{x}, {y}, {z}] has magnitude {mag} (must be 1)"
                    ),
                );
            }
        }
        if let Some(message) = arc_radius_error(&s) {
            push_finding(&mut r, RuleId::ArcRadius, Some(i), message);
        }

        // --- continuity: this segment must start where the last one left the machine ---
        // Verbatim G-code may move the machine arbitrarily, so we neither compare across it nor
        // claim to know where it ended: `unmodeled-gcode` already says the segment is outside the
        // model, and asserting continuity through it would be a stronger claim than we can support.
        if s.kind == SegmentKind::ManualGcode {
            tracked_pos = [None; 3];
        } else {
            for (k, (prev, start)) in tracked_pos.iter().zip(s.start.iter()).enumerate() {
                if let (Some(p), Some(q)) = (prev, start) {
                    let (p, q) = (p.value(), q.value());
                    if differs_beyond(p, q, CONTINUITY_TOLERANCE_MM) {
                        push_finding(
                            &mut r,
                            RuleId::Continuity,
                            Some(i),
                            format!(
                                "{} starts at {q} but the previous move ended at {p} (gap {:.6} mm); \
                                 the emitter writes endpoints only, so no repositioning move is \
                                 produced and the machine cuts straight across",
                                axis[k],
                                (p - q).abs()
                            ),
                        );
                    }
                }
            }
            for (tracked, end) in tracked_pos.iter_mut().zip(s.end.iter()) {
                *tracked = end.or(*tracked);
            }
        }

        // --- negative quantities: outside the IR's own type contract, so no contract can excuse them ---
        // `filament` < 0 is deliberately excluded: that is a retraction. `power` is included because
        // the IR spec makes `>= 0` normative (`docs/10` §3.3, `spec/dry-ir-v0.schema.json`) and this
        // is the only gate an IR file passes through — `validate_design` guards the L1 op, not a
        // toolpath decoded from JSON or DRY0/DRY1. Zero is legal there: it is "commanded off".
        for (name, value) in [
            ("length", s.length.value()),
            ("volume", s.volume.value()),
            ("speed", s.speed.value()),
        ]
        .into_iter()
        .chain(s.power.map(|p| ("power", p)))
        {
            if value < 0.0 {
                push_finding(
                    &mut r,
                    RuleId::NegativeQuantity,
                    Some(i),
                    format!("{name} is {value} (must not be negative)"),
                );
            }
        }
        for (name, value) in [("width", s.width), ("height", s.height)] {
            if let Some(v) = value.map(|l| l.value()) {
                if v <= 0.0 {
                    push_finding(
                        &mut r,
                        RuleId::NegativeQuantity,
                        Some(i),
                        format!("bead {name} is {v} (must be positive when set)"),
                    );
                }
            }
        }

        // --- declared length must agree with the segment's own geometry ---
        if has_straight_length(s.kind) {
            if let Some(expected) = endpoint_distance_mm(&s) {
                if differs_beyond(s.length.value(), expected, LENGTH_TOLERANCE) {
                    push_finding(
                        &mut r,
                        RuleId::SegmentLength,
                        Some(i),
                        format!(
                            "declared length {} disagrees with the distance between its own \
                             endpoints ({expected:.6} mm)",
                            s.length.value()
                        ),
                    );
                }
            }
        } else if s.kind == SegmentKind::Arc {
            if let Some(expected) = arc_length_mm(&s) {
                if differs_beyond(s.length.value(), expected, LENGTH_TOLERANCE) {
                    push_finding(
                        &mut r,
                        RuleId::ArcLength,
                        Some(i),
                        format!(
                            "declared length {} disagrees with the arc implied by its radius and \
                             swept angle ({expected:.6} mm)",
                            s.length.value()
                        ),
                    );
                }
            }
        }

        // --- filament consistency: volume/filament is the feedstock cross-section, one per tool ---
        if !s.travel && s.volume.value() > 0.0 && s.filament.value() > 0.0 {
            let ratio = s.volume.value() / s.filament.value();
            match tool_ratio.get(&s.tool) {
                None => {
                    tool_ratio.insert(s.tool, ratio);
                }
                Some(&base) => {
                    if (ratio - base).abs() > FILAMENT_RATIO_TOLERANCE * base.abs().max(ratio.abs())
                    {
                        push_finding(
                            &mut r,
                            RuleId::FilamentConsistency,
                            Some(i),
                            format!(
                                "volume/filament {ratio:.6} mm² differs from {base:.6} mm² seen \
                                 earlier on this tool; one of the two segments misstates how much \
                                 material it deposits"
                            ),
                        );
                    }
                }
            }
        }

        // --- contract-driven checks ---
        if let Some(tol) = c.bead_volume_tolerance {
            // Line and Spline only: `arc_fit` sums chord volumes against an arc length, and
            // `coasting` zeroes volume while keeping the bead, both by design. A pure filament move is
            // excluded for a third reason: its bead geometry is `length = 0`, so the identity's
            // right-hand side is 0 and *any* volume recovered from `E` differs from it by more than a
            // relative tolerance. There is no bead to compare against.
            let applies = matches!(s.kind, SegmentKind::Line | SegmentKind::Spline)
                && deposits_along_path(&s);
            if let (true, Some(w), Some(h)) = (applies, s.width, s.height) {
                // `flow` is omitted from the wire when exactly 1.0, so it must be defaulted.
                let flow = s.flow.unwrap_or(1.0);
                let expected = s.length.value() * w.value() * h.value() * flow;
                if (s.volume.value() - expected).abs() > tol * expected.abs() {
                    push_finding(
                        &mut r,
                        RuleId::BeadVolume,
                        Some(i),
                        format!(
                            "deposited volume {:.6} mm³ differs from the bead geometry \
                             ({expected:.6} mm³ = length x width x height x flow) by more than {tol}",
                            s.volume.value()
                        ),
                    );
                }
            }
        }
        if let Some(b) = c.bounds {
            'points: for point in bounds_points(&s) {
                for (k, coord) in point.iter().enumerate() {
                    if let Some(v) = coord {
                        let v = v.value();
                        if v < b[k][0] || v > b[k][1] {
                            push_finding(
                                &mut r,
                                RuleId::Bounds,
                                Some(i),
                                format!(
                                    "{} = {v} is outside the build volume [{}, {}]",
                                    axis[k], b[k][0], b[k][1]
                                ),
                            );
                            break 'points; // one bounds finding per segment
                        }
                    }
                }
            }
        }
        // A flow ceiling is a *deposition rate* limit, so it needs a path to deposit along
        // ([`traverses_path`]). `travel` is deliberately not required to be false: OrcaSlicer writes
        // its purge/prime lines as `G0` with an `E` word, and material pushed out at 30 mm³/s is a real
        // flow event whether or not the producer classified the move as a travel. `travel-extrudes`
        // reports the misclassification; this rule still reports the rate.
        if let (Some(max), Some(f), true) = (c.max_flow, flow(&s), traverses_path(&s)) {
            if f > max {
                push_finding(
                    &mut r,
                    RuleId::MaxFlow,
                    Some(i),
                    format!("flow {f:.3} mm³/s exceeds the ceiling {max:.3}"),
                );
            }
        }
        if let Some([lo, hi]) = c.speed_range {
            if deposits_along_path(&s) {
                let v = s.speed.value();
                if v < lo || v > hi {
                    push_finding(
                        &mut r,
                        RuleId::Speed,
                        Some(i),
                        format!("feedrate {v} is outside [{lo}, {hi}] mm/min"),
                    );
                }
            }
        }
        if c.monotonic_z {
            if let (Some(z0), Some(z1)) = (s.start[2], s.end[2]) {
                if z1 < z0 {
                    push_finding(
                        &mut r,
                        RuleId::MonotonicZ,
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
                push_finding(
                    &mut r,
                    RuleId::ColdExtrusion,
                    Some(i),
                    format!("extruding at nozzle temperature {got} (< {min} °C)"),
                );
            }
        }

        // --- retraction checks ---
        //
        // Both contracts limit the **filament's own** speed and distance, so both apply to a *pure*
        // retraction or unretraction: the E axis turns while the tool stays put ([`traverses_path`] is
        // false). That is what makes the commanded feedrate a retraction speed at all.
        //
        // A slicer wipe (`G1 X90.672 Y98.376 E-.11401 F3000`) retracts *while traversing*: `F` is the
        // wipe speed and the `E` delta is a fraction of one retraction, so neither limit is measuring
        // the quantity it names. OrcaSlicer's `retract_before_wipe` splits one retraction across both
        // forms — the stationary part at `retraction_speed`, the remainder along the wipe — and these
        // rules see only the stationary part. The coverage that costs is recorded in `docs/14`: a
        // slicer that retracted entirely inside a wipe would never be distance-checked. A wipe is a
        // different physical act and wants its own rule, not a reinterpretation of these two.
        let stationary = !traverses_path(&s);
        let is_retract = s.filament.value() < 0.0;
        // A pure unretract stages filament with no motion *and deposits nothing*. The `volume == 0`
        // conjunct is not redundant with `stationary`: it is what separates a prime from a stationary
        // **deposit** — the L1 `deposit` op lays material in place, and pinning that it is not a prime
        // is what `verify_contracts::stationary_deposit_is_not_a_retraction_prime` exists for. For
        // Dry-authored IR `volume` carries the distinction exactly. For imported G-code it cannot:
        // `lift` recovers a volume from any positive `E`, so a de-retraction and a stationary deposit
        // are the same segment, and this rule judges neither (`docs/14`).
        let is_unretract = s.filament.value() > 0.0 && stationary && s.volume.value() == 0.0;
        if stationary && (is_retract || is_unretract) {
            if let Some(max_speed) = c.max_retraction_speed {
                if s.speed.value() > max_speed {
                    push_finding(
                        &mut r,
                        RuleId::RetractionSpeed,
                        Some(i),
                        format!(
                            "retraction speed {} mm/min exceeds the limit of {}",
                            s.speed.value(),
                            max_speed
                        ),
                    );
                }
            }
        }
        let extrudes_material = !s.travel && s.volume.value() > 0.0;
        if is_retract {
            let dist = -s.filament.value();
            if let (Some(max_dist), true) = (c.max_retraction_distance, stationary) {
                if dist > max_dist {
                    push_finding(
                        &mut r,
                        RuleId::RetractionDistance,
                        Some(i),
                        format!(
                            "retraction distance {dist:.3} mm exceeds the limit of {max_dist:.3}"
                        ),
                    );
                }
            }
            // The retracted/unretracted *state* is tracked on any E-negative move, wipe included: the
            // filament really is pulled back, and `travel-without-retraction` asks about the state, not
            // about which form the retraction took.
            retracted = true;
        } else if is_unretract || extrudes_material {
            retracted = false;
            travel_run_length = 0.0;
            flagged_travel = false;
        } else if s.travel {
            travel_run_length += s.length.value();
            if let Some(max_travel) = c.max_travel_without_retract {
                if travel_run_length > max_travel && !retracted && !flagged_travel {
                    push_finding(
                        &mut r,
                        RuleId::TravelWithoutRetraction,
                        Some(i),
                        format!(
                            "travel run distance {travel_run_length:.3} mm exceeds limit of {max_travel:.3} without retraction"
                        ),
                    );
                    flagged_travel = true;
                }
            }
        }

        // --- first-layer checks ---
        // Both are adhesion advisories about a bead laid on the plate, so both need a bead: a pure
        // filament move on the first layer has no height to compare and no print speed to judge — its
        // `F` is the de-retraction rate, which is how `G1 E1 F2400` was being reported as a first-layer
        // speed of 2400 mm/min.
        if deposits_along_path(&s) {
            let z = s.end[2]
                .or(s.start[2])
                .map(Length::value)
                .filter(|z| z.is_finite());
            let is_first_layer = if let Some(z) = z {
                match first_layer_z {
                    None => {
                        first_layer_z = Some(z);
                        true
                    }
                    Some(current) if z < current - 1e-4 => {
                        // A later, lower layer invalidates provisional findings from the earlier
                        // minimum. Removing only those two rule ids keeps this pass streaming.
                        r.findings.retain(|finding| {
                            finding.rule != RuleId::FirstLayerHeight.as_str()
                                && finding.rule != RuleId::FirstLayerSpeed.as_str()
                        });
                        first_layer_z = Some(z);
                        true
                    }
                    Some(current) => (z - current).abs() < 1e-4,
                }
            } else {
                false
            };

            if is_first_layer {
                if let Some([min_h, max_h]) = c.first_layer_height_range {
                    let h_val = s
                        .height
                        .map(|height| height.value())
                        .or(first_layer_z)
                        .unwrap_or(0.0);
                    if h_val < min_h || h_val > max_h {
                        push_finding(
                            &mut r,
                            RuleId::FirstLayerHeight,
                            Some(i),
                            format!(
                                "first layer height {h_val:.3} mm is outside the range [{min_h:.3}, {max_h:.3}]"
                            ),
                        );
                    }
                }
                if let Some([min_s, max_s]) = c.first_layer_speed_range {
                    let speed_val = s.speed.value();
                    if speed_val < min_s || speed_val > max_s {
                        push_finding(
                            &mut r,
                            RuleId::FirstLayerSpeed,
                            Some(i),
                            format!(
                                "first layer speed {speed_val:.3} mm/min is outside the range [{min_s:.3}, {max_s:.3}]"
                            ),
                        );
                    }
                }
            }
        }

        // --- kinematic checks ---
        if let Some(kin) = &c.kinematics {
            let is_print = deposits_along_path(&s);

            // PeakAcceleration: centripetal acceleration of an arc must not exceed the machine max.
            // a = v² / r  where v is in mm/s and r is the arc radius in mm.
            if let Some(max_a) = kin.max_acceleration_mm_s2 {
                if s.kind == SegmentKind::Arc {
                    if let Some(radius) = arc_radius_mm(&s) {
                        let v = s.speed.value() / 60.0;
                        let a = v * v / radius;
                        if a > max_a {
                            push_finding(
                                &mut r,
                                RuleId::PeakAcceleration,
                                Some(i),
                                format!(
                                    "arc centripetal accel {a:.0} mm/s² exceeds max {max_a:.0}"
                                ),
                            );
                        }
                    }
                }
            }

            // JunctionVelocity: a junction may only be taken as fast as its own **direction change**
            // allows. With `t̂ₐ` the exit tangent of the previous printing segment, `t̂_b` this
            // segment's entry tangent, and `f = cos(φ/2)` their half-angle cosine:
            //
            //     fire iff  min(v_a, v_b) > scv · sqrt((√2 − 1)·f / (1 − f))
            //
            // Both halves are `optimize`'s: `junction_cos_half_angle` is the factor `adaptive_speed`
            // shapes corners with, and `junction_velocity_limit_mm_s` turns it into the allowed corner
            // velocity through the junction-deviation relation, calibrated so a **90° corner is allowed
            // exactly the square-corner velocity the contract names**. One machine limit computed in one
            // place: `adaptive_speed`'s own cap is `scv·f`, which is ≤ this limit everywhere, so a
            // toolpath `balanced` produced always satisfies this rule.
            //
            // `min(v_a, v_b)` because the corner cannot be entered faster than the slower of the two
            // commanded feedrates; firing on the faster one would report a corner the program has
            // already slowed down for.
            //
            // This replaces a *scalar* `‖v_b·t̂_b − v_a·t̂ₐ‖ > scv`, which had no physical model behind
            // its threshold: it treated a shallow deflection and a full reversal as comparable at equal
            // Δv, where the relation above allows a shallow corner far more and a reversal none — the
            // deficiency `docs/11` already recorded. Two consequences, both deliberate: a velocity
            // change *along a straight line* (`f = 1`, an unbounded allowance) no longer fires, because
            // a collinear 10 → 100 mm/s step is an acceleration question and not a cornering one; and a
            // corner between roughly 12° and 21° at 40 mm/s with `scv = 8` stops firing, because the
            // machine can in fact take it.
            //
            // Contiguity is still required (within 0.1 mm) so non-adjacent segments — e.g. across a
            // travel — never produce a false positive.
            let tangents = get_tangents(&s);
            if let (Some(max_jv), Some(pv), Some(pt), true) = (
                kin.max_junction_velocity_mm_s,
                prev_speed_mm_s,
                prev_exit_tangent,
                is_print,
            ) {
                if junction_contiguous(&prev_print_end, &s.start) {
                    if let Some((entry, _)) = tangents {
                        let v_junction = (s.speed.value() / 60.0).min(pv);
                        let cos_half = junction_cos_half_angle(pt, entry);
                        let allowed = junction_velocity_limit_mm_s(max_jv, cos_half);
                        if v_junction > allowed {
                            let turn_deg = 2.0 * libm::acos(cos_half.clamp(-1.0, 1.0)).to_degrees();
                            push_finding(
                                &mut r,
                                RuleId::JunctionVelocity,
                                Some(i),
                                format!(
                                    "junction turns {turn_deg:.1}° and is entered at {v_junction:.1} mm/s, \
                                     above the {allowed:.1} mm/s it allows at square-corner velocity {max_jv:.1}"
                                ),
                            );
                        }
                    }
                }
            }

            if is_print {
                prev_print_end = Some(s.end);
                prev_speed_mm_s = Some(s.speed.value() / 60.0);
                prev_exit_tangent = tangents.map(|(_, exit)| exit);
            } else if s.travel {
                // Reset junction tracking across travel moves.
                prev_print_end = None;
                prev_speed_mm_s = None;
                prev_exit_tangent = None;
            }
        }

        // --- rotary / 5-axis checks ---
        //
        // All three resolve the segment's toolframe orientation through the *same* `Kinematics` the
        // emitter uses, so what is judged here is the program that will be written rather than a
        // second derivation of it. An absent orientation is the identity (+Z), exactly as
        // `Kinematics::rotary_words` reads it — a 3-axis segment under a 5-axis model still commands
        // rotary words, and the emitter still writes them when they change.
        //
        // An orientation the model cannot resolve at all (zero or non-finite) is skipped: `finite` and
        // `orientation-not-unit` already say so, and a second finding on the same defect would only
        // dilute the first.
        if let Some(rot) = &c.rotary {
            if s.kind == SegmentKind::ManualGcode {
                prev_rotary = None;
            } else if let Ok(joints) = rot.model.resolve_joints(s.orientation, &mut rotary_state) {
                {
                    let words = rot.model.rotary_words(joints);
                    if let Some(travel) = &rot.travel_deg {
                        for w in words.iter() {
                            if let Some([lo, hi]) = travel.range(w.letter) {
                                if w.value < lo || w.value > hi {
                                    push_finding(
                                        &mut r,
                                        RuleId::RotaryTravel,
                                        Some(i),
                                        format!(
                                            "rotary axis {} is commanded to {:.3}°, outside its \
                                             travel range [{lo}, {hi}]°",
                                            w.letter, w.value
                                        ),
                                    );
                                }
                            }
                        }
                    }

                    // The rotary axes turn *during* this move: the emitter writes the rotary words on
                    // the same line as the linear endpoint, so the sweep and the motion share one
                    // duration. A segment Dry cannot time (zero length and no filament, or a
                    // non-positive feedrate) states no duration to divide by and is skipped — recorded
                    // rather than guessed at, because a zero-time reorientation is a modelling gap
                    // rather than a machine-limit violation.
                    if let (Some(max_rate), Some(prev), Some(time)) = (
                        rot.max_rotary_feed_deg_min,
                        prev_rotary,
                        segment_motion_time(&s),
                    ) {
                        let minutes = time.value() / 60.0;
                        if minutes > 0.0 {
                            for (w, from) in words.iter().zip(prev) {
                                let sweep = (w.value - from).abs();
                                let rate = sweep / minutes;
                                if rate > max_rate {
                                    push_finding(
                                        &mut r,
                                        RuleId::RotaryFeed,
                                        Some(i),
                                        format!(
                                            "rotary axis {} must sweep {sweep:.3}° in {:.4} s, a rate \
                                             of {rate:.0} °/min over the limit of {max_rate:.0}",
                                            w.letter,
                                            time.value()
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    prev_rotary = Some([words[0].value, words[1].value]);

                    // Reachability: where the rotation actually puts the tool. `bounds` checks the
                    // programmed workpiece coordinates; this checks the machine coordinates the same
                    // transform the emitter applies produces from them. Endpoints only — an arc's
                    // swept interior is not mapped, so this under-reports rather than over-reports.
                    if let Some(env) = rot.envelope_mm {
                        'rotary_points: for point in [s.start, s.end] {
                            let (Some(x), Some(y), Some(z)) = (point[0], point[1], point[2]) else {
                                continue; // an undefined axis inherits; no position is asserted
                            };
                            let p = [x.value(), y.value(), z.value()];
                            let machine = rot.model.machine_position(p, joints);
                            for (k, value) in machine.iter().enumerate() {
                                if *value < env[k][0] || *value > env[k][1] {
                                    let [wa, wb] = &words;
                                    push_finding(
                                        &mut r,
                                        RuleId::OrientationReachability,
                                        Some(i),
                                        format!(
                                            "at {}{:.3} {}{:.3} the point [{}, {}, {}] sits at \
                                             machine {} = {value:.3}, outside the reachable envelope \
                                             [{}, {}]",
                                            wa.letter,
                                            wa.value,
                                            wb.letter,
                                            wb.value,
                                            p[0],
                                            p[1],
                                            p[2],
                                            axis[k],
                                            env[k][0],
                                            env[k][1]
                                        ),
                                    );
                                    break 'rotary_points; // one reachability finding per segment
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(r)
}

/// Verify a resolved [`Toolpath`] against machine-safety **contracts** and structural
/// invariants, returning a located [`Report`].
pub fn verify(tp: &Toolpath, c: &Contracts) -> Report {
    verify_stream(tp.segments.iter().cloned().map(Ok), c).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{Feedrate, Volume};

    #[test]
    fn contract_csv_parsers_reject_inverted_ranges() {
        for (input, axis) in [
            ("1,0,0,1,0,1", "x"),
            ("0,1,1,0,0,1", "y"),
            ("0,1,0,1,1,0", "z"),
        ] {
            let error = parse_bounds_csv(input).unwrap_err().to_string();
            assert_eq!(
                error,
                format!("bounds {axis} lower bound must be <= upper bound")
            );
        }

        assert_eq!(
            parse_speed_range_csv("9000,300").unwrap_err().to_string(),
            "speed range lower bound must be <= upper bound"
        );
    }

    #[test]
    fn contract_csv_parsers_allow_equal_endpoints() {
        assert_eq!(
            parse_bounds_csv("1,1,2,2,3,3").unwrap(),
            [[1.0, 1.0], [2.0, 2.0], [3.0, 3.0]]
        );
        assert_eq!(parse_speed_range_csv("600,600").unwrap(), [600.0, 600.0]);
    }

    /// A single arc segment with the given radius (mm) and speed (mm/min), valid geometry.
    fn arc_toolpath(radius_mm: f64, speed_mm_min: f64) -> Toolpath {
        // Centre at origin; start at (radius, 0), end at (0, radius) — a CCW quarter-circle.
        // Both start and end radii equal radius_mm, so no arc-radius error fires.
        Toolpath {
            version: 0,
            meta: None,
            segments: vec![Segment {
                start: [
                    Some(Length::mm(radius_mm)),
                    Some(Length::mm(0.0)),
                    Some(Length::mm(0.2)),
                ],
                end: [
                    Some(Length::mm(0.0)),
                    Some(Length::mm(radius_mm)),
                    Some(Length::mm(0.2)),
                ],
                speed: Feedrate(speed_mm_min),
                length: Length::mm(radius_mm * std::f64::consts::FRAC_PI_2),
                volume: Volume(0.8),
                filament: Length::mm(0.33),
                width: Some(Length::mm(0.4)),
                height: Some(Length::mm(0.2)),
                kind: SegmentKind::Arc,
                centre: Some([Length::mm(0.0), Length::mm(0.0)]),
                clockwise: false,
                travel: false,
                temperature: Some(210.0),
                fan: None,
                flow: None,
                tool: None,
                power: None,
                dwell_s: None,
                manual_gcode: None,
                orientation: None,
                control_points: None,
            }],
        }
    }

    /// Two contiguous printing line segments (end of seg0 == start of seg1), at different speeds.
    fn two_segment_junction(v0_mm_min: f64, v1_mm_min: f64) -> Toolpath {
        let seg0 = Segment {
            start: [
                Some(Length::mm(0.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            speed: Feedrate(v0_mm_min),
            length: Length::mm(10.0),
            volume: Volume(0.8),
            filament: Length::mm(0.33),
            width: Some(Length::mm(0.4)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            travel: false,
            temperature: Some(210.0),
            fan: None,
            flow: None,
            tool: None,
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: None,
            control_points: None,
        };
        let seg1 = Segment {
            start: [
                Some(Length::mm(10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(10.0)),
                Some(Length::mm(10.0)),
                Some(Length::mm(0.2)),
            ],
            speed: Feedrate(v1_mm_min),
            ..seg0.clone()
        };
        Toolpath {
            version: 0,
            meta: None,
            segments: vec![seg0, seg1],
        }
    }

    #[test]
    fn arc_over_centripetal_limit_is_a_peak_acceleration_error() {
        // Arc of radius 5 mm at 6000 mm/min → v = 100 mm/s → a = v²/r = 2000 mm/s² > 1000.
        let tp = arc_toolpath(5.0, 6000.0);
        let c = Contracts {
            kinematics: Some(KinematicContracts {
                max_acceleration_mm_s2: Some(1000.0),
                max_junction_velocity_mm_s: None,
            }),
            ..Contracts::default()
        };
        let report = verify(&tp, &c);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "peak-acceleration" && f.severity == Severity::Error),
            "expected peak-acceleration Error, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn junction_over_scv_is_a_junction_velocity_warning() {
        // Two contiguous printing segments: v0 = 600 mm/min (10 mm/s), v1 = 6000 mm/min (100 mm/s).
        // Δv = 90 mm/s, limit = 5 mm/s → junction-velocity Warning.
        let tp = two_segment_junction(600.0, 6000.0);
        let c = Contracts {
            kinematics: Some(KinematicContracts {
                max_acceleration_mm_s2: None,
                max_junction_velocity_mm_s: Some(5.0),
            }),
            ..Contracts::default()
        };
        let report = verify(&tp, &c);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == "junction-velocity" && f.severity == Severity::Warning),
            "expected junction-velocity Warning, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn no_kinematics_means_no_kinematic_findings() {
        let tp = arc_toolpath(5.0, 6000.0);
        let report = verify(&tp, &Contracts::default());
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.rule == "peak-acceleration" || f.rule == "junction-velocity"),
            "expected no kinematic findings, got: {:?}",
            report.findings
        );
    }

    #[test]
    fn catalog_includes_the_two_kinematic_rules() {
        let cat = catalog();
        let pa = cat
            .iter()
            .find(|r| r.id == RuleId::PeakAcceleration)
            .expect("peak-acceleration in catalog");
        assert_eq!(pa.severity, Severity::Error);
        assert_eq!(RuleId::PeakAcceleration.as_str(), "peak-acceleration");
        let jv = cat
            .iter()
            .find(|r| r.id == RuleId::JunctionVelocity)
            .expect("junction-velocity in catalog");
        assert_eq!(jv.severity, Severity::Warning);
        assert_eq!(RuleId::JunctionVelocity.as_str(), "junction-velocity");
    }

    #[test]
    fn contracts_default_has_no_kinematics() {
        assert!(Contracts::default().kinematics.is_none());
    }

    #[test]
    fn empty_toolpath_is_ok_but_vacuously_so() {
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: vec![],
        };
        let report = verify(&tp, &Contracts::default());
        assert!(report.ok());
        // This is the canonical vacuous pass, so name it as one: `ok()` here means "nothing was
        // found wrong with nothing", and until H1.3 that was byte-identical to a real clean report.
        assert_eq!(report.segments_inspected, 0);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn manual_gcode_is_always_reported_as_unmodeled() {
        let mut tp = two_segment_junction(600.0, 600.0);
        tp.segments.truncate(1);
        tp.segments[0].kind = SegmentKind::ManualGcode;
        tp.segments[0].manual_gcode = Some("M84".to_string());
        let report = verify(&tp, &Contracts::default());
        assert!(report.findings.iter().any(|finding| {
            finding.rule == "unmodeled-gcode" && finding.severity == Severity::Warning
        }));
    }

    #[test]
    fn later_lower_layer_replaces_provisional_first_layer_findings() {
        let mut tp = two_segment_junction(600.0, 600.0);
        tp.segments[0].start[2] = Some(Length::mm(0.3));
        tp.segments[0].end[2] = Some(Length::mm(0.3));
        tp.segments[0].height = Some(Length::mm(0.5));
        tp.segments[1].start[2] = Some(Length::mm(0.2));
        tp.segments[1].end[2] = Some(Length::mm(0.2));
        tp.segments[1].height = Some(Length::mm(0.2));
        let contracts = Contracts {
            first_layer_height_range: Some([0.1, 0.3]),
            ..Contracts::default()
        };
        let report = verify(&tp, &contracts);
        assert!(
            report
                .findings
                .iter()
                .all(|finding| finding.rule != "first-layer-height"),
            "only the true minimum-Z layer should be checked: {:?}",
            report.findings
        );
    }

    #[test]
    fn rule_catalog_is_consistent() {
        let cat = catalog();
        assert_eq!(cat.len(), RuleId::ALL.len());
        for rule in &cat {
            // wire id round-trips and is unique-mapping
            assert_eq!(RuleId::from_wire(rule.id.as_str()), Some(rule.id));
            assert!(!rule.summary.is_empty());
            assert_eq!(rule.severity, rule.id.default_severity());
        }
        // process/quality advisories are warnings; everything else is an error.
        let warnings: Vec<&str> = cat
            .iter()
            .filter(|r| r.severity == Severity::Warning)
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(
            warnings,
            vec![
                // The IR's travel flag disagreeing with its deposited volume is a modelling
                // inconsistency, not an unsafe program: see `default_severity` for why it is a
                // warning globally rather than only for imported IR.
                "travel-extrudes",
                "travel-without-retraction",
                "first-layer-height",
                "first-layer-speed",
                "junction-velocity",
                "unmodeled-gcode",
                // Staged: promoted to Error one minor release after landing (design §8).
                "filament-consistency",
                "rotary-feed",
            ]
        );
    }

    /// Pins the always-on rule set exactly, so the structural baseline cannot drift silently the way
    /// "5 of 18" did before H1.3. A rule joining or leaving this list changes what `Report::ok()`
    /// means for every caller that supplies no contracts, which is a decision, not a detail.
    #[test]
    fn contracts_default_evaluates_only_structural_rules() {
        let c = Contracts::default();
        let evaluated: Vec<&str> = RuleId::ALL
            .into_iter()
            .filter(|r| r.is_evaluated(&c))
            .map(|r| r.as_str())
            .collect();
        assert_eq!(
            evaluated,
            vec![
                "finite",
                "travel-extrudes",
                "bead",
                "orientation-not-unit",
                "arc-radius",
                "unmodeled-gcode",
                "continuity",
                "negative-quantity",
                "segment-length",
                "arc-length",
                "filament-consistency",
            ],
            "the always-on structural set changed"
        );

        // Of those, the ones that can flip `ok()`. Before H1.3 this was 5 of 18; H1.3 took it to 9 of
        // 11, and downgrading `travel-extrudes` to a warning takes it to 8 — a rule leaving this
        // count is the same decision as one joining it. `laser-power-during-travel` briefly made it
        // 9 while it was always-on; process-gating it returns the always-on error set to 8.
        let can_fail: Vec<&str> = evaluated
            .iter()
            .copied()
            .filter(|id| RuleId::from_wire(id).unwrap().default_severity() == Severity::Error)
            .collect();
        assert_eq!(
            can_fail.len(),
            8,
            "error-severity always-on rules: {can_fail:?}"
        );
        assert_eq!(RuleId::ALL.len(), 28);
    }

    #[test]
    fn a_fully_populated_contract_evaluates_every_rule() {
        let c = Contracts {
            bounds: Some([[0.0, 100.0]; 3]),
            max_flow: Some(10.0),
            speed_range: Some([100.0, 6000.0]),
            monotonic_z: true,
            min_temp: Some(180.0),
            max_retraction_distance: Some(5.0),
            max_retraction_speed: Some(3000.0),
            max_travel_without_retract: Some(20.0),
            first_layer_height_range: Some([0.1, 0.4]),
            first_layer_speed_range: Some([100.0, 2000.0]),
            bead_volume_tolerance: Some(0.01),
            kinematics: Some(KinematicContracts {
                max_acceleration_mm_s2: Some(500.0),
                max_junction_velocity_mm_s: Some(8.0),
            }),
            rotary: Some(crate::emit::REFERENCE_FIVE_AXIS_LIMITS),
            travel_must_be_dark: Some(true),
        };
        assert!(RuleId::ALL.into_iter().all(|r| r.is_evaluated(&c)));

        // The point of `rules_evaluated`: "clean" under 12 rules is a different claim from "clean"
        // under 28, and until H1.3 the two reports were byte-identical.
        let tp = Toolpath {
            version: 0,
            meta: None,
            segments: Vec::new(),
        };
        assert_eq!(verify(&tp, &Contracts::default()).rules_evaluated.len(), 11);
        assert_eq!(verify(&tp, &c).rules_evaluated.len(), 28);
    }

    /// A rotary contract that states a limit but not the one a rule needs must leave that rule
    /// *unevaluated*, not silently passing: an all-empty travel table checks no axis.
    #[test]
    fn rotary_rules_are_evaluated_only_where_a_limit_is_supplied() {
        let travel_only = Contracts {
            rotary: Some(RotaryContracts {
                travel_deg: Some(RotaryTravelRanges {
                    b: Some([0.0, 120.0]),
                    ..RotaryTravelRanges::default()
                }),
                ..RotaryContracts::default()
            }),
            ..Contracts::default()
        };
        assert!(RuleId::RotaryTravel.is_evaluated(&travel_only));
        assert!(!RuleId::RotaryFeed.is_evaluated(&travel_only));
        assert!(!RuleId::OrientationReachability.is_evaluated(&travel_only));

        let empty_table = Contracts {
            rotary: Some(RotaryContracts {
                travel_deg: Some(RotaryTravelRanges::default()),
                ..RotaryContracts::default()
            }),
            ..Contracts::default()
        };
        assert!(!RuleId::RotaryTravel.is_evaluated(&empty_table));
    }

    /// A single extruding move at `speed`, carrying `orientation`, from `x0` to `x0 + 10` at z = 0.2.
    fn oriented_move(x0: f64, orientation: [f64; 3], speed_mm_min: f64) -> Segment {
        Segment {
            start: [
                Some(Length::mm(x0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            end: [
                Some(Length::mm(x0 + 10.0)),
                Some(Length::mm(0.0)),
                Some(Length::mm(0.2)),
            ],
            speed: Feedrate(speed_mm_min),
            length: Length::mm(10.0),
            volume: Volume(0.8),
            filament: Length::mm(0.33),
            width: Some(Length::mm(0.4)),
            height: Some(Length::mm(0.2)),
            kind: SegmentKind::Line,
            centre: None,
            clockwise: false,
            travel: false,
            temperature: Some(210.0),
            fan: None,
            flow: None,
            tool: None,
            power: None,
            dwell_s: None,
            manual_gcode: None,
            orientation: Some(orientation),
            control_points: None,
        }
    }

    fn tp_of(segments: Vec<Segment>) -> Toolpath {
        Toolpath {
            version: 0,
            meta: None,
            segments,
        }
    }

    fn rules_fired(report: &Report, rule: RuleId) -> Vec<&Finding> {
        report
            .findings
            .iter()
            .filter(|f| f.rule == rule.as_str())
            .collect()
    }

    /// Under the reference machine, a tool pointing at −Z asks the B axis for `acos(-1)` = 180°, which
    /// is past the 120° the trunnion can tilt. A tool pointing at +X asks for 90°, which is not.
    #[test]
    fn tilt_beyond_the_reference_trunnion_is_a_rotary_travel_error() {
        let c = Contracts {
            rotary: Some(crate::emit::REFERENCE_FIVE_AXIS_LIMITS),
            ..Contracts::default()
        };

        let over = verify(
            &tp_of(vec![oriented_move(0.0, [0.0, 0.0, -1.0], 600.0)]),
            &c,
        );
        let fired = rules_fired(&over, RuleId::RotaryTravel);
        assert_eq!(
            fired.len(),
            1,
            "expected one finding, got {:?}",
            over.findings
        );
        assert_eq!(fired[0].severity, Severity::Error);

        let within = verify(&tp_of(vec![oriented_move(0.0, [1.0, 0.0, 0.0], 600.0)]), &c);
        assert!(
            rules_fired(&within, RuleId::RotaryTravel).is_empty(),
            "90 degrees of tilt is inside [0, 120]: {:?}",
            within.findings
        );
    }

    /// The rotary axes turn during the move, so the same 90° reorientation is fine over ten seconds
    /// and impossible over a tenth of one. Nothing about the *geometry* differs between the two.
    #[test]
    fn a_reorientation_faster_than_the_axis_can_turn_is_a_rotary_feed_warning() {
        let c = Contracts {
            rotary: Some(crate::emit::REFERENCE_FIVE_AXIS_LIMITS),
            ..Contracts::default()
        };
        let vertical = oriented_move(0.0, [0.0, 0.0, 1.0], 600.0);

        // 10 mm at 6000 mm/min = 0.1 s for 90° → 54 000 °/min, far over the 3 600 °/min limit.
        let fast = verify(
            &tp_of(vec![
                vertical.clone(),
                oriented_move(10.0, [1.0, 0.0, 0.0], 6000.0),
            ]),
            &c,
        );
        let fired = rules_fired(&fast, RuleId::RotaryFeed);
        assert_eq!(
            fired.len(),
            1,
            "expected one finding, got {:?}",
            fast.findings
        );
        assert_eq!(fired[0].severity, Severity::Warning);
        assert_eq!(fired[0].segment, Some(1));

        // The same 90° at 60 mm/min takes 10 s → 540 °/min, well inside the limit.
        let slow = verify(
            &tp_of(vec![vertical, oriented_move(10.0, [1.0, 0.0, 0.0], 60.0)]),
            &c,
        );
        assert!(
            rules_fired(&slow, RuleId::RotaryFeed).is_empty(),
            "540 deg/min is inside the limit: {:?}",
            slow.findings
        );
    }

    /// Reachability is a property of the point *and* the orientation together: tilting the table 90°
    /// swings a point 100 mm out in X down to Z = −100 in machine coordinates, below the reference
    /// machine's −50 floor. The identical orientation 10 mm from the origin is fine.
    #[test]
    fn tilting_a_far_out_point_below_the_table_is_an_orientation_reachability_error() {
        let c = Contracts {
            rotary: Some(crate::emit::REFERENCE_FIVE_AXIS_LIMITS),
            ..Contracts::default()
        };

        let unreachable = verify(
            &tp_of(vec![oriented_move(100.0, [1.0, 0.0, 0.0], 60.0)]),
            &c,
        );
        let fired = rules_fired(&unreachable, RuleId::OrientationReachability);
        assert_eq!(
            fired.len(),
            1,
            "one finding per segment, got {:?}",
            unreachable.findings
        );
        assert_eq!(fired[0].severity, Severity::Error);

        let reachable = verify(&tp_of(vec![oriented_move(0.0, [1.0, 0.0, 0.0], 60.0)]), &c);
        assert!(
            rules_fired(&reachable, RuleId::OrientationReachability).is_empty(),
            "a point near the origin stays inside the envelope: {:?}",
            reachable.findings
        );
    }

    /// The whole point of gating: a 3-axis report must be unchanged. The same toolpath that trips all
    /// three rotary rules under the reference limits produces none of them under no rotary contract,
    /// and none of the three appear in `rules_evaluated`.
    #[test]
    fn no_rotary_contract_means_no_rotary_findings() {
        let tp = tp_of(vec![
            oriented_move(0.0, [0.0, 0.0, 1.0], 600.0),
            oriented_move(10.0, [0.0, 0.0, -1.0], 6000.0),
            oriented_move(100.0, [1.0, 0.0, 0.0], 60.0),
        ]);
        let with_limits = verify(
            &tp,
            &Contracts {
                rotary: Some(crate::emit::REFERENCE_FIVE_AXIS_LIMITS),
                ..Contracts::default()
            },
        );
        for rule in [
            RuleId::RotaryTravel,
            RuleId::RotaryFeed,
            RuleId::OrientationReachability,
        ] {
            assert!(
                !rules_fired(&with_limits, rule).is_empty(),
                "{} should fire under the reference limits: {:?}",
                rule.as_str(),
                with_limits.findings
            );
        }

        let bare = verify(&tp, &Contracts::default());
        for rule in [
            RuleId::RotaryTravel,
            RuleId::RotaryFeed,
            RuleId::OrientationReachability,
        ] {
            assert!(rules_fired(&bare, rule).is_empty());
            assert!(!bare.evaluated(rule));
        }
    }
}
