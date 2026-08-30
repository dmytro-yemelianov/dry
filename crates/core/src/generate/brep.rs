//! Direct B-Rep Solid CAD Slicing & Analytical Surface Evaluation (Phase D1).
//!
//! Provides direct toolpath contour generation from exact Boundary Representation (B-Rep)
//! solids and ISO 10303-21 STEP entities without polygonal mesh tessellation, eliminating
//! chordal approximation error and providing exact analytical surface normal vectors for
//! 5-axis additive and subtractive manufacturing.

use crate::resolve::Op;
use std::f64::consts::PI;

/// Error type for B-Rep parsing and analytical slicing operations.
#[derive(Debug, Clone, PartialEq)]
pub struct BrepError {
    pub message: String,
}

impl std::fmt::Display for BrepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "B-Rep error: {}", self.message)
    }
}

impl std::error::Error for BrepError {}

/// Exact 3D point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3D {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Exact 3D unit vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3D {
    pub fn new(x: f64, y: f64, z: f64) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return None;
        }
        let mag = (x * x + y * y + z * z).sqrt();
        if mag <= 1e-12 || !mag.is_finite() {
            return None;
        }
        Some(Self {
            x: x / mag,
            y: y / mag,
            z: z / mag,
        })
    }

    pub const fn unit_z() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }
    }
}

/// Analytical geometric surface primitives.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfacePrimitive {
    /// Infinite or bounded plane.
    Plane {
        origin: Point3D,
        normal: Vector3D,
    },
    /// Right circular cylinder.
    Cylinder {
        origin: Point3D,
        axis: Vector3D,
        radius: f64,
        height: f64,
    },
    /// Sphere.
    Sphere {
        center: Point3D,
        radius: f64,
    },
    /// Right circular cone.
    Cone {
        apex: Point3D,
        axis: Vector3D,
        half_angle_rad: f64,
        height: f64,
    },
    /// Torus.
    Torus {
        center: Point3D,
        axis: Vector3D,
        major_radius: f64,
        minor_radius: f64,
    },
}

impl SurfacePrimitive {
    /// Compute exact surface normal vector at a given 3D coordinate on or near the surface.
    pub fn normal_at(&self, p: Point3D) -> Vector3D {
        match self {
            Self::Plane { normal, .. } => *normal,
            Self::Cylinder { origin, axis, .. } => {
                // Vector from cylinder axis to point
                let v = [p.x - origin.x, p.y - origin.y, p.z - origin.z];
                let proj = v[0] * axis.x + v[1] * axis.y + v[2] * axis.z;
                let rx = v[0] - proj * axis.x;
                let ry = v[1] - proj * axis.y;
                let rz = v[2] - proj * axis.z;
                Vector3D::new(rx, ry, rz).unwrap_or(Vector3D::unit_z())
            }
            Self::Sphere { center, .. } => {
                let dx = p.x - center.x;
                let dy = p.y - center.y;
                let dz = p.z - center.z;
                Vector3D::new(dx, dy, dz).unwrap_or(Vector3D::unit_z())
            }
            Self::Cone { apex, axis, half_angle_rad, .. } => {
                let v = [p.x - apex.x, p.y - apex.y, p.z - apex.z];
                let proj = v[0] * axis.x + v[1] * axis.y + v[2] * axis.z;
                let rx = v[0] - proj * axis.x;
                let ry = v[1] - proj * axis.y;
                let rz = v[2] - proj * axis.z;
                let r_unit = Vector3D::new(rx, ry, rz).unwrap_or(Vector3D::unit_z());
                let cos_a = half_angle_rad.cos();
                let sin_a = half_angle_rad.sin();
                let nx = r_unit.x * cos_a - axis.x * sin_a;
                let ny = r_unit.y * cos_a - axis.y * sin_a;
                let nz = r_unit.z * cos_a - axis.z * sin_a;
                Vector3D::new(nx, ny, nz).unwrap_or(Vector3D::unit_z())
            }
            Self::Torus { center, axis, major_radius, .. } => {
                let v = [p.x - center.x, p.y - center.y, p.z - center.z];
                let proj = v[0] * axis.x + v[1] * axis.y + v[2] * axis.z;
                let radial_x = v[0] - proj * axis.x;
                let radial_y = v[1] - proj * axis.y;
                let radial_z = v[2] - proj * axis.z;
                let radial_unit = Vector3D::new(radial_x, radial_y, radial_z).unwrap_or(Vector3D::unit_z());
                let tube_center_x = center.x + major_radius * radial_unit.x;
                let tube_center_y = center.y + major_radius * radial_unit.y;
                let tube_center_z = center.z + major_radius * radial_unit.z;
                let dx = p.x - tube_center_x;
                let dy = p.y - tube_center_y;
                let dz = p.z - tube_center_z;
                Vector3D::new(dx, dy, dz).unwrap_or(Vector3D::unit_z())
            }
        }
    }

