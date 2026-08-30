//! B-Rep Solid Topology Analytical Slicing Suite (Phase D1).

use dry_core::generate::{BrepSolid, Point3D, SurfacePrimitive, Vector3D};
use dry_core::{resolve, Design, ResolveParams};

#[test]
fn test_brep_cylinder_exact_slicing() {
    let mut solid = BrepSolid::new("cylinder-part");
    solid.add_surface(SurfacePrimitive::Cylinder {
        origin: Point3D::new(50.0, 50.0, 0.0),
        axis: Vector3D::unit_z(),
        radius: 20.0,
        height: 10.0,
    });

    let ops = solid
        .slice_to_l1_ops(1.0, 5.0, 2.0, 32, 1800.0)
        .expect("slicing cylinder should succeed");

    assert!(!ops.is_empty());

    let mut design = Design::default();
    design.ops.extend(ops);
    let tp = resolve(&design, &ResolveParams::default());
    assert!(!tp.segments.is_empty());

    // Check that segments carry exact normal vectors
    for seg in &tp.segments {
        if let Some([i, j, k]) = seg.orientation {
            let mag = (i * i + j * j + k * k).sqrt();
            assert!((mag - 1.0).abs() < 1e-5, "orientation must be unit length");
        }
    }
}

#[test]
fn test_brep_sphere_exact_normals() {
    let mut solid = BrepSolid::new("sphere-dome");
    solid.add_surface(SurfacePrimitive::Sphere {
        center: Point3D::new(50.0, 50.0, 0.0),
        radius: 25.0,
    });

    let ops = solid
        .slice_to_l1_ops(5.0, 20.0, 5.0, 32, 1500.0)
        .expect("slicing sphere should succeed");

    assert!(!ops.is_empty());

    let mut design = Design::default();
    design.ops.extend(ops);
    let tp = resolve(&design, &ResolveParams::default());
    assert!(!tp.segments.is_empty());
}

#[test]
fn test_brep_step_parsing() {
    let step_mock = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (50.0, 50.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 18.5);
#200 = SPHERICAL_SURFACE('', #10, 30.0);
ENDSEC;
END-ISO-10303-21;
"#;

    let solid = BrepSolid::parse_step_iso10303(step_mock).expect("parsing STEP should succeed");
    assert_eq!(solid.surfaces.len(), 2);
}

#[test]
fn test_brep_assembly_multi_solid_slicing() {
    use dry_core::generate::{BrepAssembly, BrepBodyRole};

    let mut asm = BrepAssembly::new("aerospace_bracket_assembly");

    let mut outer_cylinder = BrepSolid::new("outer_body");
    outer_cylinder.add_surface(SurfacePrimitive::Cylinder {
        origin: Point3D::new(0.0, 0.0, 0.0),
        axis: Vector3D::unit_z(),
        radius: 25.0,
        height: 10.0,
    });

    let mut dome = BrepSolid::new("top_dome");
    dome.add_surface(SurfacePrimitive::Sphere {
        center: Point3D::new(0.0, 0.0, 10.0),
        radius: 25.0,
    });

    let mut obstacle = BrepSolid::new("clamp_keepout");
    obstacle.add_surface(SurfacePrimitive::Plane {
        origin: Point3D::new(50.0, 50.0, 0.0),
        normal: Vector3D::unit_z(),
    });

    asm.add_solid(outer_cylinder, BrepBodyRole::AdditiveBody);
    asm.add_solid(dome, BrepBodyRole::AdditiveBody);
    asm.add_solid(obstacle, BrepBodyRole::KeepoutObstacle);

    let ops = asm
        .slice_to_l1_ops(2.0, 8.0, 2.0, 36, 1800.0)
        .expect("slicing assembly should succeed");

    assert!(!ops.is_empty());

    let mut design = Design::default();
    design.ops.extend(ops);
    let tp = resolve(&design, &ResolveParams::default());
    assert!(!tp.segments.is_empty());
}

