//! `drymachina-contracts` is the vocabulary shared by the kernel and the verifier. It must compile with no
//! dependency on either — that is the whole reason it exists (plan Task 3, spec §5.7).

use drymachina_contracts::{
    parse_bounds_csv, parse_speed_range_csv, Contracts, Kinematics, RotaryContracts,
    RotaryTravelRanges, RuleId, Severity, ARC_RADIUS_TOLERANCE_MM,
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

// --- moved with the vocabulary itself (plan Tasks 3 and 5) --------------------------------------
//
// Everything below exercises `drymachina-contracts` and nothing else, but was written inside
// `verify.rs`'s in-module tests, back when the vocabulary lived there. Task 3 moved the code out
// and left these behind; Task 5 would have carried them on to `drymachina-verify` with the rest of
// `verify.rs`, leaving the crate that owns the code with six tests covering it.

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

#[test]
fn contracts_default_has_no_kinematics() {
    assert!(Contracts::default().kinematics.is_none());
}

/// The rule vocabulary's own consistency: every id round-trips through its wire form, carries a
/// summary, and lands on the right side of the error/warning split. One layer up, `catalog()`
/// projects these same facts into `Rule` entries, and `drymachina-verify` tests that projection.
#[test]
fn rule_vocabulary_is_consistent() {
    for id in RuleId::ALL {
        // wire id round-trips and is unique-mapping
        assert_eq!(RuleId::from_wire(id.as_str()), Some(id));
        assert!(!id.summary().is_empty());
    }
    // process/quality advisories are warnings; everything else is an error.
    let warnings: Vec<&str> = RuleId::ALL
        .into_iter()
        .filter(|id| id.default_severity() == Severity::Warning)
        .map(|id| id.as_str())
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
    // count is the same decision as one joining it.
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
    assert_eq!(RuleId::ALL.len(), 27);
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

/// `Kinematics` reaches the wire through hand-written `Serialize`/`Deserialize` impls rather than a
/// derive, because a profile may name a model as a bare string *or* as a struct carrying offsets.
/// Two hand-written halves can drift apart with nothing to hold them together, which is what this
/// covers: the string form and the struct form must land on the same value, and what `serialize`
/// writes must be what `deserialize` reads.
#[test]
fn kinematics_round_trips_through_its_hand_written_serde() {
    let model = Kinematics::Bc {
        pivot_offset: [1.5, -2.0, 0.25],
        rotary_offset: [3.0, -4.0],
    };
    let json = serde_json::to_string(&model).unwrap();
    assert_eq!(
        json,
        r#"{"type":"bc","pivot_offset":[1.5,-2.0,0.25],"rotary_offset":[3.0,-4.0]}"#
    );
    assert_eq!(serde_json::from_str::<Kinematics>(&json).unwrap(), model);

    // The bare-string form is the zero-offset model of the same name.
    for name in ["ab", "ac", "bc"] {
        assert_eq!(
            serde_json::from_str::<Kinematics>(&format!("\"{name}\"")).unwrap(),
            Kinematics::named(name).unwrap()
        );
    }
}

/// Both rejection paths, because they are worded separately: an unknown *name* and an unknown
/// struct `type` are different messages, and a non-finite offset is refused by `validate` rather
/// than by serde.
#[test]
fn kinematics_refuses_unknown_models_and_non_finite_offsets() {
    assert_eq!(
        Kinematics::named("xy").unwrap_err(),
        "unknown kinematics: xy"
    );
    assert!(serde_json::from_str::<Kinematics>(r#""xy""#)
        .unwrap_err()
        .to_string()
        .contains("unknown kinematics: xy"));
    assert!(serde_json::from_str::<Kinematics>(r#"{"type":"xy"}"#)
        .unwrap_err()
        .to_string()
        .contains("unknown kinematics type: xy"));

    assert!(Kinematics::named("bc").unwrap().validate().is_ok());
    assert_eq!(
        Kinematics::Bc {
            pivot_offset: [0.0, f64::NAN, 0.0],
            rotary_offset: [0.0, 0.0],
        }
        .validate()
        .unwrap_err(),
        "pivot_offset[y] must be finite"
    );
    assert_eq!(
        Kinematics::Ac {
            pivot_offset: [0.0, 0.0, 0.0],
            rotary_offset: [0.0, f64::INFINITY],
        }
        .validate()
        .unwrap_err(),
        "rotary_offset[1] must be finite"
    );
}
