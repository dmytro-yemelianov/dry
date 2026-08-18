use dry_core::{emit_idex_mode, emit_select_head, HeadConfig, HeadMode};

#[test]
fn test_idex_modes_and_head_selection() {
    let head0 = HeadConfig::new(0, "Left Extruder", -35.0);
    let head1 = HeadConfig::new(1, "Right Extruder", 235.0);

    assert_eq!(head0.head_index, 0);
    assert_eq!(head1.park_x, 235.0);

    assert_eq!(emit_select_head(0), "T0 ; Select Toolhead 0");
    assert_eq!(emit_select_head(1), "T1 ; Select Toolhead 1");

    let auto_park = emit_idex_mode(HeadMode::Independent, None);
    assert_eq!(auto_park[0], "M605 S1 ; Set IDEX Mode: Auto-Park (Independent)");

    let dup = emit_idex_mode(HeadMode::Duplication, Some(110.0));
    assert_eq!(dup[0], "M605 S2 X110.000 ; Set IDEX Mode: Duplication");
    assert_eq!(dup[1], "M605 W ; Activate Duplication");

    let mirror = emit_idex_mode(HeadMode::Mirrored, None);
    assert_eq!(mirror[0], "M605 S3 ; Set IDEX Mode: Mirrored");
}
