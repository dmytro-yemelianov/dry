# ISO/ASTM 52915 3MF Toolpath Extension Conformance Statement

**Document Reference**: `DRY-CONF-3MF-52915-001`  
**Standard**: ISO/ASTM 52915 (3MF Core & 3MF Toolpath Extension Specification 2022/07)  
**Conformance Level**: Level 1 Full Interoperability (Lossless Round-Trip & Multi-Axis Intent)  
**Engine Release**: `Dry v0.7.0`  

---

## 1. Conformance Overview

The `Dry` CAM compiler provides full bi-directional serialization and deserialization for the 3MF Toolpath Extension format under the official 3MF Consortium namespace:

```xml
xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"
xmlns:tp="http://schemas.microsoft.com/3dmanufacturing/toolpath/2022/07"
```

---

## 2. Element & Attribute Support Matrix

| 3MF Toolpath Element / Attribute | XML Representation | Dry IR Mapping | Support Status |
|---|---|---|---|
| **Linear Motion** | `<tp:segment type="line" ...>` | `SegmentKind::Line` | **100% Lossless** |
| **Circular Arc** | `<tp:segment type="arc" cx="..." cy="..." cw="...">` | `SegmentKind::Arc` | **100% Lossless** |
| **Dwell / Pause** | `<tp:segment type="dwell" dwell="...">` | `SegmentKind::Dwell` | **100% Lossless** |
| **Travel Rapid** | `<tp:segment travel="true" ...>` | `Segment.travel = true` | **100% Lossless** |
| **Feedrate** | `feedrate="<f64>"` | `Segment.speed: Feedrate` | **100% Lossless** |
| **Bead Cross-Section** | `width="<f64>" height="<f64>"` | `Segment.width`, `Segment.height` | **100% Lossless** |
| **Extruded Volume** | `volume="<f64>"` | `Segment.volume: Volume` | **100% Lossless** |
| **Thermal Channel** | `temp="<f64>"` | `Segment.temperature: Option<f64>` | **100% Lossless** |
| **Fan Channel** | `fan="<f64>"` | `Segment.fan: Option<f64>` | **100% Lossless** |
| **5-Axis Toolframe** | `i="<f64>" j="<f64>" k="<f64>"` | `Segment.orientation: Option<[f64; 3]>` | **100% Lossless** |

---

## 3. Automated Verification

Conformance is continuously checked via automated test suite `crates/core/tests/threemf_conformance.rs`, verifying:
1. Exact golden byte-for-byte property preservation across `export_3mf_xml` $\to$ `import_3mf_xml`.
2. Rejection of malformed documents containing `NaN`, `inf`, or negative feedrates.
3. Total element preservation with zero attribute dropping.
