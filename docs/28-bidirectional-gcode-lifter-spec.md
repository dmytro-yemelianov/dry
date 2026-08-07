# Bidirectional G-code Lifter & Macro Quarantine Specification ($L_3 \to L_2$)

**Version:** 1.0.0  
**Status:** Normative Standard  
**Document ID:** `DRY-SPEC-2026-V28`

---

## 1. Objective

The Bidirectional G-code Lifter (`crates/core/src/gcode/lift.rs`) parses target machine code ($L_3$) back into structured $L_2$ Dry IR motion segments (`Toolpath`).

This specification establishes the **Tri-Tier Lossiness Taxonomy** and the **Power Channel & Macro Quarantine Rules** for G-code lifting.

---

## 2. Tri-Tier Lossiness Taxonomy

When lifting target G-code back to Dry IR:
1. **Exact Recovered Semantics**:
   - Coordinates ($X, Y, Z, A, B, C$), arc centers ($I, J, K$), linear feedrate ($F$), spindle/laser power ($S$).
2. **Inferred Semantics**:
   - Extrusive vs travel classification (via $E$ delta), layer linkage (`trace.layers`), estimated deposited volume.
3. **Quarantined Opaque Nodes (`manual_gcode`)**:
   - Non-motion vendor M-codes ($M_{109}, M_{190}, G_{29}$, custom macro calls) are preserved verbatim inside `SegmentKind::ManualGcode` IR nodes, keeping the surrounding motion trajectory accurate without fabricating fake geometry.

---

## 3. Power Channel ($S$-Word) Lifting Rules

- **Spindle/Laser On ($M_3 / M_4$)**: Sets modal `power_active = true` and updates state with $S$ word value.
- **Spindle/Laser Off ($M_5$)**: Sets modal `power_active = false`. Subsequent motion segments carry `power: Some(0.0)`.
- **Power Value Propagation**: Active motion segments carry `power: Some(current_s)`.

---

## 4. 5-Axis Orientation Word Recovery

- For 5-axis programs carrying rotary words ($A, B, C$), `gcode/lift.rs` converts rotary positions back into TCP unit orientation vectors $\mathbf{u} = (i, j, k)$ using the inverse kinematic map:
  $$\mathbf{u} = R_{\text{rotary}}(A, B, C) \cdot \mathbf{e}_z$$
