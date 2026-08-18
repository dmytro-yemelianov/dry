//! Named coordinate frames and rigid 3D transforms (D1.2, `docs/20-dry-ir-ecosystem-implementation-plan.md` §6.2).
//!
//! Every geometric point in Dry exists within a coordinate frame. The standard reserved frame IDs are:
//! - `design` (local authoring coordinates)
//! - `workpiece` (workpiece coordinate system / WCS, e.g. G54-G59)
//! - `fixture` (tombstone / fixture plate coordinates)
//! - `tool` (tool center point / TCP frame)
//! - `machine` (physical machine root / joint space)
//!
//! Transforms are 3D rigid Euclidean transforms ($SE(3)$) consisting of a 3D translation $(x,y,z)$ and a
//! unit rotation quaternion $(x, y, z, w)$ with canonical sign normalization.

use serde::{Deserialize, Serialize};

/// Canonical named frame identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FrameId {
    Design,
    Workpiece,
    Fixture,
    Tool,
    Machine,
    #[serde(untagged)]
    Custom(String),
}

impl FrameId {
    /// Return the string identifier of the frame.
    pub fn as_str(&self) -> &str {
        match self {
            FrameId::Design => "design",
            FrameId::Workpiece => "workpiece",
            FrameId::Fixture => "fixture",
            FrameId::Tool => "tool",
            FrameId::Machine => "machine",
            FrameId::Custom(s) => s.as_str(),
        }
    }
}

/// A normalized unit rotation quaternion $(x, y, z, w)$ representing active 3D rotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quaternion {
    /// Identity rotation (0°).
    pub const IDENTITY: Quaternion = Quaternion {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Create a quaternion from raw components, normalizing and ensuring $w \ge 0$ for canonical representation.
    pub fn new(x: f64, y: f64, z: f64, w: f64) -> Self {
        let norm = libm::sqrt(x * x + y * y + z * z + w * w);
        if norm == 0.0 || !norm.is_finite() {
            return Self::IDENTITY;
        }
        let sign = if w < 0.0 { -1.0 } else { 1.0 };
        Quaternion {
            x: (x / norm) * sign,
            y: (y / norm) * sign,
            z: (z / norm) * sign,
            w: (w / norm) * sign,
        }
    }

    /// Construct a rotation of `angle_rad` around normalized axis `(ax, ay, az)`.
    pub fn from_axis_angle(ax: f64, ay: f64, az: f64, angle_rad: f64) -> Self {
        let axis_len = libm::sqrt(ax * ax + ay * ay + az * az);
        if axis_len == 0.0 || !axis_len.is_finite() {
            return Self::IDENTITY;
        }
        let half_angle = angle_rad * 0.5;
        let s = libm::sin(half_angle) / axis_len;
        let c = libm::cos(half_angle);
        Self::new(ax * s, ay * s, az * s, c)
    }

    /// Multiply two quaternions ($q_1 \cdot q_2$).
    pub fn multiply(self, rhs: Quaternion) -> Quaternion {
        let w = self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z;
        let x = self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y;
        let y = self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x;
        let z = self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w;
        Quaternion::new(x, y, z, w)
    }

    /// Rotate a 3D point `(px, py, pz)` by this quaternion.
    pub fn rotate_point(&self, px: f64, py: f64, pz: f64) -> (f64, f64, f64) {
        // v' = v + 2 * r x (r x v + w * v)
        let rx = self.x;
        let ry = self.y;
        let rz = self.z;
        let rw = self.w;

        // t = 2 * (r x p)
        let tx = 2.0 * (ry * pz - rz * py);
        let ty = 2.0 * (rz * px - rx * pz);
        let tz = 2.0 * (rx * py - ry * px);

        // v' = p + w * t + (r x t)
        let vpx = px + rw * tx + (ry * tz - rz * ty);
        let vpy = py + rw * ty + (rz * tx - rx * tz);
        let vpz = pz + rw * tz + (rx * ty - ry * tx);

        (vpx, vpy, vpz)
    }
}

/// A 3D rigid Euclidean transform ($SE(3)$) consisting of translation and rotation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform3D {
    pub translation: [f64; 3],
    pub rotation: Quaternion,
}

impl Transform3D {
    /// Identity transform (no translation, no rotation).
    pub const IDENTITY: Transform3D = Transform3D {
        translation: [0.0, 0.0, 0.0],
        rotation: Quaternion::IDENTITY,
    };

    /// Pure translation transform.
    pub fn from_translation(x: f64, y: f64, z: f64) -> Self {
        Transform3D {
            translation: [x, y, z],
            rotation: Quaternion::IDENTITY,
        }
    }

    /// Pure rotation transform.
    pub fn from_rotation(rotation: Quaternion) -> Self {
        Transform3D {
            translation: [0.0, 0.0, 0.0],
            rotation,
        }
    }

    /// Combine translation and rotation.
    pub fn new(translation: [f64; 3], rotation: Quaternion) -> Self {
        Transform3D {
            translation,
            rotation,
        }
    }

    /// Transform a 3D point $p$ into parent frame: $p' = R \cdot p + T$.
    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        let (rx, ry, rz) = self.rotation.rotate_point(p[0], p[1], p[2]);
        [
            rx + self.translation[0],
            ry + self.translation[1],
            rz + self.translation[2],
        ]
    }

    /// Compose this transform with an inner transform: $T_{composed} = T_{outer} \circ T_{inner}$.
    pub fn compose(&self, inner: &Transform3D) -> Transform3D {
        let (tx, ty, tz) = self.rotation.rotate_point(
            inner.translation[0],
            inner.translation[1],
            inner.translation[2],
        );
        Transform3D {
            translation: [
                self.translation[0] + tx,
                self.translation[1] + ty,
                self.translation[2] + tz,
            ],
            rotation: self.rotation.multiply(inner.rotation),
        }
    }
}
