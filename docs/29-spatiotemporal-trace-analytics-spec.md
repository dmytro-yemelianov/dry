# Spatiotemporal Trace Analytics & $K$-Sigma Outlier Windowing Specification

**Version:** 1.0.0  
**Status:** Normative Standard  
**Document ID:** `DRY-SPEC-2026-V29`

---

## 1. Objective

Spatiotemporal Trace Analytics (`crates/core/src/trace.rs`) processes an $L_2$ motion toolpath (`Toolpath`) to compute time-weighted metrics, flow rate distributions, layer partitions (`trace.layers`), and $K$-sigma flow outlier windowing.

This provides manufacturing telemetry and quality control data for fleet management, edge execution, and AI toolpath analysis.

---

## 2. Statistical Outlier Windowing ($K$-Sigma Detection)

For each extruding segment $i$, volumetric flow rate is computed as:
$$Q_i = \frac{V_i}{\Delta t_i} = \frac{\text{volume}}{\text{length} / \text{speed}}$$

### 2.1 Mean & Standard Deviation
$$\mu_Q = \frac{1}{\sum \Delta t_i} \sum_{i} Q_i \cdot \Delta t_i$$
$$\sigma_Q = \sqrt{\frac{1}{\sum \Delta t_i} \sum_{i} (Q_i - \mu_Q)^2 \cdot \Delta t_i}$$

### 2.2 $K$-Sigma Outlier Predicate
A segment is classified as a **Flow Outlier** if:
$$|Q_i - \mu_Q| > K \cdot \sigma_Q \quad (\text{default } K = 3.0)$$

Outliers are recorded in the `TraceReport` analytics block as `flow_outliers: Vec<OutlierSegment>`.

---

## 3. Layer Partitioning (`trace.layers`)

- Toolpaths are partitioned into layers keyed on extrusion $Z$ height.
- Monotonic $Z$ layers group sequential extruding moves at the same nominal height.
- Each layer records:
  - `layer_index`: 0-based layer sequence number.
  - `z_height_mm`: Nominal extrusion $Z$ height.
  - `extruded_volume_mm3`: Total deposited material in layer.
  - `print_time_s`: Duration of motion within layer.
  - `move_count`: Number of segments in layer.
