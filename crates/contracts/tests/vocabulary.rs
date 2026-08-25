//! `kmet-contracts` is the vocabulary shared by the kernel and the verifier. It must compile with no
//! dependency on either — that is the whole reason it exists (plan Task 3, spec §5.7).

use kmet_contracts::{
    parse_bounds_csv, parse_speed_range_csv, Contracts, Kinematics, RotaryContracts, RuleId,
    Severity, ARC_RADIUS_TOLERANCE_MM,
};

#[test]
fn contracts_default_is_permissive() {
    let c = Contracts::default();
    assert!(c.bounds.is_none());
    assert!(c.max_flow.is_none());
    assert!(!c.monotonic_z);
}

#[test]
fn bounds_csv_round_trips() {
    let b = parse_bounds_csv("0,200,0,200,0,250").unwrap();
    assert_eq!(b[0], [0.0, 200.0]);
    assert_eq!(b[2], [0.0, 250.0]);
}

#[test]
fn speed_range_csv_round_trips() {
    let s = parse_speed_range_csv("300,9000").unwrap();
    assert_eq!(s, [300.0, 9000.0]);
}

#[test]
fn severity_and_rule_id_are_serialisable() {
    assert_eq!(
        serde_json::to_string(&Severity::Error).unwrap(),
        "\"error\""
    );
    // `RuleId` reaches the wire through its kebab-case id rather than a serde derive, so this is
    // what a report actually carries.
    assert_eq!(RuleId::Bounds.as_str(), "bounds");
    assert_eq!(RuleId::from_wire("bounds"), Some(RuleId::Bounds));
}

#[test]
fn rotary_contracts_carry_a_kinematic_model() {
    let rc = RotaryContracts {
        model: Kinematics::Ac {
            pivot_offset: [0.0; 3],
            rotary_offset: [0.0; 2],
        },
        travel_deg: None,
        max_rotary_feed_deg_min: None,
        envelope_mm: None,
    };
    assert!(rc.travel_deg.is_none());
}

#[test]
fn arc_tolerance_is_exposed() {
    assert_eq!(ARC_RADIUS_TOLERANCE_MM, 1e-6);
}
