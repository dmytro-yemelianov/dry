use dry_core::{verify, Contracts, DesignBuilder, RuleId, Severity};

#[test]
fn test_laser_power_during_travel_flagged_by_verifier() {
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
    let rep = verify(&tp, &Contracts::default());

    // Should have finding for laser power during travel
    let laser_finding = rep
        .findings
        .iter()
        .find(|f| f.rule == RuleId::LaserPowerDuringTravel.as_str());

    assert!(
        laser_finding.is_some(),
        "Expected finding for LaserPowerDuringTravel on rapid travel move"
    );
    assert_eq!(laser_finding.unwrap().severity, Severity::Error);
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
    let rep = verify(&tp, &Contracts::default());

    let laser_finding = rep
        .findings
        .iter()
        .find(|f| f.rule == RuleId::LaserPowerDuringTravel.as_str());

    assert!(laser_finding.is_none());
}
