//! # kmet-contracts — the shared vocabulary
//!
//! The types the kernel and the verifier both name: verification contracts, rule identifiers,
//! severities, and the kinematic model enum. Deliberately logic-free and deliberately below both, so
//! `kmet-kernel` and `kmet-verify` can be separate crates without a cycle.
//!
//! See `docs/superpowers/specs/2026-08-25-ip-registration-and-preservation-design.md` §5.7.
//!
//! Everything here moved verbatim out of `dry-core` (`crates/core/src/verify.rs` and
//! `crates/core/src/emit/kinematics.rs`), which re-exports it all from its former paths. Three
//! intra-doc links below were demoted to plain code spans because their targets — `Report`,
//! `Report::rules_evaluated`, `REFERENCE_FIVE_AXIS_LIMITS` — stayed behind; nothing else changed.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

/// The limits a toolpath is checked against. An unset (`None`/`false`) field disables that check.
///
/// This is `Serialize` as well as `Deserialize` because `Report` echoes the contracts it ran under
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
fn reference_rotary_model() -> Kinematics {
    REFERENCE_FIVE_AXIS_MACHINE
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
    pub model: Kinematics,
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
    /// A retraction/unretraction speed exceeds the limit.
    RetractionSpeed,
    /// A travel run exceeds the allowed distance without a retraction (stringing risk — advisory).
    TravelWithoutRetraction,
    /// First-layer height is outside the allowed range (adhesion — advisory).
    FirstLayerHeight,
    /// First-layer speed is outside the allowed range (adhesion — advisory).
    FirstLayerSpeed,
    /// An arc's centripetal acceleration exceeds the machine's max acceleration.
    PeakAcceleration,
    /// A junction's velocity change exceeds the machine's square-corner velocity.
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
}

impl RuleId {
    /// Every rule, in catalog order.
    pub const ALL: [RuleId; 27] = [
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
                "a junction's velocity change exceeds the machine's square-corner velocity"
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
        }
    }

    /// Whether this rule is evaluated at all under `c` — i.e. whether it is structural (always on)
    /// or its gating contract supplies a limit.
    ///
    /// This is what makes a vacuous pass visible: `Report::rules_evaluated` is built from it, so a
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
            RuleId::Bounds => c.bounds.is_some(),
            RuleId::MaxFlow => c.max_flow.is_some(),
            RuleId::Speed => c.speed_range.is_some(),
            RuleId::MonotonicZ => c.monotonic_z,
            RuleId::ColdExtrusion => c.min_temp.is_some(),
            RuleId::RetractionDistance => c.max_retraction_distance.is_some(),
            RuleId::RetractionSpeed => c.max_retraction_speed.is_some(),
            RuleId::TravelWithoutRetraction => c.max_travel_without_retract.is_some(),
            RuleId::FirstLayerHeight => c.first_layer_height_range.is_some(),
            RuleId::FirstLayerSpeed => c.first_layer_speed_range.is_some(),
            RuleId::BeadVolume => c.bead_volume_tolerance.is_some(),
            RuleId::PeakAcceleration => c
                .kinematics
                .as_ref()
                .is_some_and(|k| k.max_acceleration_mm_s2.is_some()),
            RuleId::JunctionVelocity => c
                .kinematics
                .as_ref()
                .is_some_and(|k| k.max_junction_velocity_mm_s.is_some()),
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

/// Relative tolerance for `arc-radius`: how far `|end − centre|` may differ from `|start − centre|`
/// before the two contradict each other.
///
/// The single definition in the tree. `resolve` applies the same epsilon at the L1 gate and imports
/// it from here rather than restating it — two literals could be retuned apart, and the published
/// boundary `FM1.F64.VERIFY.ARC_RADIUS` can only pin one of them.
pub const ARC_RADIUS_TOLERANCE_MM: f64 = 1e-6;

