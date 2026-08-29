//! `drape` — Mesh Heightfield 5-Axis Drape Generator with BVH Acceleration (E1.3, `docs/04-tasks.md`).
//!
//! Generates conformal non-planar 3D printing and 5-axis subtractive milling toolpaths draped
//! over 3D triangle meshes (STL/OBJ).
//!
//! Uses a Bounding Volume Hierarchy (BVH) for O(log N) ray-mesh surface projection and computes
//! exact surface-normal orientation vectors `(i, j, k)` for multi-axis kinematic emitters.

use crate::resolve::{Design, Op};
use serde::{Deserialize, Serialize};

/// Axis-Aligned Bounding Box (AABB) in 3D Euclidean space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb {
    pub fn new(min: [f64; 3], max: [f64; 3]) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: &[[f64; 3]]) -> Self {
        if points.is_empty() {
            return Self {
                min: [0.0; 3],
                max: [0.0; 3],
            };
        }
        let mut min = points[0];
        let mut max = points[0];
        for p in points.iter().skip(1) {
            for i in 0..3 {
                if p[i] < min[i] {
                    min[i] = p[i];
                }
                if p[i] > max[i] {
                    max[i] = p[i];
                }
            }
        }
        Self { min, max }
    }

    pub fn union(&self, other: &Aabb) -> Self {
        Self {
            min: [
                self.min[0].min(other.min[0]),
                self.min[1].min(other.min[1]),
                self.min[2].min(other.min[2]),
            ],
            max: [
                self.max[0].max(other.max[0]),
                self.max[1].max(other.max[1]),
                self.max[2].max(other.max[2]),
            ],
        }
    }

    pub fn longest_axis(&self) -> usize {
        let dx = self.max[0] - self.min[0];
        let dy = self.max[1] - self.min[1];
        let dz = self.max[2] - self.min[2];
        if dx >= dy && dx >= dz {
            0
        } else if dy >= dz {
            1
        } else {
            2
        }
    }

    /// Fast Ray-AABB slab intersection test.
    pub fn ray_intersect(&self, origin: [f64; 3], dir: [f64; 3]) -> Option<(f64, f64)> {
        let mut tmin = f64::NEG_INFINITY;
        let mut tmax = f64::INFINITY;

        for i in 0..3 {
            if dir[i].abs() < 1e-12 {
                if origin[i] < self.min[i] || origin[i] > self.max[i] {
                    return None;
                }
            } else {
                let inv_d = 1.0 / dir[i];
                let mut t1 = (self.min[i] - origin[i]) * inv_d;
                let mut t2 = (self.max[i] - origin[i]) * inv_d;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some((tmin, tmax))
    }
}

/// A 3D planar triangle with vertices `v0, v1, v2` and pre-computed unit normal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Triangle {
    pub v0: [f64; 3],
    pub v1: [f64; 3],
    pub v2: [f64; 3],
    pub normal: [f64; 3],
}

