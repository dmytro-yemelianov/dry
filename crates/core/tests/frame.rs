use dry_core::{FrameId, Quaternion, Transform3D};
use std::f64::consts::PI;

#[test]
fn test_frame_id_canonical_strings() {
    assert_eq!(FrameId::Design.as_str(), "design");
    assert_eq!(FrameId::Workpiece.as_str(), "workpiece");
    assert_eq!(FrameId::Fixture.as_str(), "fixture");
    assert_eq!(FrameId::Tool.as_str(), "tool");
    assert_eq!(FrameId::Machine.as_str(), "machine");
    assert_eq!(FrameId::Custom("part-1".into()).as_str(), "part-1");
}

#[test]
fn test_quaternion_identity_and_rotation() {
    let q_id = Quaternion::IDENTITY;
    let p = (10.0, 0.0, 0.0);
    let (px, py, pz) = q_id.rotate_point(p.0, p.1, p.2);
    assert!((px - 10.0).abs() < 1e-9);
    assert!(py.abs() < 1e-9);
    assert!(pz.abs() < 1e-9);

    // 90 deg rotation around Z
    let q_z90 = Quaternion::from_axis_angle(0.0, 0.0, 1.0, PI / 2.0);
    let (rx, ry, rz) = q_z90.rotate_point(10.0, 0.0, 0.0);
    assert!(rx.abs() < 1e-7);
    assert!((ry - 10.0).abs() < 1e-7);
    assert!(rz.abs() < 1e-7);
}

#[test]
fn test_transform3d_composition() {
    // T1: translate by (10, 0, 0)
    let t1 = Transform3D::from_translation(10.0, 0.0, 0.0);
    // T2: 90 deg rotation around Z
    let q_z90 = Quaternion::from_axis_angle(0.0, 0.0, 1.0, PI / 2.0);
    let t2 = Transform3D::from_rotation(q_z90);

    // Composed: rotate then translate
    let composed = t1.compose(&t2);
    let p = [10.0, 0.0, 0.0];
    let res = composed.transform_point(p);
    // (10, 0, 0) rotated by 90 deg -> (0, 10, 0), then translated by (10, 0, 0) -> (10, 10, 0)
    assert!((res[0] - 10.0).abs() < 1e-7);
    assert!((res[1] - 10.0).abs() < 1e-7);
    assert!(res[2].abs() < 1e-7);
}
