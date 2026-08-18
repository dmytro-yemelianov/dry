use dry_core::{
    optimize_corner_feedrate, resolve, Design, Op, ResolveParams,
};

#[test]
fn test_engagement_corner_deceleration() {
    let mut design = Design::default();
    // Move to start point
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.0),
    });
    // Cut straight line along X at 2000 mm/min
    design.ops.push(Op::Speed { print: 2000.0 });
    design.ops.push(Op::Extruder { on: true });
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(0.0),
        z: Some(0.0),
    });
    // Sharp 90-degree turn along Y
    design.ops.push(Op::Move {
        x: Some(50.0),
        y: Some(50.0),
        z: Some(0.0),
    });

    let mut toolpath = resolve(&design, &ResolveParams::default());

    // Segments: 0 is rapid to (0,0,0), 1 is cut to (50,0,0), 2 is cut to (50,50,0)
    let cut_seg_index = 1;
    let initial_speed = toolpath.segments[cut_seg_index].speed.value();
    assert_eq!(initial_speed, 2000.0);

    // Run engagement corner feedrate optimization
    optimize_corner_feedrate(&mut toolpath, 0.4);

    let optimized_corner_speed = toolpath.segments[cut_seg_index].speed.value();
    // Entering corner should be decelerated (cos(90°/2) = cos(45°) ~ 0.707 * 2000 ~ 1414 mm/min)
    assert!(optimized_corner_speed < 2000.0);
    assert!(optimized_corner_speed >= 800.0); // Bounded by min_feed_ratio 0.4
}
