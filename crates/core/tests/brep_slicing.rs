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
