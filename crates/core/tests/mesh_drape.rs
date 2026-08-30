//! Conformance and integration tests for Mesh Heightfield 5-Axis Drape Generator (E1.3).

use dry_core::generate::drape::{
    drape_design, Aabb, DrapeOptions, DrapePattern, Triangle, TriangleMesh,
};
use dry_core::resolve::{resolve_checked, ResolveParams};

#[test]
fn aabb_ray_intersection() {
    let aabb = Aabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
    // Downward ray from (5, 5, 20)
    let hit = aabb.ray_intersect([5.0, 5.0, 20.0], [0.0, 0.0, -1.0]);
    assert!(hit.is_some());
    let (tmin, tmax) = hit.unwrap();
    assert!((tmin - 10.0).abs() < 1e-6);
    assert!((tmax - 20.0).abs() < 1e-6);

    // Ray missing AABB
    let miss = aabb.ray_intersect([15.0, 5.0, 20.0], [0.0, 0.0, -1.0]);
    assert!(miss.is_none());
}

#[test]
fn triangle_ray_intersection_and_normal() {
    // Horizontal triangle at Z = 5
    let tri = Triangle::new([0.0, 0.0, 5.0], [10.0, 0.0, 5.0], [0.0, 10.0, 5.0]);
    assert!((tri.normal[0]).abs() < 1e-6);
    assert!((tri.normal[1]).abs() < 1e-6);
    assert!((tri.normal[2] - 1.0).abs() < 1e-6);

    let hit = tri.ray_intersect([2.0, 2.0, 20.0], [0.0, 0.0, -1.0]);
    assert!(hit.is_some());
    let (t, n) = hit.unwrap();
    assert!((t - 15.0).abs() < 1e-6);
    assert!((n[2] - 1.0).abs() < 1e-6);

    // Ray outside triangle
    let miss = tri.ray_intersect([8.0, 8.0, 20.0], [0.0, 0.0, -1.0]);
    assert!(miss.is_none());
}

#[test]
fn obj_and_stl_parsers() {
    let obj_text = r#"
# Simple pyramid
v 0.0 0.0 0.0
v 10.0 0.0 0.0
v 10.0 10.0 0.0
v 0.0 10.0 0.0
v 5.0 5.0 10.0
f 1 2 5
f 2 3 5
f 3 4 5
f 4 1 5
"#;
    let mesh_obj = TriangleMesh::from_obj(obj_text).expect("OBJ parsing should succeed");
    assert_eq!(mesh_obj.triangles.len(), 4);
    assert!((mesh_obj.bounds.max[2] - 10.0).abs() < 1e-6);

    // ASCII STL
    let stl_ascii = r#"
solid test_tri
  facet normal 0.0 0.0 1.0
    outer loop
      vertex 0.0 0.0 0.0
      vertex 10.0 0.0 0.0
      vertex 5.0 10.0 0.0
    endloop
  endfacet
endsolid test_tri
"#;
    let mesh_stl =
        TriangleMesh::from_stl(stl_ascii.as_bytes()).expect("ASCII STL parsing should succeed");
    assert_eq!(mesh_stl.triangles.len(), 1);
}

#[test]
fn mesh_5axis_drape_end_to_end() {
    // Create a 2-triangle sloped roof: (0,0,0) to (20,20,10)
    let t1 = Triangle::new([0.0, 0.0, 0.0], [20.0, 0.0, 10.0], [20.0, 20.0, 10.0]);
    let t2 = Triangle::new([0.0, 0.0, 0.0], [20.0, 20.0, 10.0], [0.0, 20.0, 0.0]);
    let mesh = TriangleMesh::new(vec![t1, t2]);

    let options = DrapeOptions {
        mesh,
        pattern: DrapePattern::ZigZagX,
        x_range: Some([2.0, 18.0]),
        y_range: Some([2.0, 18.0]),
        stepover: 4.0,
        resolution: 2.0,
        standoff_offset: 0.5,
        safe_z: Some(25.0),
        feedrate: 2000.0,
        plunge_feed: 500.0,
        width: 0.45,
        height: 0.2,
    };

    let design = drape_design(&options).expect("drape design generation should succeed");
    assert!(!design.ops.is_empty());

    let tp = resolve_checked(&design, &ResolveParams::default()).expect("resolve should succeed");
    assert!(!tp.segments.is_empty());

    // Verify all extruding segments carry valid unit orientations
    let mut oriented_count = 0;
    for seg in &tp.segments {
        if !seg.travel {
            assert!(
                seg.orientation.is_some(),
                "Conformal 5-axis move must carry orientation normal"
            );
            let [i, j, k] = seg.orientation.unwrap();
            let norm = (i * i + j * j + k * k).sqrt();
            assert!(
                (norm - 1.0).abs() < 1e-4,
                "Surface normal orientation must be unit length"
            );
            assert!(
                k > 0.0,
                "Upward surface normal must have positive Z component"
            );
            oriented_count += 1;
        }
    }
    assert!(oriented_count > 0);
}
