//! `features` — expand the first bounded L0 feature graph into the existing L1 [`Op`] sequence.
//!
//! P2.3 deliberately stays planar and FFF-oriented: a feature owns a coordinate-local L1 op list and
//! a pose consisting of XYZ translation plus rotation about Z. Process/channel state retains normal L1
//! sequence semantics. Groups preserve order; repeats compose a step pose. Full 3D named coordinate
//! frames belong to D1.3.

use crate::resolve::{Design, Op};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

pub const DEFAULT_MAX_EXPANDED_OPS: usize = 1_000_000;
pub const DEFAULT_MAX_EXPANDED_NODES: usize = 100_000;
pub const DEFAULT_MAX_FEATURE_DEPTH: usize = 64;

/// A planar feature pose. Translation is in millimetres and rotation is in degrees about +Z.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeaturePose {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub z: f64,
    #[serde(default)]
    pub rotate_z_deg: f64,
}

/// One node in the first bounded L0 feature graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeatureNode {
    /// A coordinate-local L1 op sequence placed at `pose`.
    Feature {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default)]
        pose: FeaturePose,
        ops: Vec<Op>,
    },
    /// Ordered composition. Children expand in source order.
    Group { children: Vec<FeatureNode> },
    /// Repeat `child` `count` times. Instance zero uses the identity; each later instance composes one
    /// additional `step`.
    Repeat {
        count: u32,
        #[serde(default)]
        step: FeaturePose,
        child: Box<FeatureNode>,
    },
}

/// A versionless internal L0 program for P2.3. Its public stabilization/versioning belongs to D1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureProgram {
    #[serde(default)]
    pub features: Vec<FeatureNode>,
}

/// Resource limits applied before an expanded L1 design can exhaust memory or recursion depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandLimits {
    pub max_ops: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
}

