//! 3D Dexel Grid Stock Subtraction & Volumetric CAM Simulation (Track E2).
//!
//! Models raw workpiece stock as a 2D regular array of vertical depth elements (Dexels).
//! Sweeps tool geometries along cutting toolpath segments to simulate subtractive machining,
//! tracking remaining stock volume, removed chip volume, and surface scallop distribution.

use crate::ir::Toolpath;
use serde::{Deserialize, Serialize};

/// A 3D Dexel (Depth-Element) stock workpiece model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DexelGrid {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
    pub resolution_mm: f64,
    pub nx: usize,
    pub ny: usize,
    /// Current top Z coordinate for each grid cell `[ix * ny + iy]`.
    pub heights: Vec<f64>,
    /// Base Z coordinate for each grid cell.
    pub base_z: f64,
}

/// Volumetric and surface quality simulation summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DexelSimulationReport {
    pub initial_volume_mm3: f64,
    pub remaining_volume_mm3: f64,
    pub removed_volume_mm3: f64,
    pub material_removal_ratio: f64,
    pub min_height_mm: f64,
    pub max_height_mm: f64,
}

impl DexelGrid {
    /// Creates a new rectangular stock workpiece.
    pub fn new_stock(
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
        resolution_mm: f64,
    ) -> Result<Self, String> {
        if max_x <= min_x || max_y <= min_y || max_z <= min_z {
            return Err("Invalid stock bounding box dimensions".into());
        }
        if resolution_mm <= 0.0 || !resolution_mm.is_finite() {
            return Err("Resolution must be positive and finite".into());
        }

        let nx = (((max_x - min_x) / resolution_mm).ceil() as usize).max(1);
        let ny = (((max_y - min_y) / resolution_mm).ceil() as usize).max(1);

        let total_cells = nx.checked_mul(ny).ok_or("Stock grid size overflow")?;
        let heights = vec![max_z; total_cells];

        Ok(Self {
            min_x,
            min_y,
            min_z,
            max_x,
            max_y,
            max_z,
            resolution_mm,
            nx,
            ny,
            heights,
            base_z: min_z,
        })
    }

    /// Cell center world X coordinate.
    #[inline]
    pub fn cell_x(&self, ix: usize) -> f64 {
        self.min_x + (ix as f64 + 0.5) * self.resolution_mm
    }

    /// Cell center world Y coordinate.
    #[inline]
    pub fn cell_y(&self, iy: usize) -> f64 {
        self.min_y + (iy as f64 + 0.5) * self.resolution_mm
    }

    /// Linear cell index.
    #[inline]
    fn cell_idx(&self, ix: usize, iy: usize) -> usize {
        ix * self.ny + iy
    }

    /// Carves a straight cutting motion between `(x0, y0, z0)` and `(x1, y1, z1)` with given tool radius.
    pub fn carve_segment(
        &mut self,
        p0: [f64; 3],
        p1: [f64; 3],
        tool_radius: f64,
        is_ballnose: bool,
    ) {
        if tool_radius <= 0.0 {
            return;
        }

        let seg_min_x = (p0[0].min(p1[0]) - tool_radius).max(self.min_x);
        let seg_max_x = (p0[0].max(p1[0]) + tool_radius).min(self.max_x);
        let seg_min_y = (p0[1].min(p1[1]) - tool_radius).max(self.min_y);
        let seg_max_y = (p0[1].max(p1[1]) + tool_radius).min(self.max_y);

        if seg_min_x >= seg_max_x || seg_min_y >= seg_max_y {
            return;
        }

        let ix_start =
            (((seg_min_x - self.min_x) / self.resolution_mm).floor() as usize).min(self.nx - 1);
        let ix_end =
            (((seg_max_x - self.min_x) / self.resolution_mm).ceil() as usize).min(self.nx - 1);
        let iy_start =
            (((seg_min_y - self.min_y) / self.resolution_mm).floor() as usize).min(self.ny - 1);
        let iy_end =
            (((seg_max_y - self.min_y) / self.resolution_mm).ceil() as usize).min(self.ny - 1);

        let vx = p1[0] - p0[0];
        let vy = p1[1] - p0[1];
        let vz = p1[2] - p0[2];
        let len_sq_2d = vx * vx + vy * vy;

        for ix in ix_start..=ix_end {
            let cx = self.cell_x(ix);
            for iy in iy_start..=iy_end {
                let cy = self.cell_y(iy);

                // Project (cx, cy) onto 2D line segment p0 -> p1
                let t = if len_sq_2d < 1e-12 {
                    0.0
                } else {
                    let dot = (cx - p0[0]) * vx + (cy - p0[1]) * vy;
                    (dot / len_sq_2d).clamp(0.0, 1.0)
                };

                let proj_x = p0[0] + t * vx;
                let proj_y = p0[1] + t * vy;
                let proj_z = p0[2] + t * vz;

                let dx = cx - proj_x;
                let dy = cy - proj_y;
                let dist_xy_sq = dx * dx + dy * dy;

                if dist_xy_sq <= tool_radius * tool_radius {
                    let tool_bottom_z = if is_ballnose {
                        let rad_offset = (tool_radius * tool_radius - dist_xy_sq).sqrt();
                        proj_z + (tool_radius - rad_offset)
                    } else {
                        proj_z
                    };

                    let idx = self.cell_idx(ix, iy);
                    if tool_bottom_z < self.heights[idx] {
                        self.heights[idx] = tool_bottom_z.max(self.base_z);
                    }
                }
            }
        }
    }