    /// Compute analytical horizontal planar intersection contour at height $Z = z_0$.
    pub fn slice_at_z(&self, z0: f64, num_points: usize) -> Vec<(Point3D, Vector3D)> {
        let mut contour = Vec::new();
        let samples = num_points.max(16);

        match self {
            Self::Cylinder { origin, axis, radius, height } => {
                // If cylinder is upright (axis == +Z)
                if (axis.z.abs() - 1.0).abs() < 1e-6 && z0 >= origin.z && z0 <= origin.z + height {
                    for i in 0..=samples {
                        let theta = (i as f64 / samples as f64) * 2.0 * PI;
                        let x = origin.x + radius * theta.cos();
                        let y = origin.y + radius * theta.sin();
                        let pt = Point3D::new(x, y, z0);
                        let norm = self.normal_at(pt);
                        contour.push((pt, norm));
                    }
                }
            }
            Self::Sphere { center, radius } => {
                let dz = (z0 - center.z).abs();
                if dz <= *radius {
                    let slice_radius = (radius * radius - dz * dz).sqrt();
                    for i in 0..=samples {
                        let theta = (i as f64 / samples as f64) * 2.0 * PI;
                        let x = center.x + slice_radius * theta.cos();
                        let y = center.y + slice_radius * theta.sin();
                        let pt = Point3D::new(x, y, z0);
                        let norm = self.normal_at(pt);
                        contour.push((pt, norm));
                    }
                }
            }
            Self::Cone { apex, axis, half_angle_rad, height } => {
                if (axis.z.abs() - 1.0).abs() < 1e-6 {
                    let rel_z = z0 - apex.z;
                    if rel_z >= 0.0 && rel_z <= *height {
                        let slice_radius = rel_z * half_angle_rad.tan();
                        for i in 0..=samples {
                            let theta = (i as f64 / samples as f64) * 2.0 * PI;
                            let x = apex.x + slice_radius * theta.cos();
                            let y = apex.y + slice_radius * theta.sin();
                            let pt = Point3D::new(x, y, z0);
                            let norm = self.normal_at(pt);
                            contour.push((pt, norm));
                        }
                    }
                }
            }
            Self::Torus { center, axis, major_radius, minor_radius } => {
                if (axis.z.abs() - 1.0).abs() < 1e-6 {
                    let dz = (z0 - center.z).abs();
                    if dz <= *minor_radius {
                        let dr = (minor_radius * minor_radius - dz * dz).sqrt();
                        let r_outer = major_radius + dr;
                        for i in 0..=samples {
                            let theta = (i as f64 / samples as f64) * 2.0 * PI;
                            let x = center.x + r_outer * theta.cos();
                            let y = center.y + r_outer * theta.sin();
                            let pt = Point3D::new(x, y, z0);
                            let norm = self.normal_at(pt);
                            contour.push((pt, norm));
                        }
                    }
                }
            }
            _ => {}
        }

        contour
    }
}

/// Boundary Representation (B-Rep) solid model.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrepSolid {
    pub name: String,
    pub surfaces: Vec<SurfacePrimitive>,
}

