use dry_core::{
    lower_workingstep_to_ops, parse_step_nc, resolve, Design, ResolveParams, StepNcFeature,
};

#[test]
fn test_parse_and_lower_step_nc_workingsteps() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<stepnc xmlns="urn:iso:std:iso-10303-14649">
  <workingsteps>
    <workingstep id="ws-1" type="hole" x="50" y="25" diameter="6.0" depth="15.0" feed="800" rpm="3000"/>
    <workingstep id="ws-2" type="pocket" x="10" y="10" length="40" width="30" depth="5.0" feed="1500"/>
  </workingsteps>
</stepnc>"#;

    let steps = parse_step_nc(xml).expect("parse valid STEP-NC XML");
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].id, "ws-1");

    match &steps[0].feature {
        StepNcFeature::RoundHole {
            x,
            y,
            diameter,
            depth,
        } => {
            assert_eq!(*x, 50.0);
            assert_eq!(*y, 25.0);
            assert_eq!(*diameter, 6.0);
            assert_eq!(*depth, 15.0);
        }
        _ => panic!("expected RoundHole feature"),
    }

    let mut design = Design::default();
    for step in &steps {
        let ops = lower_workingstep_to_ops(step);
        assert!(!ops.is_empty());
        design.ops.extend(ops);
    }

    let toolpath = resolve(&design, &ResolveParams::default());
    assert!(!toolpath.segments.is_empty());
}

/// A malformed geometric attribute must be refused, not replaced with a default.
///
/// Every geometric attribute used to be read as `parse().ok()` and then `unwrap_or`'d to a
/// hard-coded value — `depth` to 5mm, `x`/`y` to 0, `diameter` to 6mm. A European decimal comma, a
/// unit suffix or a typo was therefore indistinguishable from an absent attribute and from one
/// actually written, so the importer produced a valid-looking program machined to the wrong depth at
/// the wrong coordinates with nothing reported. `.parse::<f64>()` also accepts `NaN` and `inf`.
#[test]
fn malformed_geometry_is_refused_rather_than_defaulted() {
    let hole = |attrs: &str| {
        format!("<stepnc>\n<workingstep id=\"w1\" type=\"hole\" {attrs}/>\n</stepnc>")
    };

    // The well-formed control still parses, with the values as written.
    let ok = dry_core::parse_step_nc(&hole(r#"x="40.0" y="25.0" diameter="8.0" depth="12.5""#))
        .expect("a well-formed hole must parse");
    assert_eq!(ok.len(), 1);

    for (label, attrs) in [
        (
            "European decimal comma",
            r#"x="40.0" y="25.0" diameter="8.0" depth="12,5""#,
        ),
        (
            "unit suffix",
            r#"x="40.0" y="25.0" diameter="8.0" depth="12.5mm""#,
        ),
        (
            "unparseable position",
            r#"x="forty" y="25.0" diameter="8.0" depth="12.5""#,
        ),
        (
            "NaN depth",
            r#"x="40.0" y="25.0" diameter="8.0" depth="NaN""#,
        ),
        (
            "infinite depth",
            r#"x="40.0" y="25.0" diameter="8.0" depth="inf""#,
        ),
    ] {
        let err =
            dry_core::parse_step_nc(&hole(attrs)).expect_err(&format!("{label} must be refused"));
        assert!(
            err.contains("not a number") || err.contains("not finite"),
            "{label}: expected a parse refusal, got {err}"
        );
    }
}

/// A geometric attribute the feature cannot be placed without must be required, not defaulted.
#[test]
fn absent_required_geometry_is_refused() {
    // A hole with no depth: previously drilled 5mm, a number nobody wrote.
    let err = dry_core::parse_step_nc(
        "<stepnc>\n<workingstep id=\"w1\" type=\"hole\" x=\"40\" y=\"25\" diameter=\"8\"/>\n</stepnc>",
    )
    .expect_err("a hole with no depth must be refused");
    assert!(
        err.contains("depth"),
        "the message must name the attribute: {err}"
    );

    // A pocket with no width: previously 20mm.
    let err = dry_core::parse_step_nc(
        "<stepnc>\n<workingstep id=\"w1\" type=\"pocket\" x=\"10\" y=\"10\" length=\"40\" depth=\"5\"/>\n</stepnc>",
    )
    .expect_err("a pocket with no width must be refused");
    assert!(
        err.contains("width"),
        "the message must name the attribute: {err}"
    );

    // Optional descriptive attributes stay optional.
    dry_core::parse_step_nc(
        "<stepnc>\n<workingstep id=\"w1\" type=\"hole\" x=\"40\" y=\"25\" diameter=\"8\" depth=\"12\"/>\n</stepnc>",
    )
    .expect("feed and rpm are genuinely optional");
}

#[test]
fn test_parse_and_lower_step_nc_slot_and_peck_drilling() {
    let xml = r#"<stepnc>
  <workingsteps>
    <workingstep id="ws-peck" type="peck_drilling" x="30" y="30" diameter="6.0" depth="10.0" peck="2.5" feed="600"/>
    <workingstep id="ws-slot" type="slot" x1="10" y1="20" x2="50" y2="20" width="8.0" depth="3.0" feed="1200"/>
  </workingsteps>
</stepnc>"#;

    let steps = dry_core::parse_step_nc(xml).expect("parse slot and peck drilling");
    assert_eq!(steps.len(), 2);

    let mut design = dry_core::Design::default();
    for step in &steps {
        let ops = dry_core::lower_workingstep_to_ops(step);
        assert!(!ops.is_empty());
        design.ops.extend(ops);
    }

    let tp = dry_core::resolve(&design, &dry_core::ResolveParams::default());
    assert!(!tp.segments.is_empty());
}
