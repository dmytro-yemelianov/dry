//! Which segments each verifier rule *applies to* — the scope questions that the first real user file
//! (a 123,613-line OrcaSlicer 2.4.2 slice for a Creality Ender-3 S1) settled by producing 2,310 errors
//! on a healthy print.
//!
//! Every fixture here is hand-written G-code imported through `gcode::lift`, not a hand-built
//! `Segment`: the defects were all in the *inferred* IR shape a slicer file produces — `travel` from
//! "`G0`, or no `E` word", a `volume` recovered from any positive `E`, a geometric length of zero on a
//! pure filament move — so a fixture that skipped the importer would not have caught any of them.
//!
//! Four classes, each with its negative *and* its positive control, because a scope fix that simply
//! stops a rule firing is indistinguishable from deleting it:
//!
//!  1. `max-flow` on an E-only unretract (must not fire) vs. a genuinely over-flowing move (must).
//!  2. `retraction-speed`/`-distance` on a wipe-while-retracting (must not) vs. a pure retract (must).
//!  3. `junction-velocity` on a constant-speed 90° corner (must fire — the case the rule is named for).
//!  4. `junction-velocity` on a collinear speed change (must not — that is acceleration, not cornering).

use dry_core::verify::{Contracts, KinematicContracts, RuleId};
use dry_core::{import_gcode, verify, GcodeImportParams, Report};

fn params() -> GcodeImportParams {
    GcodeImportParams {
        // Relative E, as every modern slicer emits (`M83`), so each `E` word is its own delta.
        relative_e: true,
        line_width: Some(0.42),
        layer_height: Some(0.2),
        ..GcodeImportParams::default()
    }
}

fn review(source: &str, c: &Contracts) -> Report {
    let tp = import_gcode(source, &params()).expect("fixture imports");
    verify(&tp, c)
}

fn count(r: &Report, rule: RuleId) -> usize {
    r.findings
        .iter()
        .filter(|f| f.rule == rule.as_str())
        .count()
}

fn messages(r: &Report, rule: RuleId) -> Vec<&str> {
    r.findings
        .iter()
        .filter(|f| f.rule == rule.as_str())
        .map(|f| f.message.as_str())
        .collect()
}

/// A 1.75 mm filament has a 2.4053 mm² cross-section, so `G1 E1 F2400` moves 1 mm of feedstock in
/// 1/40 s — 96.2 mm³/s if you score it as a deposition rate, against any real ceiling of ~12.
///
/// It deposits nothing: the filament is being staged into the melt zone before the next extrusion.
/// This is the shape of all 813 `max-flow` errors the pilot file raised, every one carrying that same
/// 96.211 mm³/s.
#[test]
fn max_flow_ignores_an_e_only_unretract() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 E1 F2400
G1 X10 Y0 E0.42 F2400
";
    let c = Contracts {
        max_flow: Some(12.0),
        ..Contracts::default()
    };
    let r = review(gcode, &c);
    assert!(r.evaluated(RuleId::MaxFlow), "the ceiling must be in force");
    assert_eq!(
        count(&r, RuleId::MaxFlow),
        0,
        "an E-only unretract deposits along no path: {:?}",
        messages(&r, RuleId::MaxFlow)
    );
}

/// The positive control for the fix above: a move that really does traverse a path while depositing
/// more than the ceiling allows still fails. 0.42 mm × 0.2 mm bead over 10 mm at 6000 mm/min is
/// 10 mm³ in 0.1 s = 100 mm³/s.
#[test]
fn max_flow_still_fires_on_a_real_over_flowing_move() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E4.16 F6000
";
    let c = Contracts {
        max_flow: Some(12.0),
        ..Contracts::default()
    };
    let r = review(gcode, &c);
    assert_eq!(
        count(&r, RuleId::MaxFlow),
        1,
        "expected one max-flow error, got {:?}",
        messages(&r, RuleId::MaxFlow)
    );
}

