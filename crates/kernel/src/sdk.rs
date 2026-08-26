//! Rust Authoring SDK for Dry designs.
//!
//! Provides a fluent `DesignBuilder` for programmatically constructing L1 [`Design`] instances with type safety and chaining.

use crate::resolve::{Design, Op};

/// Fluent builder for constructing Dry L1 [`Design`] operation sequences.
#[derive(Debug, Clone, Default)]
pub struct DesignBuilder {
    ops: Vec<Op>,
}

impl DesignBuilder {
    /// Creates a new empty [`DesignBuilder`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the extrusion bead geometry dimensions (width and height in mm).
    pub fn geometry(
        mut self,
        width: impl Into<Option<f64>>,
        height: impl Into<Option<f64>>,
    ) -> Self {
        self.ops.push(Op::Geometry {
            width: width.into(),
            height: height.into(),
        });
        self
    }

    /// Toggles the extruder state (on/off).
    pub fn extruder(mut self, on: bool) -> Self {
        self.ops.push(Op::Extruder { on });
        self
    }

    /// Sets the print feedrate in mm/min.
    pub fn speed(mut self, print: f64) -> Self {
        self.ops.push(Op::Speed { print });
        self
    }

    /// Sets the nozzle temperature channel (°C).
    pub fn temperature(mut self, nozzle: f64) -> Self {
        self.ops.push(Op::Temperature { nozzle });
        self
    }

    /// Sets the part-cooling fan speed ratio (0.0 to 1.0).
    pub fn fan(mut self, speed: f64) -> Self {
        self.ops.push(Op::Fan { speed });
        self
    }

    /// Sets the flow multiplier channel (default 1.0).
    pub fn flow(mut self, ratio: f64) -> Self {
        self.ops.push(Op::Flow { ratio });
        self
    }

    /// Sets the active tool index.
    pub fn tool(mut self, index: u32) -> Self {
        self.ops.push(Op::Tool { index });
        self
    }

    /// Sets the spindle/laser power channel (the target's `S` word value; `0.0` is commanded off).
    pub fn power(mut self, level: f64) -> Self {
        self.ops.push(Op::Power { level });
        self
    }

    /// Sets the toolframe orientation vector `[i, j, k]`.
    pub fn orient(mut self, i: f64, j: f64, k: f64) -> Self {
        self.ops.push(Op::Orient { i, j, k });
        self
    }

    /// Sets the toolframe orientation vector from a slice `[i, j, k]`.
    pub fn orient_vec(self, vector: [f64; 3]) -> Self {
        self.orient(vector[0], vector[1], vector[2])
    }

    /// Adds a pause / dwell for the specified seconds.
    pub fn dwell(mut self, seconds: f64) -> Self {
        self.ops.push(Op::Dwell { seconds });
        self
    }

    /// Moves to a specified location (axes set to `None` inherit current position).
    pub fn move_to(
        mut self,
        x: impl Into<Option<f64>>,
        y: impl Into<Option<f64>>,
        z: impl Into<Option<f64>>,
    ) -> Self {
        self.ops.push(Op::Move {
            x: x.into(),
            y: y.into(),
            z: z.into(),
        });
        self
    }

    /// Convenience method to move to explicit `(x, y, z)` coordinates in mm.
    pub fn move_xyz(self, x: f64, y: f64, z: f64) -> Self {
        self.move_to(Some(x), Some(y), Some(z))
    }

    /// Adds a circular arc move centered at `(cx, cy)`.
    pub fn arc_to(
        mut self,
        cx: f64,
        cy: f64,
        x: impl Into<Option<f64>>,
        y: impl Into<Option<f64>>,
        z: impl Into<Option<f64>>,
        clockwise: bool,
    ) -> Self {
        self.ops.push(Op::Arc {
            cx,
            cy,
            x: x.into(),
            y: y.into(),
            z: z.into(),
            clockwise,
        });
        self
    }

    /// Adds a Catmull-Rom spline with given control points.
    pub fn spline(mut self, points: Vec<[Option<f64>; 3]>) -> Self {
        self.ops.push(Op::Spline { points });
        self
    }

    /// Adds a clothoid (Euler-spiral) corner blend around the construction corner
    /// `(corner_x, corner_y)`, consuming `blend` mm of tangent length from each leg on the way to
    /// `(x, y, z)`.
    pub fn clothoid_to(
        mut self,
        corner_x: f64,
        corner_y: f64,
        x: impl Into<Option<f64>>,
        y: impl Into<Option<f64>>,
        z: impl Into<Option<f64>>,
        blend: f64,
    ) -> Self {
        self.ops.push(Op::Clothoid {
            corner_x,
            corner_y,
            x: x.into(),
            y: y.into(),
            z: z.into(),
            blend,
        });
        self
    }

    /// Injects verbatim custom G-code.
    pub fn manual_gcode(mut self, text: impl Into<String>) -> Self {
        self.ops.push(Op::ManualGcode { text: text.into() });
        self
    }

    /// Adds an explicit retraction.
    pub fn retract(
        mut self,
        distance: impl Into<Option<f64>>,
        speed: impl Into<Option<f64>>,
    ) -> Self {
        self.ops.push(Op::Retract {
            distance: distance.into(),
            speed: speed.into(),
        });
        self
    }

    /// Adds an explicit unretraction/prime.
    pub fn unretract(
        mut self,
        distance: impl Into<Option<f64>>,
        speed: impl Into<Option<f64>>,
    ) -> Self {
        self.ops.push(Op::Unretract {
            distance: distance.into(),
            speed: speed.into(),
        });
        self
    }

    /// Adds stationary extrusion of a specified volume in mm³.
    pub fn deposit(mut self, volume: f64, speed: f64) -> Self {
        self.ops.push(Op::Deposit { volume, speed });
        self
    }

    /// Appends a raw [`Op`] directly to the design.
    pub fn op(mut self, op: Op) -> Self {
        self.ops.push(op);
        self
    }

    /// Consumes the builder and returns the completed [`Design`].
    pub fn build(self) -> Design {
        Design { ops: self.ops }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_constructs_expected_design() {
        let design = DesignBuilder::new()
            .geometry(0.4, 0.2)
            .extruder(true)
            .speed(1800.0)
            .temperature(215.0)
            .fan(0.8)
            .move_xyz(10.0, 20.0, 0.2)
            .dwell(1.5)
            .build();

        assert_eq!(design.ops.len(), 7);
        match &design.ops[0] {
            Op::Geometry { width, height } => {
                assert_eq!(*width, Some(0.4));
                assert_eq!(*height, Some(0.2));
            }
            _ => panic!("expected Geometry op"),
        }
    }
}