impl Default for ExpandLimits {
    fn default() -> Self {
        Self {
            max_ops: DEFAULT_MAX_EXPANDED_OPS,
            max_nodes: DEFAULT_MAX_EXPANDED_NODES,
            max_depth: DEFAULT_MAX_FEATURE_DEPTH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandError {
    message: String,
}

impl ExpandError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExpandError {}

#[derive(Debug, Clone, Copy)]
struct Transform {
    cos: f64,
    sin: f64,
    translation: [f64; 3],
}

impl Transform {
    fn identity() -> Self {
        Self {
            cos: 1.0,
            sin: 0.0,
            translation: [0.0; 3],
        }
    }

    fn from_pose(pose: FeaturePose, path: &str) -> Result<Self, ExpandError> {
        for (name, value) in [
            ("x", pose.x),
            ("y", pose.y),
            ("z", pose.z),
            ("rotate_z_deg", pose.rotate_z_deg),
        ] {
            if !value.is_finite() {
                return Err(ExpandError::new(format!(
                    "{path}.{name} must be finite, got {value}"
                )));
            }
        }
        let angle = radians_from_degrees(pose.rotate_z_deg);
        Ok(Self {
            cos: libm::cos(angle),
            sin: libm::sin(angle),
            translation: [pose.x, pose.y, pose.z],
        })
    }

    /// Compose `self` (parent) with `local`: `self(local(point))`.
    fn compose(self, local: Self) -> Self {
        let [lx, ly, lz] = local.translation;
        let translated = self.apply_vector([lx, ly, lz]);
        Self {
            cos: self.cos * local.cos - self.sin * local.sin,
            sin: self.sin * local.cos + self.cos * local.sin,
            translation: [
                translated[0] + self.translation[0],
                translated[1] + self.translation[1],
                translated[2] + self.translation[2],
            ],
        }
    }

    fn apply_point(self, point: [f64; 3]) -> [f64; 3] {
        let rotated = self.apply_vector(point);
        [
            rotated[0] + self.translation[0],
            rotated[1] + self.translation[1],
            rotated[2] + self.translation[2],
        ]
    }

    fn apply_xy(self, point: [f64; 2]) -> [f64; 2] {
        [
            self.cos * point[0] - self.sin * point[1] + self.translation[0],
            self.sin * point[0] + self.cos * point[1] + self.translation[1],
        ]
    }

    fn apply_vector(self, vector: [f64; 3]) -> [f64; 3] {
        [
            self.cos * vector[0] - self.sin * vector[1],
            self.sin * vector[0] + self.cos * vector[1],
            vector[2],
        ]
    }

    fn is_identity(self) -> bool {
        const EPS: f64 = 1e-12;
        (self.cos - 1.0).abs() <= EPS
            && self.sin.abs() <= EPS
            && self.translation.iter().all(|value| value.abs() <= EPS)
    }
}

fn radians_from_degrees(degrees: f64) -> f64 {
    degrees * PI / 180.0
}

struct ExpandState {
    limits: ExpandLimits,
    nodes: usize,
    ops: Vec<Op>,
}

impl ExpandState {
    fn new(limits: ExpandLimits) -> Self {
        Self {
            limits,
            nodes: 0,
            ops: Vec::new(),
        }
    }

    fn visit_node(&mut self, path: &str) -> Result<(), ExpandError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| ExpandError::new("expanded node count overflowed usize"))?;
        if self.nodes > self.limits.max_nodes {
            return Err(ExpandError::new(format!(
                "{path} exceeds max expanded nodes ({})",
                self.limits.max_nodes
            )));
        }
        Ok(())
    }

    fn push_op(&mut self, op: Op, path: &str) -> Result<(), ExpandError> {
        if self.ops.len() >= self.limits.max_ops {
            return Err(ExpandError::new(format!(
                "{path} exceeds max expanded ops ({})",
                self.limits.max_ops
            )));
        }
        self.ops.push(op);
        Ok(())
    }
}

/// Expand a bounded L0 feature program to the current L1 design.
pub fn expand_features(program: &FeatureProgram) -> Result<Design, ExpandError> {
    expand_features_with_limits(program, ExpandLimits::default())
}

pub fn expand_features_with_limits(
    program: &FeatureProgram,
    limits: ExpandLimits,
) -> Result<Design, ExpandError> {
    let mut state = ExpandState::new(limits);
    for (index, feature) in program.features.iter().enumerate() {
        expand_node(
            feature,
            Transform::identity(),
            0,
            &format!("features[{index}]"),
            &mut state,
        )?;
    }
    Ok(Design { ops: state.ops })
}

fn expand_node(
    node: &FeatureNode,
    parent: Transform,
    depth: usize,
    path: &str,
    state: &mut ExpandState,
) -> Result<(), ExpandError> {
    if depth > state.limits.max_depth {
        return Err(ExpandError::new(format!(
            "{path} exceeds max feature depth ({})",
            state.limits.max_depth
        )));
    }
    state.visit_node(path)?;

    match node {
        FeatureNode::Feature { name, pose, ops } => {
            if name.as_ref().is_some_and(|value| value.is_empty()) {
                return Err(ExpandError::new(format!("{path}.name must not be empty")));
            }
            let local = Transform::from_pose(*pose, &format!("{path}.pose"))?;
            expand_feature_ops(ops, parent.compose(local), path, state)
        }
        FeatureNode::Group { children } => {
            for (index, child) in children.iter().enumerate() {
                expand_node(
                    child,
                    parent,
                    depth + 1,
                    &format!("{path}.children[{index}]"),
                    state,
                )?;
            }
            Ok(())
        }
        FeatureNode::Repeat { count, step, child } => {
            let step = Transform::from_pose(*step, &format!("{path}.step"))?;
            let mut instance = Transform::identity();
            for index in 0..*count {
                expand_node(
                    child,
                    parent.compose(instance),
                    depth + 1,
                    &format!("{path}.instances[{index}]"),
                    state,
                )?;
                instance = instance.compose(step);
            }
            Ok(())
        }
    }
}

fn expand_feature_ops(
    ops: &[Op],
    transform: Transform,
    path: &str,
    state: &mut ExpandState,
) -> Result<(), ExpandError> {
    let mut position = [None; 3];
    for (index, op) in ops.iter().enumerate() {
        let op_path = format!("{path}.ops[{index}]");
        let transformed = match op {
            Op::Move { x, y, z } => {
                let local = inherit_point([*x, *y, *z], position, &op_path)?;
                position = local.map(Some);
                let point = transform.apply_point(local);
                Op::Move {
                    x: Some(point[0]),
                    y: Some(point[1]),
                    z: Some(point[2]),
                }
            }
            Op::Arc {
                cx,
                cy,
                x,
                y,
                z,
                clockwise,
            } => {
                require_defined_position(position, &op_path)?;
                let local = inherit_point([*x, *y, *z], position, &op_path)?;
                position = local.map(Some);
                let end = transform.apply_point(local);
                let centre = transform.apply_xy([
                    require_finite(*cx, &format!("{op_path}.cx"))?,
                    require_finite(*cy, &format!("{op_path}.cy"))?,
                ]);
                Op::Arc {
                    cx: centre[0],
                    cy: centre[1],
                    x: Some(end[0]),
                    y: Some(end[1]),
                    z: Some(end[2]),
                    clockwise: *clockwise,
                }
            }
            Op::Spline { points } => {
                if !points.is_empty() {
                    require_defined_position(position, &op_path)?;
                }
                let mut transformed = Vec::with_capacity(points.len());
                for (point_index, point) in points.iter().enumerate() {
                    let local = inherit_point(
                        *point,
                        position,
                        &format!("{op_path}.points[{point_index}]"),
                    )?;
                    position = local.map(Some);
                    transformed.push(transform.apply_point(local).map(Some));
                }
                Op::Spline {
                    points: transformed,
                }
            }
            Op::Orient { i, j, k } => {
                let vector = transform.apply_vector([
                    require_finite(*i, &format!("{op_path}.i"))?,
                    require_finite(*j, &format!("{op_path}.j"))?,
                    require_finite(*k, &format!("{op_path}.k"))?,
                ]);
                Op::Orient {
                    i: vector[0],
                    j: vector[1],
                    k: vector[2],
                }
            }
            Op::ManualGcode { .. } if !transform.is_identity() => {
                return Err(ExpandError::new(format!(
                    "{op_path}.manual_gcode cannot be transformed safely"
                )));
            }
            other => other.clone(),
        };
        state.push_op(transformed, &op_path)?;
    }
    Ok(())
}

fn require_defined_position(position: [Option<f64>; 3], path: &str) -> Result<(), ExpandError> {
    if position.iter().all(Option::is_some) {
        Ok(())
    } else {
        Err(ExpandError::new(format!(
            "{path} requires a fully defined local start point"
        )))
    }
}

fn require_finite(value: f64, path: &str) -> Result<f64, ExpandError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ExpandError::new(format!(
            "{path} must be finite, got {value}"
        )))
    }
}

fn inherit_point(
    point: [Option<f64>; 3],
    position: [Option<f64>; 3],
    path: &str,
) -> Result<[f64; 3], ExpandError> {
    let mut out = [0.0; 3];
    for (axis, name) in ["x", "y", "z"].into_iter().enumerate() {
        let value = point[axis].or(position[axis]).ok_or_else(|| {
            ExpandError::new(format!(
                "{path}.{name} is undefined; features must be locally self-contained"
            ))
        })?;
        if !value.is_finite() {
            return Err(ExpandError::new(format!(
                "{path}.{name} must be finite, got {value}"
            )));
        }
        out[axis] = value;
    }
    Ok(out)
}

#[cfg(test)]
#[path = "features/native_numeric_tests.rs"]
mod native_numeric_tests;
