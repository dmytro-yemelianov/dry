use dry_core::{GcodeTemplate, TemplateContext};

#[test]
fn test_macro_templating_variable_interpolation() {
    let mut template = GcodeTemplate::new();
    template.start_macro = Some(
        "G28 ; Home\nT{{ tool_number }} M06 ; Tool\nS{{ spindle_rpm }} M03 ; Spindle\nF{{ feedrate }} ; Feed".into(),
    );
    template.end_macro = Some("M05 ; Spindle Stop\nG00 Z{{ max_z }} ; Park Z\nM30 ; End".into());

    let ctx = TemplateContext {
        tool_number: Some(4),
        spindle_rpm: Some(12000.0),
        feedrate: Some(1800.0),
        max_x: Some(300.0),
        max_y: Some(300.0),
        max_z: Some(150.0),
    };

    let start = template.render_start(&ctx).expect("must render start");
    assert!(start.contains("T4 M06"));
    assert!(start.contains("S12000 M03"));
    assert!(start.contains("F1800.0"));

    let end = template.render_end(&ctx).expect("must render end");
    assert!(end.contains("G00 Z150.000 ; Park Z"));
    assert!(end.contains("M30 ; End"));
}