impl Triangle {
    pub fn new(v0: [f64; 3], v1: [f64; 3], v2: [f64; 3]) -> Self {
        let e1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let e2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
        let nx = e1[1] * e2[2] - e1[2] * e2[1];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        let nz = e1[0] * e2[1] - e1[1] * e2[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        let normal = if len > 1e-12 {
            [nx / len, ny / len, nz / len]
        } else {
            [0.0, 0.0, 1.0]
        };
        Self {
            v0,
            v1,
            v2,
            normal,
        }
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(&[self.v0, self.v1, self.v2])
    }

    pub fn centroid(&self) -> [f64; 3] {
        [
            (self.v0[0] + self.v1[0] + self.v2[0]) / 3.0,
            (self.v0[1] + self.v1[1] + self.v2[1]) / 3.0,
            (self.v0[2] + self.v1[2] + self.v2[2]) / 3.0,
        ]
    }

    /// Möller–Trumbore Ray-Triangle intersection algorithm.
    /// Returns `Some((t, [nx, ny, nz]))` where `t` is the distance along ray.
    pub fn ray_intersect(&self, origin: [f64; 3], dir: [f64; 3]) -> Option<(f64, [f64; 3])> {
        let e1 = [
            self.v1[0] - self.v0[0],
            self.v1[1] - self.v0[1],
            self.v1[2] - self.v0[2],
        ];
        let e2 = [
            self.v2[0] - self.v0[0],
            self.v2[1] - self.v0[1],
            self.v2[2] - self.v0[2],
        ];

        let pvec = [
            dir[1] * e2[2] - dir[2] * e2[1],
            dir[2] * e2[0] - dir[0] * e2[2],
            dir[0] * e2[1] - dir[1] * e2[0],
        ];

        let det = e1[0] * pvec[0] + e1[1] * pvec[1] + e1[2] * pvec[2];

        if det.abs() < 1e-12 {
            return None;
        }

        let inv_det = 1.0 / det;
        let tvec = [
            origin[0] - self.v0[0],
            origin[1] - self.v0[1],
            origin[2] - self.v0[2],
        ];

        let u = (tvec[0] * pvec[0] + tvec[1] * pvec[1] + tvec[2] * pvec[2]) * inv_det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let qvec = [
            tvec[1] * e1[2] - tvec[2] * e1[1],
            tvec[2] * e1[0] - tvec[0] * e1[2],
            tvec[0] * e1[1] - tvec[1] * e1[0],
        ];

        let v = (dir[0] * qvec[0] + dir[1] * qvec[1] + dir[2] * qvec[2]) * inv_det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = (e2[0] * qvec[0] + e2[1] * qvec[1] + e2[2] * qvec[2]) * inv_det;
        if t < 0.0 {
            return None;
        }

        // Ensure normal points upward against downward ray
        let n = if self.normal[2] < 0.0 {
            [-self.normal[0], -self.normal[1], -self.normal[2]]
        } else {
            self.normal
        };

        Some((t, n))
    }
}

/// A Bounding Volume Hierarchy node.
#[derive(Debug, Clone)]
pub struct BvhNode {
    pub aabb: Aabb,
    pub left: Option<Box<BvhNode>>,
    pub right: Option<Box<BvhNode>>,
    pub triangles: Vec<Triangle>,
}

impl BvhNode {
    pub fn build(mut triangles: Vec<Triangle>, leaf_size: usize) -> Self {
        if triangles.is_empty() {
            return Self {
                aabb: Aabb::new([0.0; 3], [0.0; 3]),
                left: None,
                right: None,
                triangles: Vec::new(),
            };
        }

        let mut aabb = triangles[0].aabb();
        for tri in triangles.iter().skip(1) {
            aabb = aabb.union(&tri.aabb());
        }

        if triangles.len() <= leaf_size {
            return Self {
                aabb,
                left: None,
                right: None,
                triangles,
            };
        }

        let axis = aabb.longest_axis();
        triangles.sort_unstable_by(|a, b| {
            a.centroid()[axis]
                .partial_cmp(&b.centroid()[axis])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mid = triangles.len() / 2;
        let right_tris = triangles.split_off(mid);
        let left_node = Self::build(triangles, leaf_size);
        let right_node = Self::build(right_tris, leaf_size);

        Self {
            aabb,
            left: Some(Box::new(left_node)),
            right: Some(Box::new(right_node)),
            triangles: Vec::new(),
        }
    }

    /// Recursively trace downward ray and find highest intersection point.
    pub fn ray_cast(&self, origin: [f64; 3], dir: [f64; 3]) -> Option<(f64, [f64; 3])> {
        self.aabb.ray_intersect(origin, dir)?;

        if self.left.is_none() && self.right.is_none() {
            let mut closest_hit: Option<(f64, [f64; 3])> = None;
            for tri in &self.triangles {
                if let Some((t, n)) = tri.ray_intersect(origin, dir) {
                    match closest_hit {
                        None => closest_hit = Some((t, n)),
                        Some((cur_t, _)) if t < cur_t => closest_hit = Some((t, n)),
                        _ => {}
                    }
                }
            }
            return closest_hit;
        }

        let hit_left = self.left.as_ref().and_then(|l| l.ray_cast(origin, dir));
        let hit_right = self.right.as_ref().and_then(|r| r.ray_cast(origin, dir));

        match (hit_left, hit_right) {
            (Some((t1, n1)), Some((t2, n2))) => {
                if t1 <= t2 {
                    Some((t1, n1))
                } else {
                    Some((t2, n2))
                }
            }
            (Some(h), None) | (None, Some(h)) => Some(h),
            (None, None) => None,
        }
    }
}

/// 3D Triangle Mesh containing geometry and accelerated BVH tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangleMesh {
    pub triangles: Vec<Triangle>,
    pub bounds: Aabb,
    #[serde(skip)]
    pub bvh: Option<BvhNode>,
}

impl TriangleMesh {
    pub fn new(triangles: Vec<Triangle>) -> Self {
        let bounds = if triangles.is_empty() {
            Aabb::new([0.0; 3], [0.0; 3])
        } else {
            let mut b = triangles[0].aabb();
            for tri in triangles.iter().skip(1) {
                b = b.union(&tri.aabb());
            }
            b
        };
        let bvh = Some(BvhNode::build(triangles.clone(), 4));
        Self {
            triangles,
            bounds,
            bvh,
        }
    }

