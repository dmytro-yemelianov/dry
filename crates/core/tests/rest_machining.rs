//! CNC Multi-Pass Rest Machining Integration Tests.

use dry_core::{
    emit_stream, generate_corner_rest_machining_ops, resolve, CncFrame, Design, EmitParams,
    FirmwareFlavor, ResolveParams, RestMachiningParams,
};

#[test]
fn test_rest_machining_90deg_corner_e2e() {
    let params = RestMachiningParams {
        rough_tool_diameter: 12.0,
        rest_tool_diameter: 3.0,
        corner_vertex: [100.0, 100.0],
        corner_angle_deg: 90.0,
        z_cut: -4.0,
        z_clearance: 5.0,
        feedrate: 900.0,
        radial_passes: 3,
    };

    let ops = generate_corner_rest_machining_ops(&params).expect("Failed to generate rest ops");
    let design = Design { ops };
    let toolpath = resolve(&design, &ResolveParams::default());

    assert!(toolpath.segments.len() >= 6);

    let emit_params = EmitParams {
        flavor: FirmwareFlavor::Rs274,
        cnc_frame: Some(CncFrame {
            wcs: Some(54),
            spindle_rpm: Some(12000.0),
            coolant: Some(true),
            tool: None,
        }),
        ..EmitParams::default()
    };

    let gcode_lines = emit_stream(toolpath.segments.iter().cloned().map(Ok), &emit_params)
        .expect("Failed to emit G-code");
    assert!(gcode_lines.iter().any(|l| l.contains("G54")));
    assert!(gcode_lines.iter().any(|l| l.contains("S12000")));
    assert!(gcode_lines.iter().any(|l| l.contains("M8")));
}

#[test]
fn test_rest_machining_rejects_invalid_tool_sizes() {
    let params = RestMachiningParams {
        rough_tool_diameter: 4.0,
        rest_tool_diameter: 6.0, // Invalid: rest tool is bigger than rough tool
        ..RestMachiningParams::default()
    };

    assert!(generate_corner_rest_machining_ops(&params).is_err());
}

#[test]
fn test_rest_machining_non_orthogonal_corners() {
    for &angle in &[60.0, 120.0, 135.0] {
        let params = RestMachiningParams {
            rough_tool_diameter: 10.0,
            rest_tool_diameter: 4.0,
            corner_vertex: [50.0, 50.0],
            corner_angle_deg: angle,
            z_cut: -2.0,
            z_clearance: 5.0,
            feedrate: 800.0,
            radial_passes: 2,
        };

        let ops = generate_corner_rest_machining_ops(&params).expect("Failed non-orthogonal rest ops");
        assert!(!ops.is_empty());
        let design = Design { ops };
        let toolpath = resolve(&design, &ResolveParams::default());
        assert!(!toolpath.segments.is_empty());
    }
}