    /// Simulates a full toolpath against the stock workpiece.
    /// Carve every cutting segment of `toolpath` out of the stock.
    ///
    /// Refuses a `tool_radius` that is not finite and positive. Returning `Ok(())` after carving
    /// nothing would be indistinguishable from a toolpath that genuinely misses the stock: the report
    /// would read `removed_volume_mm3 = 0.0`, a finite and entirely plausible number, and a caller
    /// would reasonably conclude the program does not cut. `new_stock` already refuses a
    /// non-positive resolution for the same reason; this closes the other half.
    pub fn simulate_toolpath(
        &mut self,
        toolpath: &Toolpath,
        tool_radius: f64,
        is_ballnose: bool,
    ) -> Result<(), String> {
        if !(tool_radius.is_finite() && tool_radius > 0.0) {
            return Err(format!(
                "tool_radius must be finite and > 0, got {tool_radius}"
            ));
        }
        for seg in &toolpath.segments {
            if !seg.travel {
                let start_pt = [
                    seg.start[0].map(|l| l.value()).unwrap_or(0.0),
                    seg.start[1].map(|l| l.value()).unwrap_or(0.0),
                    seg.start[2].map(|l| l.value()).unwrap_or(0.0),
                ];
                let end_pt = [
                    seg.end[0].map(|l| l.value()).unwrap_or(0.0),
                    seg.end[1].map(|l| l.value()).unwrap_or(0.0),
                    seg.end[2].map(|l| l.value()).unwrap_or(0.0),
                ];
                self.carve_segment(start_pt, end_pt, tool_radius, is_ballnose);
            }
        }
        Ok(())
    }

    /// Calculates total initial stock volume (mm³).
    pub fn initial_volume(&self) -> f64 {
        (self.max_x - self.min_x) * (self.max_y - self.min_y) * (self.max_z - self.min_z)
    }

    /// Calculates remaining stock volume after machining (mm³).
    pub fn remaining_volume(&self) -> f64 {
        let cell_area = self.resolution_mm * self.resolution_mm;
        let mut sum_height = 0.0;
        for &h in &self.heights {
            sum_height += (h - self.base_z).max(0.0);
        }
        sum_height * cell_area
    }

    /// Calculates removed material volume (mm³).
    pub fn removed_volume(&self) -> f64 {
        (self.initial_volume() - self.remaining_volume()).max(0.0)
    }

    /// Generates a comprehensive volumetric simulation report.
    pub fn generate_report(&self) -> DexelSimulationReport {
        let init_vol = self.initial_volume();
        let rem_vol = self.remaining_volume();
        let rem_removed = (init_vol - rem_vol).max(0.0);
        let ratio = if init_vol > 0.0 {
            rem_removed / init_vol
        } else {
            0.0
        };

        let mut min_h = f64::INFINITY;
        let mut max_h = f64::NEG_INFINITY;
        for &h in &self.heights {
            if h < min_h {
                min_h = h;
            }
            if h > max_h {
                max_h = h;
            }
        }

        DexelSimulationReport {
            initial_volume_mm3: init_vol,
            remaining_volume_mm3: rem_vol,
            removed_volume_mm3: rem_removed,
            material_removal_ratio: ratio,
            min_height_mm: min_h,
            max_height_mm: max_h,
        }
    }
}
