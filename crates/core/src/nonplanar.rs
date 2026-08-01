//! Non-planar toolpath authoring and surface orientation helpers.
//!
//! Provides vector math and surface normal alignment tools for multi-axis 4D/5D printing,
//! non-planar slicing, and conformal tool orientation.

/// Computes the unit normal vector to a triangle defined by three 3D points `p0`, `p1`, and `p2`.
pub fn compute_triangle_normal(
    p0: [f64; 3],
    p1: [f64; 3],
    p2: [f64; 3],
) -> Result<[f64; 3], &'static str> {
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

    let nx = u[1] * v[2] - u[2] * v[1];
    let ny = u[2] * v[0] - u[0] * v[2];
    let nz = u[0] * v[1] - u[1] * v[0];

    let norm = (nx * nx + ny * ny + nz * nz).sqrt();
    if norm < 1e-12 {
        return Err("collinear or degenerate triangle vertices");
    }

    Ok([nx / norm, ny / norm, nz / norm])
}

/// Offsets a 3D point `p` by `distance_mm` along a unit normal vector `normal`.
pub fn offset_along_normal(p: [f64; 3], normal: [f64; 3], distance_mm: f64) -> [f64; 3] {
    [
        p[0] + distance_mm * normal[0],
        p[1] + distance_mm * normal[1],
        p[2] + distance_mm * normal[2],
    ]
}

/// Computes the non-planar Z-coordinate at `(x, y)` given a parametric surface function `surface_fn(x, y)`.
pub fn conformal_surface_z(x: f64, y: f64, surface_fn: impl Fn(f64, f64) -> f64) -> f64 {
    surface_fn(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_normal_points_up_for_xy_plane() {
        let p0 = [0.0, 0.0, 0.0];
        let p1 = [10.0, 0.0, 0.0];
        let p2 = [0.0, 10.0, 0.0];

        let normal = compute_triangle_normal(p0, p1, p2).unwrap();
        assert!((normal[0] - 0.0).abs() < 1e-9);
        assert!((normal[1] - 0.0).abs() < 1e-9);
        assert!((normal[2] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn offset_along_normal_moves_correct_distance() {
        let p = [0.0, 0.0, 5.0];
        let normal = [0.0, 0.0, 1.0];
        let offset_p = offset_along_normal(p, normal, 2.5);

        assert_eq!(offset_p, [0.0, 0.0, 7.5]);
    }
}
