use dry_core::{emit_gcode_chunks, resolve, Design, EmitParams, Op, ResolveParams};

#[test]
fn test_chunked_stream_gcode_emission() {
    let mut design = Design::default();
    design.ops.push(Op::Move {
        x: Some(0.0),
        y: Some(0.0),
        z: Some(0.2),
    });
    for i in 1..=20 {
        design.ops.push(Op::Extruder { on: true });
        design.ops.push(Op::Move {
            x: Some((i * 5) as f64),
            y: Some((i * 5) as f64),
            z: Some(0.2),
        });
    }

    let toolpath = resolve(&design, &ResolveParams::default());
    let params = EmitParams::default();

    // Chunk size of 5 lines per block
    let chunks = emit_gcode_chunks(&toolpath, &params, 5).expect("must emit chunks");
    assert!(!chunks.is_empty());

    // Reconstruct full gcode
    let joined = chunks.join("\n");
    assert!(joined.contains("G1 X100"));
}