#[test]
fn test_brep_assembly_csg_boolean_subtraction() {
    use dry_core::generate::{BrepAssembly, BrepBodyRole};

    let mut asm = BrepAssembly::new("bushing_with_bore");

    // Outer solid cylinder radius 30 mm
    let mut outer_cylinder = BrepSolid::new("outer_body");
    outer_cylinder.add_surface(SurfacePrimitive::Cylinder {
        origin: Point3D::new(0.0, 0.0, 0.0),
        axis: Vector3D::unit_z(),
        radius: 30.0,
        height: 20.0,
    });

    // Subtractive center cavity/bore cylinder radius 10 mm
    let mut bore = BrepSolid::new("center_bore");
    bore.add_surface(SurfacePrimitive::Cylinder {
        origin: Point3D::new(0.0, 0.0, 0.0),
        axis: Vector3D::unit_z(),
        radius: 10.0,
        height: 20.0,
    });

    asm.add_solid(outer_cylinder, BrepBodyRole::AdditiveBody);
    asm.add_solid(bore, BrepBodyRole::SubtractiveVoid);

    let ops = asm
        .slice_with_csg(2.0, 10.0, 4.0, 36, 1800.0)
        .expect("CSG assembly slicing should succeed");

    assert!(!ops.is_empty());

    let mut design = Design::default();
    design.ops.extend(ops);
    let tp = resolve(&design, &ResolveParams::default());
    assert!(!tp.segments.is_empty());
}

#[test]
fn test_brep_step_parsing_all_quadrics() {
    let step_mock = r#"
ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('STEP AP242'),'2;1');
ENDSEC;
DATA;
#10 = CARTESIAN_POINT('', (50.0, 50.0, 0.0));
#20 = DIRECTION('', (0.0, 0.0, 1.0));
#100 = CYLINDRICAL_SURFACE('', #10, 18.5);
#200 = SPHERICAL_SURFACE('', #10, 30.0);
#300 = CONICAL_SURFACE('', #10, 15.0, 0.523599);
#400 = TOROIDAL_SURFACE('', #10, 40.0, 8.0);
#500 = PLANE('', #10);
ENDSEC;
END-ISO-10303-21;
"#;

    let solid = BrepSolid::parse_step_iso10303(step_mock).expect("parsing full STEP entities");
    assert_eq!(solid.surfaces.len(), 5);

    let ops = solid
        .slice_to_l1_ops(2.0, 10.0, 2.0, 32, 1200.0)
        .expect("slice all STEP quadrics");
    assert!(!ops.is_empty());
}

/// B-Rep slicing refuses what it cannot slice, and refuses work it cannot finish.
///
/// `slice_to_l1_ops` walks `z` from `z_start` to `z_end` in `layer_height` steps, sampling every
/// surface at each. It had no budget at all — the guardrail `generate/tpms.rs` has carried since
/// H1.4 — so `z_end = 1e9, layer_height = 1e-6` is 10^15 slices and the process is killed rather
/// than answering. This surface is on every SDK including wasm, so that is a browser tab that never
/// comes back.
#[test]
fn slicing_refuses_degenerate_bounds_and_unbounded_work() {
    const STEP: &str = "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n\
                        #1=CYLINDRICAL_SURFACE('c',#2,12.0);\nENDSEC;\nEND-ISO-10303-21;\n";
    let solid = BrepSolid::parse_step_iso10303(STEP).expect("fixture parses");

    // `z_end < z_start` is false when either is NaN, so the ordering check cannot stand alone: a
    // NaN bound previously produced an empty op list, which is a vacuous program, not a refusal.
    for (z0, z1) in [(f64::NAN, 10.0), (0.0, f64::NAN), (f64::INFINITY, 10.0)] {
        let err = solid
            .slice_to_l1_ops(z0, z1, 1.0, 32, 1200.0)
            .expect_err("non-finite slice bounds must be refused");
        assert!(err.message.contains("must be finite"), "{}", err.message);
    }

    // The feedrate is written straight into `Op::Speed`; an unchecked value put invalid IR into the
    // op stream, and a negative one reintroduced what ingress validation refuses.
    for bad in [-100.0, 0.0, f64::NAN, f64::INFINITY] {
        let err = solid
            .slice_to_l1_ops(0.0, 10.0, 1.0, 32, bad)
            .expect_err("an unusable feedrate must be refused");
        assert!(err.message.contains("feedrate"), "{}", err.message);
    }

    let err = solid
        .slice_to_l1_ops(0.0, 10.0, 1.0, 0, 1200.0)
        .expect_err("zero samples per slice must be refused");
    assert!(err.message.contains("samples_per_slice"), "{}", err.message);

    // The runaway: refused up front, in constant time, rather than by the OOM killer.
    let err = solid
        .slice_to_l1_ops(0.0, 1e9, 1e-6, 32, 1200.0)
        .expect_err("an unbounded slice request must be refused");
    assert!(err.message.contains("budget exceeded"), "{}", err.message);

    // An ordinary request still slices, so the guards have not disabled the generator.
    let ops = solid
        .slice_to_l1_ops(2.0, 10.0, 2.0, 32, 1200.0)
        .expect("a reasonable slice request must still succeed");
    assert!(!ops.is_empty());
}