    pub fn ensure_bvh(&mut self) {
        if self.bvh.is_none() {
            self.bvh = Some(BvhNode::build(self.triangles.clone(), 4));
        }
    }

    /// Cast a vertical ray downwards at `(x, y)` from clearance height `z_start`.
    /// Returns `Some((hit_z, normal))` if surface is hit.
    pub fn project_point(&self, x: f64, y: f64, z_start: f64) -> Option<(f64, [f64; 3])> {
        let origin = [x, y, z_start];
        let dir = [0.0, 0.0, -1.0];
        if let Some(ref bvh) = self.bvh {
            bvh.ray_cast(origin, dir).map(|(t, n)| (z_start - t, n))
        } else {
            let mut closest: Option<(f64, [f64; 3])> = None;
            for tri in &self.triangles {
                if let Some((t, n)) = tri.ray_intersect(origin, dir) {
                    match closest {
                        None => closest = Some((t, n)),
                        Some((cur_t, _)) if t < cur_t => closest = Some((t, n)),
                        _ => {}
                    }
                }
            }
            closest.map(|(t, n)| (z_start - t, n))
        }
    }

    /// Parse Wavefront OBJ file content.
    pub fn from_obj(text: &str) -> Result<Self, DrapeError> {
        let mut vertices: Vec<[f64; 3]> = Vec::new();
        let mut triangles: Vec<Triangle> = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            if parts[0] == "v" && parts.len() >= 4 {
                let x: f64 = parts[1]
                    .parse()
                    .map_err(|e| DrapeError::InvalidMesh(format!("invalid vertex x: {e}")))?;
                let y: f64 = parts[2]
                    .parse()
                    .map_err(|e| DrapeError::InvalidMesh(format!("invalid vertex y: {e}")))?;
                let z: f64 = parts[3]
                    .parse()
                    .map_err(|e| DrapeError::InvalidMesh(format!("invalid vertex z: {e}")))?;
                vertices.push([x, y, z]);
            } else if parts[0] == "f" && parts.len() >= 4 {
                let parse_idx = |s: &str| -> Result<usize, DrapeError> {
                    let token = s.split('/').next().unwrap_or("");
                    let idx: i64 = token
                        .parse()
                        .map_err(|e| DrapeError::InvalidMesh(format!("invalid face index: {e}")))?;
                    if idx > 0 {
                        Ok((idx - 1) as usize)
                    } else if idx < 0 {
                        Ok((vertices.len() as i64 + idx) as usize)
                    } else {
                        Err(DrapeError::InvalidMesh("zero face index".into()))
                    }
                };

                let i0 = parse_idx(parts[1])?;
                let i1 = parse_idx(parts[2])?;
                let i2 = parse_idx(parts[3])?;

                if i0 >= vertices.len() || i1 >= vertices.len() || i2 >= vertices.len() {
                    return Err(DrapeError::InvalidMesh("face index out of bounds".into()));
                }

                triangles.push(Triangle::new(vertices[i0], vertices[i1], vertices[i2]));

                // Quad / polygon fan triangulation
                for k in 4..parts.len() {
                    let ik = parse_idx(parts[k])?;
                    let i_prev = parse_idx(parts[k - 1])?;
                    if ik < vertices.len() && i_prev < vertices.len() {
                        triangles.push(Triangle::new(vertices[i0], vertices[i_prev], vertices[ik]));
                    }
                }
            }
        }

        if triangles.is_empty() {
            return Err(DrapeError::InvalidMesh(
                "no valid triangles found in OBJ".into(),
            ));
        }

        Ok(Self::new(triangles))
    }

    /// Parse ASCII or Binary STL file bytes.
    pub fn from_stl(bytes: &[u8]) -> Result<Self, DrapeError> {
        // Try ASCII STL first if it decodes as UTF-8 and contains STL keywords
        if let Ok(text) = std::str::from_utf8(bytes) {
            let trimmed = text.trim();
            if trimmed.starts_with("solid") && (trimmed.contains("facet") || trimmed.contains("vertex")) {
                return Self::from_stl_ascii(trimmed);
            }
        }

        // Binary STL
        if bytes.len() < 84 {
            return Err(DrapeError::InvalidMesh("STL file too short".into()));
        }

        Self::from_stl_binary(bytes)
    }

