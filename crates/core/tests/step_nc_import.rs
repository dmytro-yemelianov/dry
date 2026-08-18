use dry_core::{lower_workingstep_to_ops, parse_step_nc, resolve, Design, Op, ResolveParams, StepNcFeature};

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
        StepNcFeature::RoundHole { x, y, diameter, depth } => {
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