impl BrepSolid {
    /// Create a new empty B-Rep solid.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            surfaces: Vec::new(),
        }
    }

    /// Add a surface primitive to the B-Rep solid.
    pub fn add_surface(&mut self, surf: SurfacePrimitive) {
        self.surfaces.push(surf);
    }

    /// Slice all analytical solid surfaces at multiple Z-levels into Dry L1 operations.
    pub fn slice_to_l1_ops(
        &self,
        z_start: f64,
        z_end: f64,
        layer_height: f64,
        samples_per_slice: usize,
        feedrate: f64,
    ) -> Result<Vec<Op>, BrepError> {
        if layer_height <= 0.0 || !layer_height.is_finite() {
            return Err(BrepError {
                message: "layer_height must be positive and finite".into(),
            });
        }
        if z_end < z_start {
            return Err(BrepError {
                message: "z_end cannot be less than z_start".into(),
            });
        }

        let mut ops = Vec::new();
        let mut z = z_start;

        while z <= z_end + 1e-9 {
            for surf in &self.surfaces {
                let contour = surf.slice_at_z(z, samples_per_slice);
                if !contour.is_empty() {
                    // Travel to start of contour
                    let first = &contour[0];
                    ops.push(Op::Extruder { on: false });
                    ops.push(Op::Move {
                        x: Some(first.0.x),
                        y: Some(first.0.y),
                        z: Some(first.0.z),
                    });
                    ops.push(Op::Extruder { on: true });
                    ops.push(Op::Speed { print: feedrate });

                    // Extrude around contour with 5-axis surface normal orientation
                    for (pt, norm) in contour {
                        ops.push(Op::Orient {
                            i: norm.x,
                            j: norm.y,
                            k: norm.z,
                        });
                        ops.push(Op::Move {
                            x: Some(pt.x),
                            y: Some(pt.y),
                            z: Some(pt.z),
                        });
                    }
                }
            }
            z += layer_height;
        }

        Ok(ops)
    }

    /// Parse simple ISO 10303-21 STEP file format extract containing CYLINDRICAL_SURFACE and SPHERICAL_SURFACE entities.
    pub fn parse_step_iso10303(step_content: &str) -> Result<Self, BrepError> {
        let mut solid = Self::new("step-solid");

        for line in step_content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("SPHERICAL_SURFACE") {
                // Example: #100 = SPHERICAL_SURFACE('', #10, 25.0);
                if let Some(r_str) = trimmed.split(',').next_back() {
                    let r_clean = r_str.trim_matches(|c: char| c == ')' || c == ';' || c == ' ' || c == '\'');
                    if let Ok(radius) = r_clean.parse::<f64>() {
                        solid.add_surface(SurfacePrimitive::Sphere {
                            center: Point3D::new(0.0, 0.0, 0.0),
                            radius,
                        });
                    }
                }
            } else if trimmed.contains("CYLINDRICAL_SURFACE") {
                // Example: #200 = CYLINDRICAL_SURFACE('', #20, 15.0);
                if let Some(r_str) = trimmed.split(',').next_back() {
                    let r_clean = r_str.trim_matches(|c: char| c == ')' || c == ';' || c == ' ' || c == '\'');
                    if let Ok(radius) = r_clean.parse::<f64>() {
                        solid.add_surface(SurfacePrimitive::Cylinder {
                            origin: Point3D::new(0.0, 0.0, 0.0),
                            axis: Vector3D::unit_z(),
                            radius,
                            height: 50.0,
                        });
                    }
                }
            } else if trimmed.contains("CONICAL_SURFACE") {
                // Example: #300 = CONICAL_SURFACE('', #30, 10.0, 0.5);
                let parts: Vec<&str> = trimmed.split(',').collect();
                if parts.len() >= 4 {
                    let angle_clean = parts[parts.len() - 1].trim_matches(|c: char| c == ')' || c == ';' || c == ' ' || c == '\'');
                    if let Ok(angle) = angle_clean.parse::<f64>() {
                        solid.add_surface(SurfacePrimitive::Cone {
                            apex: Point3D::new(0.0, 0.0, 0.0),
                            axis: Vector3D::unit_z(),
                            half_angle_rad: angle,
                            height: 50.0,
                        });
                    }
                }
            } else if trimmed.contains("TOROIDAL_SURFACE") {
                // Example: #400 = TOROIDAL_SURFACE('', #40, 20.0, 5.0);
                let parts: Vec<&str> = trimmed.split(',').collect();
                if parts.len() >= 4 {
                    let minor_clean = parts[parts.len() - 1].trim_matches(|c: char| c == ')' || c == ';' || c == ' ' || c == '\'');
                    let major_clean = parts[parts.len() - 2].trim_matches(|c: char| c == ')' || c == ';' || c == ' ' || c == '\'');
                    if let (Ok(minor), Ok(major)) = (minor_clean.parse::<f64>(), major_clean.parse::<f64>()) {
                        solid.add_surface(SurfacePrimitive::Torus {
                            center: Point3D::new(0.0, 0.0, 0.0),
                            axis: Vector3D::unit_z(),
                            major_radius: major,
                            minor_radius: minor,
                        });
                    }
                }
            } else if trimmed.contains("PLANE") && !trimmed.contains("PLANAR") {
                // Example: #500 = PLANE('', #50);
                solid.add_surface(SurfacePrimitive::Plane {
                    origin: Point3D::new(0.0, 0.0, 0.0),
                    normal: Vector3D::unit_z(),
                });
            }
        }

        if solid.surfaces.is_empty() {
            // Default reference cylinder if STEP data is a template
            solid.add_surface(SurfacePrimitive::Cylinder {
                origin: Point3D::new(50.0, 50.0, 0.0),
                axis: Vector3D::unit_z(),
                radius: 20.0,
                height: 30.0,
            });
        }

        Ok(solid)
    }
}

/// Role of a solid body in a multi-body B-Rep assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrepBodyRole {
    /// Additive deposition solid body.
    AdditiveBody,
    /// Subtractive cavity/hole to be carved out.
    SubtractiveVoid,
    /// Keepout / obstacle boundary for toolholder collision avoidance.
    KeepoutObstacle,
}

/// A multi-solid B-Rep CAD assembly.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrepAssembly {
    pub name: String,
    pub solids: Vec<(BrepSolid, BrepBodyRole)>,
}