    fn from_stl_ascii(text: &str) -> Result<Self, DrapeError> {
        let mut triangles: Vec<Triangle> = Vec::new();
        let mut current_verts: Vec<[f64; 3]> = Vec::new();

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("vertex") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let x: f64 = parts[1]
                        .parse()
                        .map_err(|e| DrapeError::InvalidMesh(format!("invalid vertex x: {e}")))?;
                    let y: f64 = parts[2]
                        .parse()
                        .map_err(|e| DrapeError::InvalidMesh(format!("invalid vertex y: {e}")))?;
                    let z: f64 = parts[3]
                        .parse()
                        .map_err(|e| DrapeError::InvalidMesh(format!("invalid vertex z: {e}")))?;
                    current_verts.push([x, y, z]);
                }
            } else if line.starts_with("endfacet") {
                if current_verts.len() >= 3 {
                    triangles.push(Triangle::new(
                        current_verts[0],
                        current_verts[1],
                        current_verts[2],
                    ));
                }
                current_verts.clear();
            }
        }

        if triangles.is_empty() {
            return Err(DrapeError::InvalidMesh(
                "no valid triangles in ASCII STL".into(),
            ));
        }

        Ok(Self::new(triangles))
    }

    fn from_stl_binary(bytes: &[u8]) -> Result<Self, DrapeError> {
        let num_triangles = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
        let expected_len = 84 + num_triangles * 50;
        if bytes.len() < expected_len {
            return Err(DrapeError::InvalidMesh(format!(
                "binary STL truncated: expected {expected_len} bytes, got {}",
                bytes.len()
            )));
        }

        let mut triangles = Vec::with_capacity(num_triangles);
        let mut offset = 84;

        for _ in 0..num_triangles {
            let read_f32 = |off: usize| -> f64 {
                f32::from_le_bytes([
                    bytes[off],
                    bytes[off + 1],
                    bytes[off + 2],
                    bytes[off + 3],
                ]) as f64
            };

            let v0 = [
                read_f32(offset + 12),
                read_f32(offset + 16),
                read_f32(offset + 20),
            ];
            let v1 = [
                read_f32(offset + 24),
                read_f32(offset + 28),
                read_f32(offset + 32),
            ];
            let v2 = [
                read_f32(offset + 36),
                read_f32(offset + 40),
                read_f32(offset + 44),
            ];

            triangles.push(Triangle::new(v0, v1, v2));
            offset += 50;
        }

        Ok(Self::new(triangles))
    }
}

/// Draping toolpath pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DrapePattern {
    #[default]
    RasterX,
    RasterY,
    #[serde(alias = "zigzag-x", alias = "zigzag_x", alias = "zigzagX")]
    ZigZagX,
    #[serde(alias = "zigzag-y", alias = "zigzag_y", alias = "zigzagY")]
    ZigZagY,
    #[serde(alias = "spiral-concentric", alias = "spiral_concentric", alias = "spiralConcentric")]
    SpiralConcentric,
}

/// Configuration options for mesh draping toolpath generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrapeOptions {
    /// 3D Triangle mesh to drape onto.
    pub mesh: TriangleMesh,
    /// Toolpath pattern.
    #[serde(default)]
    pub pattern: DrapePattern,
    /// Custom X bounds `[min_x, max_x]` (defaults to mesh bounds).
    #[serde(default)]
    pub x_range: Option<[f64; 2]>,
    /// Custom Y bounds `[min_y, max_y]` (defaults to mesh bounds).
    #[serde(default)]
    pub y_range: Option<[f64; 2]>,
    /// Stepover line pitch (mm). Default 1.0 mm.
    #[serde(default = "default_stepover")]
    pub stepover: f64,
    /// Sampling point resolution along path (mm). Default 0.5 mm.
    #[serde(default = "default_resolution")]
    pub resolution: f64,
    /// Normal standoff distance offset (mm). Default 0.0.
    #[serde(default)]
    pub standoff_offset: f64,
    /// Clearance safe Z plane (mm). Default mesh max_z + 10.0.
    #[serde(default)]
    pub safe_z: Option<f64>,
    /// Print/milling feedrate (mm/min). Default 1800.0.
    #[serde(default = "default_feedrate")]
    pub feedrate: f64,
    /// Plunge feedrate (mm/min). Default 600.0.
    #[serde(default = "default_plunge_feed")]
    pub plunge_feed: f64,
    /// Extrusion bead width (mm). Default 0.45.
    #[serde(default = "default_width")]
    pub width: f64,
    /// Extrusion bead height (mm). Default 0.2.
    #[serde(default = "default_height")]
    pub height: f64,
}