/// OrcaSlicer's wipe: XY motion with simultaneous retraction, at the *wipe* feedrate. `F3000` is the
/// speed the nozzle sweeps the surface at, not a speed the extruder is being driven at, and the `E`
/// delta is the `retract_before_wipe` remainder rather than a whole retraction. ~1,500 of the pilot
/// file's retraction errors were this move.
#[test]
fn retraction_rules_ignore_a_wipe_while_retracting() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F2400
G1 X9 Y0.5 E-0.3 F3000
";
    let c = Contracts {
        max_retraction_speed: Some(1800.0),
        max_retraction_distance: Some(0.1),
        ..Contracts::default()
    };
    let r = review(gcode, &c);
    assert!(r.evaluated(RuleId::RetractionSpeed));
    assert!(r.evaluated(RuleId::RetractionDistance));
    assert_eq!(
        (
            count(&r, RuleId::RetractionSpeed),
            count(&r, RuleId::RetractionDistance)
        ),
        (0, 0),
        "a wipe is not a retraction: {:?} {:?}",
        messages(&r, RuleId::RetractionSpeed),
        messages(&r, RuleId::RetractionDistance)
    );
}

/// The positive control: the stationary `G1 E-1 F3600` beside that wipe is a pure retraction, and both
/// limits still judge it. `retraction-distance` needs the *stationary* form to be measured at all,
/// which is exactly the coverage note in `docs/14`.
#[test]
fn retraction_rules_still_fire_on_a_pure_retract() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F2400
G1 E-1 F3600
";
    let c = Contracts {
        max_retraction_speed: Some(1800.0),
        max_retraction_distance: Some(0.8),
        ..Contracts::default()
    };
    let r = review(gcode, &c);
    assert_eq!(
        count(&r, RuleId::RetractionSpeed),
        1,
        "expected the 3600 mm/min retract to be flagged: {:?}",
        messages(&r, RuleId::RetractionSpeed)
    );
    assert_eq!(
        count(&r, RuleId::RetractionDistance),
        1,
        "expected the 1 mm retract to exceed 0.8 mm: {:?}",
        messages(&r, RuleId::RetractionDistance)
    );
}

/// A deliberate limitation, pinned so it cannot be lost by accident: an *imported* de-retraction is
/// **not** speed-judged. `G1 E1 F6000` is 100 mm/s of filament and would be flagged if the rule could
/// see it, but `lift` recovers a volume from any positive `E`, which makes this segment byte-identical
/// in shape to a legitimate stationary L1 `deposit` (material laid in place — see
/// `verify_contracts::stationary_deposit_is_not_a_retraction_prime`). `retraction-speed` is an *error*,
/// so the ambiguity is resolved toward not gating a legal program; closing it needs the importer to
/// record which act the `E` word performed, not a reinterpretation here. Recorded in `docs/14`.
#[test]
fn retraction_speed_cannot_see_an_imported_unretract() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 E1 F6000
G1 X10 Y0 E0.42 F2400
";
    let c = Contracts {
        max_retraction_speed: Some(2400.0),
        ..Contracts::default()
    };
    let r = review(gcode, &c);
    assert!(r.evaluated(RuleId::RetractionSpeed));
    assert_eq!(
        count(&r, RuleId::RetractionSpeed),
        0,
        "an imported de-retraction is indistinguishable from a stationary deposit: {:?}",
        messages(&r, RuleId::RetractionSpeed)
    );
}

fn scv(limit: f64) -> Contracts {
    Contracts {
        kinematics: Some(KinematicContracts {
            max_acceleration_mm_s2: None,
            max_junction_velocity_mm_s: Some(limit),
        }),
        ..Contracts::default()
    }
}

/// The case the rule is *named* for and the case H1.3 recorded it as missing: a 90° corner taken at a
/// constant feedrate. There is no speed *difference* here at all — 2400 mm/min in, 2400 out — so any
/// measure of Δ|v| sees nothing. What the machine must do is reverse one axis instantaneously.
///
/// The limit is calibrated on exactly this corner: a 90° junction is allowed the square-corner velocity
/// itself, so 40 mm/s against `scv = 8` fires, and the message must say so in the units of the rule.
#[test]
fn junction_velocity_catches_a_constant_speed_right_angle() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F2400
G1 X10 Y10 E0.42 F2400
";
    let r = review(gcode, &scv(8.0));
    let found = messages(&r, RuleId::JunctionVelocity);
    assert_eq!(
        found.len(),
        1,
        "expected the 90° corner to be flagged once, got {found:?}"
    );
    assert!(
        found[0].contains("turns 90.0°") && found[0].contains("8.0 mm/s it allows"),
        "the message must name the direction change and the allowance: {}",
        found[0]
    );
}

