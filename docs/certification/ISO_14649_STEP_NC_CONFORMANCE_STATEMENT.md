# ISO 14649 (STEP-NC AP 238) Compliance & Interoperability Statement

**Document Reference**: `DRY-CONF-STEPNC-14649-001`  
**Governing Standards**: ISO 14649-10/11, ISO 10303-238:2020 (STEP-NC AP 238)  
**Schema Namespace**: `urn:iso:std:iso-10303-14649`  
**Engine Release**: `Dry v0.7.0`  

---

## 1. Executive Summary

`Dry` supports machine-independent CNC toolpath and manufacturing intent exchange via ISO 14649 (STEP-NC). By raising the semantic level above controller-specific G-code dials (G1/G2/G3), STEP-NC enables deterministic toolpath transmission across 3-axis, 4-axis, and 5-axis CNC machining centers.

---

## 2. Workingstep & Technology Mapping

| STEP-NC Construction | Schema Element | Dry Representation |
|---|---|---|
| **Program Header** | `<header schema="ISO-10303-238:2020" .../>` | Compiler intent metadata |
| **Workpiece** | `<workpiece unit="mm"/>` | Dimensional frame definition |
| **Milling Workingstep** | `<workingstep type="motion">` | `SegmentKind::Line` / `SegmentKind::Arc` |
| **Rapid Positioning** | `<workingstep type="rapid">` | Travel motion (`Segment.travel = true`) |
| **Tool Orientation (5-Axis)** | `<toolframe i="..." j="..." k="..."/>` | Unit tool orientation vector |
| **Circular Interpolation** | `<arc centre_x="..." centre_y="..."/>` | Planar arc center coordinates |
| **Dwell Workingstep** | `<workingstep type="pause" duration_s="..."/>` | `SegmentKind::Dwell` |

---

## 3. Conformance Validation

Verified by `crates/core/tests/step_nc_conformance.rs` and `crates/core/tests/step_nc_import.rs`.
All emitted XML documents validate against standard XML schema parsers without requiring proprietary post-processors.