fn default_stepover() -> f64 {
    1.0
}
fn default_resolution() -> f64 {
    0.5
}
fn default_feedrate() -> f64 {
    1800.0
}
fn default_plunge_feed() -> f64 {
    600.0
}
fn default_width() -> f64 {
    0.45
}
fn default_height() -> f64 {
    0.2
}

/// Errors occurring during mesh draping.
#[derive(Debug, Clone, PartialEq)]
pub enum DrapeError {
    InvalidMesh(String),
    InvalidParameters(String),
    NoSurfaceHit(String),
}

impl std::fmt::Display for DrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidMesh(msg) => write!(f, "Invalid mesh: {msg}"),
            Self::InvalidParameters(msg) => write!(f, "Invalid drape parameters: {msg}"),
            Self::NoSurfaceHit(msg) => write!(f, "No surface hit: {msg}"),
        }
    }
}

impl std::error::Error for DrapeError {}

/// Generate conformal 5-axis draping ops over the provided mesh.
pub fn drape_ops(options: &DrapeOptions) -> Result<Vec<Op>, DrapeError> {
    if options.stepover <= 0.0 || !options.stepover.is_finite() {
        return Err(DrapeError::InvalidParameters("stepover must be positive and finite".into()));
    }
    if options.resolution <= 0.0 || !options.resolution.is_finite() {
        return Err(DrapeError::InvalidParameters("resolution must be positive and finite".into()));
    }

    let mut mesh = options.mesh.clone();
    mesh.ensure_bvh();

    let x_bounds = options
        .x_range
        .unwrap_or([mesh.bounds.min[0], mesh.bounds.max[0]]);
    let y_bounds = options
        .y_range
        .unwrap_or([mesh.bounds.min[1], mesh.bounds.max[1]]);

    let safe_z = options
        .safe_z
        .unwrap_or(mesh.bounds.max[2] + 10.0);

    if x_bounds[0] >= x_bounds[1] || y_bounds[0] >= y_bounds[1] {
        return Err(DrapeError::InvalidParameters("invalid bounding interval".into()));
    }

    let mut ops = Vec::new();

    // Setup initial state
    ops.push(Op::Geometry {
        width: Some(options.width),
        height: Some(options.height),
    });
    ops.push(Op::Speed {
        print: options.feedrate,
    });
    ops.push(Op::Extruder { on: false });

    // Generate passes based on pattern
    let passes: Vec<Vec<([f64; 3], [f64; 3])>> = match options.pattern {
        DrapePattern::SpiralConcentric => {
            let cx = (x_bounds[0] + x_bounds[1]) / 2.0;
            let cy = (y_bounds[0] + y_bounds[1]) / 2.0;
            let max_radius = ((x_bounds[1] - x_bounds[0]).hypot(y_bounds[1] - y_bounds[0])) / 2.0;
            let num_rings = ((max_radius / options.stepover).ceil() as usize).max(1);
            let mut all_passes = Vec::new();

            for ring in 1..=num_rings {
                let radius = ring as f64 * options.stepover;
                let circumference = 2.0 * std::f64::consts::PI * radius;
                let num_samples = (circumference / options.resolution).ceil().max(12.0) as usize;
                let mut pass_points = Vec::new();

                for s in 0..=num_samples {
                    let angle = 2.0 * std::f64::consts::PI * (s as f64 / num_samples as f64);
                    let x = cx + radius * angle.cos();
                    let y = cy + radius * angle.sin();
                    if let Some((hit_z, normal)) = mesh.project_point(x, y, safe_z) {
                        let px = x + options.standoff_offset * normal[0];
                        let py = y + options.standoff_offset * normal[1];
                        let pz = hit_z + options.standoff_offset * normal[2];
                        pass_points.push(([px, py, pz], normal));
                    }
                }
                if !pass_points.is_empty() {
                    all_passes.push(pass_points);
                }
            }
            all_passes
        }
        _ => {
            let mut all_passes = Vec::new();
            let y_steps = ((y_bounds[1] - y_bounds[0]) / options.stepover).ceil() as usize;
            let x_samples = ((x_bounds[1] - x_bounds[0]) / options.resolution).ceil() as usize;

            for yi in 0..=y_steps {
                let y = (y_bounds[0] + yi as f64 * options.stepover).min(y_bounds[1]);
                let reverse_x = (options.pattern == DrapePattern::ZigZagX) && (yi % 2 == 1);
                let mut pass_points = Vec::new();

                for xi in 0..=x_samples {
                    let sample_idx = if reverse_x { x_samples - xi } else { xi };
                    let x = (x_bounds[0] + sample_idx as f64 * options.resolution).min(x_bounds[1]);

                    if let Some((hit_z, normal)) = mesh.project_point(x, y, safe_z) {
                        let px = x + options.standoff_offset * normal[0];
                        let py = y + options.standoff_offset * normal[1];
                        let pz = hit_z + options.standoff_offset * normal[2];
                        pass_points.push(([px, py, pz], normal));
                    }
                }
                if !pass_points.is_empty() {
                    all_passes.push(pass_points);
                }
            }
            all_passes
        }
    };

    let mut total_hits = 0;

    for pass_points in passes {
        total_hits += pass_points.len();

        let (first_p, first_n) = pass_points[0];

        // 1. Retract/travel at safe_z
        ops.push(Op::Extruder { on: false });
        ops.push(Op::Speed {
            print: options.feedrate * 2.0,
        });
        ops.push(Op::Move {
            x: Some(first_p[0]),
            y: Some(first_p[1]),
            z: Some(safe_z),
        });

        // 2. Set tool orientation
        ops.push(Op::Orient {
            i: first_n[0],
            j: first_n[1],
            k: first_n[2],
        });

        // 3. Plunge down
        ops.push(Op::Speed {
            print: options.plunge_feed,
        });
        ops.push(Op::Move {
            x: Some(first_p[0]),
            y: Some(first_p[1]),
            z: Some(first_p[2]),
        });

        // 4. Trace conformal surface
        ops.push(Op::Extruder { on: true });
        ops.push(Op::Speed {
            print: options.feedrate,
        });

        for (pt, n) in pass_points.iter().skip(1) {
            ops.push(Op::Orient {
                i: n[0],
                j: n[1],
                k: n[2],
            });
            ops.push(Op::Move {
                x: Some(pt[0]),
                y: Some(pt[1]),
                z: Some(pt[2]),
            });
        }
    }


    if total_hits == 0 {
        return Err(DrapeError::NoSurfaceHit(
            "no surface intersections found across requested bounding domain".into(),
        ));
    }

    // Final retract
    ops.push(Op::Extruder { on: false });
    ops.push(Op::Move {
        x: None,
        y: None,
        z: Some(safe_z),
    });

    Ok(ops)
}

