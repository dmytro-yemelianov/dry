//! `laser-power-during-travel` — the process-gated beam-off-during-travel rule.
//!
//! The rule is gated on `Contracts::travel_must_be_dark` rather than always-on because `Op::Power`
//! is one channel with two meanings (spindle RPM and laser PWM) that disagree about travel. These
//! tests pin both halves: a laser profile that opts in catches the lit travel, and a milling program
//! that does not opt in is left alone.

use dry_core::{verify, Contracts, DesignBuilder, RuleId, Severity};

fn dark_travel_contract() -> Contracts {
    Contracts {
        travel_must_be_dark: Some(true),
        ..Contracts::default()
    }
}

fn finding(rep: &dry_core::verify::Report) -> Option<&dry_core::verify::Finding> {
    rep.findings
        .iter()
        .find(|f| f.rule == RuleId::LaserPowerDuringTravel.as_str())
}

#[test]
fn test_laser_power_during_travel_flagged_under_a_dark_travel_contract() {
    let d = DesignBuilder::new()
        .geometry(0.5, 0.2)
        .point(0.0, 0.0, 0.0)
        .extruder(true) // Start cutting
        .power(1000.0) // Laser ON
        .point(10.0, 0.0, 0.0)
        .extruder(false) // Traversal move with power still active (forgot to power off)
        .point(50.0, 0.0, 0.0)
        .build();

    let tp = d.ir().unwrap();
    let rep = verify(&tp, &dark_travel_contract());

    let f =
        finding(&rep).expect("Expected finding for LaserPowerDuringTravel on rapid travel move");
    assert_eq!(f.severity, Severity::Error);
}

#[test]
fn test_clean_laser_program_with_power_shutoff_passes_verify() {
    let d = DesignBuilder::new()
        .geometry(0.5, 0.2)
        .point(0.0, 0.0, 0.0)
        .extruder(true) // Start cutting
        .power(1000.0)
        .point(10.0, 0.0, 0.0)
        .power(0.0) // Safely command laser OFF before travel
        .extruder(false)
        .point(50.0, 0.0, 0.0)
        .build();

    let tp = d.ir().unwrap();
    let rep = verify(&tp, &dark_travel_contract());

    assert!(finding(&rep).is_none());
}

/// The reason the rule is not always-on. A spindle turning through a rapid is mandatory practice,
/// not a hazard, and it reaches `Segment.power` by the same channel a laser does. Without the
/// contract this program must verify clean; with an always-on rule it did not.
#[test]
fn test_milling_rapid_with_the_spindle_running_is_not_flagged_by_default() {
    let d = DesignBuilder::new()
        .geometry(6.0, 1.0)
        .point(0.0, 0.0, -1.0)
        .power(8000.0) // S8000 — spindle RPM, not a beam
        .extruder(true)
        .point(40.0, 0.0, -1.0) // cut
        .extruder(false)
        .point(40.0, 6.0, -1.0) // rapid across to the next pass, spindle still turning
        .extruder(true)
        .point(0.0, 6.0, -1.0) // cut
        .build();

    let tp = d.ir().unwrap();
    let rep = verify(&tp, &Contracts::default());

    assert!(
        finding(&rep).is_none(),
        "a milling rapid with the spindle running must not be a finding without an opt-in contract"
    );
    assert!(
        !rep.rules_evaluated
            .iter()
            .any(|r| r == RuleId::LaserPowerDuringTravel.as_str()),
        "an un-contracted rule must not be reported as evaluated (a vacuous pass)"
    );
}

/// The same milling program *does* trip the rule once a profile declares the process is beam-based —
/// which is the point of gating it on the profile rather than on the IR.
#[test]
fn test_the_same_program_is_flagged_once_a_profile_opts_in() {
    let d = DesignBuilder::new()
        .geometry(6.0, 1.0)
        .point(0.0, 0.0, -1.0)
        .power(8000.0)
        .extruder(true)
        .point(40.0, 0.0, -1.0)
        .extruder(false)
        .point(40.0, 6.0, -1.0)
        .extruder(true)
        .point(0.0, 6.0, -1.0)
        .build();

    let tp = d.ir().unwrap();
    let rep = verify(&tp, &dark_travel_contract());

    assert!(finding(&rep).is_some());
    assert!(rep
        .rules_evaluated
        .iter()
        .any(|r| r == RuleId::LaserPowerDuringTravel.as_str()));
}