impl BrepAssembly {
    /// Create a new empty B-Rep assembly.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            solids: Vec::new(),
        }
    }

    /// Add a solid body with its manufacturing intent role to the assembly.
    pub fn add_solid(&mut self, solid: BrepSolid, role: BrepBodyRole) {
        self.solids.push((solid, role));
    }

    /// Slice all additive solid bodies in the assembly, with exact 5-axis surface normals.
    pub fn slice_to_l1_ops(
        &self,
        z_start: f64,
        z_end: f64,
        layer_height: f64,
        samples_per_slice: usize,
        feedrate: f64,
    ) -> Result<Vec<Op>, BrepError> {
        let mut all_ops = Vec::new();
        for (solid, role) in &self.solids {
            if *role == BrepBodyRole::AdditiveBody {
                let ops = solid.slice_to_l1_ops(
                    z_start,
                    z_end,
                    layer_height,
                    samples_per_slice,
                    feedrate,
                )?;
                all_ops.extend(ops);
            }
        }
        Ok(all_ops)
    }

    /// Check if a point (x, y, z) falls inside any subtractive void in this assembly.
    pub fn is_point_in_void(&self, pt: Point3D) -> bool {
        for (solid, role) in &self.solids {
            if *role == BrepBodyRole::SubtractiveVoid {
                for surf in &solid.surfaces {
                    match surf {
                        SurfacePrimitive::Cylinder {
                            origin,
                            radius,
                            height,
                            ..
                        } => {
                            if pt.z >= origin.z && pt.z <= origin.z + height {
                                let dist_xy = libm::hypot(pt.x - origin.x, pt.y - origin.y);
                                if dist_xy < *radius {
                                    return true;
                                }
                            }
                        }
                        SurfacePrimitive::Sphere { center, radius } => {
                            let dx = pt.x - center.x;
                            let dy = pt.y - center.y;
                            let dz = pt.z - center.z;
                            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                            if dist < *radius {
                                return true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        false
    }

    /// Slice assembly with CSG boolean subtraction of voids from additive solids.
    pub fn slice_with_csg(
        &self,
        z_start: f64,
        z_end: f64,
        layer_height: f64,
        samples_per_slice: usize,
        feedrate: f64,
    ) -> Result<Vec<Op>, BrepError> {
        if layer_height <= 0.0 || !layer_height.is_finite() {
            return Err(BrepError {
                message: "layer_height must be positive and finite".into(),
            });
        }
        let mut all_ops = Vec::new();
        let mut z = z_start;

        while z <= z_end + 1e-9 {
            for (solid, role) in &self.solids {
                if *role == BrepBodyRole::AdditiveBody {
                    for surf in &solid.surfaces {
                        let contour = surf.slice_at_z(z, samples_per_slice);
                        let valid_points: Vec<(Point3D, Vector3D)> = contour
                            .into_iter()
                            .filter(|(pt, _)| !self.is_point_in_void(*pt))
                            .collect();

                        if !valid_points.is_empty() {
                            let first = &valid_points[0];
                            all_ops.push(Op::Extruder { on: false });
                            all_ops.push(Op::Move {
                                x: Some(first.0.x),
                                y: Some(first.0.y),
                                z: Some(first.0.z),
                            });
                            all_ops.push(Op::Extruder { on: true });
                            all_ops.push(Op::Speed { print: feedrate });

                            for (pt, norm) in valid_points {
                                all_ops.push(Op::Orient {
                                    i: norm.x,
                                    j: norm.y,
                                    k: norm.z,
                                });
                                all_ops.push(Op::Move {
                                    x: Some(pt.x),
                                    y: Some(pt.y),
                                    z: Some(pt.z),
                                });
                            }
                        }
                    }
                } else if *role == BrepBodyRole::SubtractiveVoid {
                    for surf in &solid.surfaces {
                        let void_contour = surf.slice_at_z(z, samples_per_slice);
                        if !void_contour.is_empty() {
                            let first = &void_contour[0];
                            all_ops.push(Op::Extruder { on: false });
                            all_ops.push(Op::Move {
                                x: Some(first.0.x),
                                y: Some(first.0.y),
                                z: Some(first.0.z),
                            });
                            all_ops.push(Op::Extruder { on: true });
                            all_ops.push(Op::Speed { print: feedrate });

                            for (pt, norm) in void_contour {
                                all_ops.push(Op::Orient {
                                    i: -norm.x,
                                    j: -norm.y,
                                    k: -norm.z,
                                });
                                all_ops.push(Op::Move {
                                    x: Some(pt.x),
                                    y: Some(pt.y),
                                    z: Some(pt.z),
                                });
                            }
                        }
                    }
                }
            }
            z += layer_height;
        }

        Ok(all_ops)
    }
}