/// Generate a complete Dry `Design` from mesh draping options.
pub fn drape_design(options: &DrapeOptions) -> Result<Design, DrapeError> {
    let ops = drape_ops(options)?;
    Ok(Design { ops })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_dome_mesh() -> TriangleMesh {
        // Two triangles forming a simple roof/dome
        let t1 = Triangle::new([0.0, 0.0, 5.0], [20.0, 0.0, 0.0], [20.0, 20.0, 0.0]);
        let t2 = Triangle::new([0.0, 0.0, 5.0], [20.0, 20.0, 0.0], [0.0, 20.0, 5.0]);
        TriangleMesh::new(vec![t1, t2])
    }

    #[test]
    fn test_drape_spiral_concentric() {
        let mesh = sample_dome_mesh();
        let opts = DrapeOptions {
            mesh,
            pattern: DrapePattern::SpiralConcentric,
            x_range: Some([5.0, 15.0]),
            y_range: Some([5.0, 15.0]),
            stepover: 2.0,
            resolution: 1.0,
            standoff_offset: 0.5,
            safe_z: Some(15.0),
            feedrate: 1800.0,
            plunge_feed: 600.0,
            width: 0.45,
            height: 0.2,
        };
        let ops = drape_ops(&opts).expect("should generate spiral concentric drape ops");
        assert!(!ops.is_empty());
        // Verify tool orientations are generated
        let orient_count = ops.iter().filter(|op| matches!(op, Op::Orient { .. })).count();
        assert!(orient_count > 5);
    }
}

