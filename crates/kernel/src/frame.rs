//! SE(3) Pose & Frame Graph Resolution (Milestone D1.3)
//!
//! Provides named coordinate frames (world, machine, workpiece, fixture, tool) and
//! explicit 3D rigid transforms in SE(3) with translation vectors and rotation matrices.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct FrameError {
    pub message: String,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Frame error: {}", self.message)
    }
}

impl std::error::Error for FrameError {}

/// A 3D rigid transformation in SE(3).
#[derive(Debug, Clone, PartialEq)]
pub struct TransformSE3 {
    pub translation: [f64; 3],
    pub rotation: [[f64; 3]; 3],
}

impl TransformSE3 {
    pub fn identity() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        }
    }

    pub fn from_translation(x: f64, y: f64, z: f64) -> Self {
        let mut t = Self::identity();
        t.translation = [x, y, z];
        t
    }

    /// Composes `self` followed by `next`: $T = T_{\text{next}} \circ T_{\text{self}}$.
    pub fn compose(&self, next: &TransformSE3) -> Self {
        let mut rot = [[0.0; 3]; 3];
        for (i, row) in rot.iter_mut().enumerate() {
            for (j, value) in row.iter_mut().enumerate() {
                *value = next.rotation[i][0] * self.rotation[0][j]
                    + next.rotation[i][1] * self.rotation[1][j]
                    + next.rotation[i][2] * self.rotation[2][j];
            }
        }

        let trans_rot = [
            next.rotation[0][0] * self.translation[0]
                + next.rotation[0][1] * self.translation[1]
                + next.rotation[0][2] * self.translation[2],
            next.rotation[1][0] * self.translation[0]
                + next.rotation[1][1] * self.translation[1]
                + next.rotation[1][2] * self.translation[2],
            next.rotation[2][0] * self.translation[0]
                + next.rotation[2][1] * self.translation[1]
                + next.rotation[2][2] * self.translation[2],
        ];

        let translation = [
            next.translation[0] + trans_rot[0],
            next.translation[1] + trans_rot[1],
            next.translation[2] + trans_rot[2],
        ];

        Self {
            translation,
            rotation: rot,
        }
    }

    pub fn transform_point(&self, p: [f64; 3]) -> [f64; 3] {
        [
            self.translation[0]
                + self.rotation[0][0] * p[0]
                + self.rotation[0][1] * p[1]
                + self.rotation[0][2] * p[2],
            self.translation[1]
                + self.rotation[1][0] * p[0]
                + self.rotation[1][1] * p[1]
                + self.rotation[1][2] * p[2],
            self.translation[2]
                + self.rotation[2][0] * p[0]
                + self.rotation[2][1] * p[1]
                + self.rotation[2][2] * p[2],
        ]
    }
}

/// Node in the coordinate frame tree.
#[derive(Debug, Clone)]
struct FrameNode {
    parent: Option<String>,
    pose_from_parent: TransformSE3,
}

/// A graph of named coordinate frames.
#[derive(Debug, Clone, Default)]
pub struct FrameGraph {
    nodes: HashMap<String, FrameNode>,
}

impl FrameGraph {
    pub fn new() -> Self {
        let mut graph = Self::default();
        graph.nodes.insert(
            "world".to_string(),
            FrameNode {
                parent: None,
                pose_from_parent: TransformSE3::identity(),
            },
        );
        graph
    }

    pub fn add_frame(
        &mut self,
        name: &str,
        parent: &str,
        pose_from_parent: TransformSE3,
    ) -> Result<(), FrameError> {
        if name == "world" {
            return Err(FrameError {
                message: "The root frame 'world' cannot be redefined".to_string(),
            });
        }
        if !self.nodes.contains_key(parent) {
            return Err(FrameError {
                message: format!("Parent frame '{parent}' does not exist in graph"),
            });
        }
        if self.nodes.contains_key(name) {
            return Err(FrameError {
                message: format!("Frame '{name}' already exists in graph"),
            });
        }
        self.nodes.insert(
            name.to_string(),
            FrameNode {
                parent: Some(parent.to_string()),
                pose_from_parent,
            },
        );
        Ok(())
    }

    /// Resolves the absolute pose of `frame_name` relative to root `"world"`.
    pub fn resolve_to_world(&self, frame_name: &str) -> Result<TransformSE3, FrameError> {
        let mut current = frame_name;
        let mut chain = Vec::new();
        let mut visited = HashSet::new();

        while let Some(node) = self.nodes.get(current) {
            if !visited.insert(current) {
                return Err(FrameError {
                    message: format!("Frame graph contains a cycle at '{current}'"),
                });
            }
            chain.push(&node.pose_from_parent);
            if let Some(ref parent) = node.parent {
                current = parent.as_str();
            } else {
                break;
            }
        }

        if current != "world" {
            return Err(FrameError {
                message: format!("Frame '{frame_name}' is disconnected from root 'world'"),
            });
        }

        let mut transform = TransformSE3::identity();
        for pose in chain.into_iter().rev() {
            transform = transform.compose(pose);
        }

        Ok(transform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_graph_resolves_nested_translations() {
        let mut graph = FrameGraph::new();
        graph
            .add_frame(
                "machine",
                "world",
                TransformSE3::from_translation(10.0, 0.0, 0.0),
            )
            .unwrap();
        graph
            .add_frame(
                "workpiece",
                "machine",
                TransformSE3::from_translation(0.0, 20.0, 5.0),
            )
            .unwrap();

        let pose = graph.resolve_to_world("workpiece").unwrap();
        assert_eq!(pose.translation, [10.0, 20.0, 5.0]);

        let p_world = pose.transform_point([1.0, 1.0, 1.0]);
        assert_eq!(p_world, [11.0, 21.0, 6.0]);
    }

    #[test]
    fn frame_graph_rejects_redefinitions_that_could_create_cycles() {
        let mut graph = FrameGraph::new();
        graph
            .add_frame(
                "machine",
                "world",
                TransformSE3::from_translation(10.0, 0.0, 0.0),
            )
            .unwrap();
        graph
            .add_frame(
                "workpiece",
                "machine",
                TransformSE3::from_translation(0.0, 20.0, 0.0),
            )
            .unwrap();

        let error = graph
            .add_frame("machine", "workpiece", TransformSE3::identity())
            .unwrap_err();
        assert!(error.message.contains("already exists"));

        let error = graph
            .add_frame("world", "machine", TransformSE3::identity())
            .unwrap_err();
        assert!(error.message.contains("cannot be redefined"));
    }
}