/// The rotary kinematics of the 5-axis machine: which two rotary axes carry the toolframe orientation,
/// and how the tool-direction unit vector maps onto them. Supports mechanical TCP (Tool Center Point)
/// translation offsets and rotary joint rotation offsets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kinematics {
    /// Tilting head: `A` about X then `B` about Y. Words `A`,`B`.
    Ab {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
    /// `A` about X, `C` about Z (e.g. table/trunnion). Words `A`,`C`.
    Ac {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
    /// `B` about Y, `C` about Z. Words `B`,`C`.
    Bc {
        pivot_offset: [f64; 3],
        rotary_offset: [f64; 2],
    },
}

/// Reference machine model used for the 5-axis task: B/C rotary axes with zero offsets.
///
/// The offsets stay zero deliberately: a zero-pivot table is the one configuration whose forward
/// transform is exactly a rotation about the WCS origin, so every emitted 5-axis program is
/// reproducible without a machine-specific calibration. What the model does *not* carry is any
/// limit — see `REFERENCE_FIVE_AXIS_LIMITS` for those.
pub const REFERENCE_FIVE_AXIS_MACHINE: Kinematics = Kinematics::Bc {
    pivot_offset: [0.0, 0.0, 0.0],
    rotary_offset: [0.0, 0.0],
};

impl Default for Kinematics {
    fn default() -> Self {
        Kinematics::Ab {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, 0.0],
        }
    }
}

impl Kinematics {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Ab {
                pivot_offset,
                rotary_offset,
            }
            | Self::Ac {
                pivot_offset,
                rotary_offset,
            }
            | Self::Bc {
                pivot_offset,
                rotary_offset,
            } => {
                for (axis, value) in ["x", "y", "z"].iter().zip(*pivot_offset) {
                    if !value.is_finite() {
                        return Err(format!("pivot_offset[{axis}] must be finite"));
                    }
                }
                for (axis, value) in ["0", "1"].iter().zip(*rotary_offset) {
                    if !value.is_finite() {
                        return Err(format!("rotary_offset[{axis}] must be finite"));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn named(name: &str) -> Result<Self, String> {
        match name {
            "ab" => Ok(Kinematics::Ab {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            }),
            "ac" => Ok(Kinematics::Ac {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            }),
            "bc" => Ok(Kinematics::Bc {
                pivot_offset: [0.0, 0.0, 0.0],
                rotary_offset: [0.0, 0.0],
            }),
            other => Err(format!("unknown kinematics: {other}")),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Kinematics {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawKinematics {
            String(String),
            Struct(RawKinematicsStruct),
        }

        #[derive(Deserialize)]
        struct RawKinematicsStruct {
            #[serde(rename = "type")]
            kind: String,
            #[serde(default)]
            pivot_offset: [f64; 3],
            #[serde(default)]
            rotary_offset: [f64; 2],
        }

        let raw = RawKinematics::deserialize(deserializer)?;
        match raw {
            RawKinematics::String(s) => match s.as_str() {
                "ab" | "ac" | "bc" => Kinematics::named(&s).map_err(D::Error::custom),
                other => Err(D::Error::custom(format!("unknown kinematics: {other}"))),
            },
            RawKinematics::Struct(s) => match s.kind.as_str() {
                "ab" => Ok(Kinematics::Ab {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                "ac" => Ok(Kinematics::Ac {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                "bc" => Ok(Kinematics::Bc {
                    pivot_offset: s.pivot_offset,
                    rotary_offset: s.rotary_offset,
                }),
                other => Err(D::Error::custom(format!(
                    "unknown kinematics type: {other}"
                ))),
            },
        }
    }
}

impl Serialize for Kinematics {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Raw {
            #[serde(rename = "type")]
            kind: &'static str,
            #[serde(default)]
            pivot_offset: [f64; 3],
            #[serde(default)]
            rotary_offset: [f64; 2],
        }

        match self {
            Self::Ab {
                pivot_offset,
                rotary_offset,
            } => Raw {
                kind: "ab",
                pivot_offset: *pivot_offset,
                rotary_offset: *rotary_offset,
            }
            .serialize(serializer),
            Self::Ac {
                pivot_offset,
                rotary_offset,
            } => Raw {
                kind: "ac",
                pivot_offset: *pivot_offset,
                rotary_offset: *rotary_offset,
            }
            .serialize(serializer),
            Self::Bc {
                pivot_offset,
                rotary_offset,
            } => Raw {
                kind: "bc",
                pivot_offset: *pivot_offset,
                rotary_offset: *rotary_offset,
            }
            .serialize(serializer),
        }
    }
}