/// The same corner at the speed it is allowed passes. This is what pins the calibration rather than
/// merely the sign of the comparison: 8 mm/s is 480 mm/min, and a 90° junction entered at exactly the
/// square-corner velocity is not a violation.
#[test]
fn junction_velocity_allows_a_right_angle_at_the_square_corner_velocity() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F480
G1 X10 Y10 E0.42 F480
";
    let r = review(gcode, &scv(8.0));
    assert_eq!(
        count(&r, RuleId::JunctionVelocity),
        0,
        "a 90° corner at exactly scv must pass: {:?}",
        messages(&r, RuleId::JunctionVelocity)
    );
}

/// A collinear 10 → 100 mm/s step: a large velocity *change* and no direction change whatsoever. The
/// scalar measure this rule used to carry fired here (Δv = 90 mm/s), under the name of a cornering
/// limit. A straight line has no corner, so nothing about junction velocity is violated; whether the
/// machine can make that step in the distance available is an *acceleration* question, and `docs/14`
/// records that no rule currently asks it.
#[test]
fn junction_velocity_ignores_a_collinear_speed_change() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F600
G1 X20 Y0 E0.42 F6000
";
    let r = review(gcode, &scv(8.0));
    assert_eq!(
        count(&r, RuleId::JunctionVelocity),
        0,
        "a straight line has no corner: {:?}",
        messages(&r, RuleId::JunctionVelocity)
    );
}

/// A shallow corner is allowed *more* than the square-corner velocity, which is the whole content of
/// the junction-deviation relation and the property the flat Δv form could not express. Two 10 mm legs
/// meeting at ~5.7° (`atan2(1, 10)`) at 40 mm/s: allowed ≈ 8 · sqrt(0.4142 · 0.9988/0.0012) ≈ 152 mm/s.
#[test]
fn junction_velocity_allows_a_shallow_corner_at_print_speed() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F2400
G1 X20 Y1 E0.42 F2400
";
    let r = review(gcode, &scv(8.0));
    assert_eq!(
        count(&r, RuleId::JunctionVelocity),
        0,
        "a 5.7° deflection at 40 mm/s is within reach: {:?}",
        messages(&r, RuleId::JunctionVelocity)
    );
}

/// A full reversal is allowed nothing, so it fires at any print speed. The other calibration anchor.
#[test]
fn junction_velocity_catches_a_reversal() {
    let gcode = "\
M83
G0 X0 Y0 Z0.2 F9000
G1 X10 Y0 E0.42 F600
G1 X0 Y0 E0.42 F600
";
    let r = review(gcode, &scv(8.0));
    let found = messages(&r, RuleId::JunctionVelocity);
    assert_eq!(found.len(), 1, "expected the reversal to fire: {found:?}");
    assert!(
        found[0].contains("turns 180.0°") && found[0].contains("0.0 mm/s it allows"),
        "a reversal allows nothing: {}",
        found[0]
    );
}

/// `balanced` shaping and this rule must not disagree about one machine limit: the optimizer's absolute
/// per-junction cap is `scv·cos(φ/2)`, the rule's allowance is
/// `scv·sqrt((√2−1)·cos(φ/2)/(1−cos(φ/2)))`, and the first is ≤ the second for every junction because
/// `f(1−f) ≤ 1/4 < √2−1`. So a toolpath `adaptive_speed_with_kinematics` produced always verifies clean
/// under the same `scv` — an optimizer may be conservative, a verifier may not.
#[test]
fn optimize_junction_cap_never_exceeds_verify_limit() {
    // 4001 samples of cos(φ/2) across the open interval; f = 1 is the straight junction, where the
    // allowance is infinite and the inequality is trivial.
    for i in 1..4001 {
        let f = i as f64 / 4001.0;
        let scv = 8.0;
        let cap = scv * f;
        let allowed = scv * ((std::f64::consts::SQRT_2 - 1.0) * f / (1.0 - f)).sqrt();
        assert!(
            cap <= allowed,
            "optimizer cap {cap} exceeds verify allowance {allowed} at cos(phi/2) = {f}"
        );
    }
}
