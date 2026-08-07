# Universal Dry IR v0 Migration & Authoring Guide

**Version:** 1.0.0  
**Status:** User Guide  
**Document ID:** `DRY-GUIDE-2026-V30`

---

## Overview

Dry IR v0 is the normative, machine-independent universal intermediate representation for automated manufacturing toolpath compilation.

This guide provides practical code migration patterns for authoring toolpaths using native Dry SDKs in **Python**, **TypeScript/WASM**, and **Rust**.

---

## 1. Python SDK Authoring (`dry.Design`)

### Basic 3-Axis Extrusion
```python
import dry

design = (
    dry.Design()
    .geometry(width=0.6, height=0.2)
    .extruder(on=True)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
    .point(10, 10, 0.2)
    .point(0, 10, 0.2)
    .point(0, 0, 0.2)
)

# Resolve motion and emit G-code
gcode_lines = design.gcode(printer="generic")
metrics = design.simulate()
print(f"Total print duration: {metrics['total_time_s']:.2f}s")
```

### 5-Axis Non-Planar Toolpath with Power Channel
```python
import dry

design = (
    dry.Design()
    .power(800.0)  # Laser S-word / Spindle power
    .orient(0.7071, 0.0, 0.7071)  # 5-axis toolframe orientation vector
    .point(0, 0, 0.5)
    .point(20, 0, 0.5)
    .point(20, 20, 0.5)
    .power(0.0)  # Off
)

# Emit RS-274 CNC or GRBL Laser G-code
cnc_code = design.gcode(printer="rs274")
```

---

## 2. TypeScript / WASM SDK Authoring (`@dry/sdk`)

```typescript
import { Design, resolveGcode, resolveMetrics } from '@dry/sdk';

const design = new Design()
  .geometry(0.6, 0.2)
  .extruder(true)
  .point(0, 0, 0.2)
  .point(10, 0, 0.2)
  .point(10, 10, 0.2)
  .point(0, 10, 0.2)
  .point(0, 0, 0.2);

const opsJson = JSON.stringify(design.toOps());
const gcode = resolveGcode(opsJson, '{}', true, false, false, 'ab');
console.log(`Emitted ${gcode.length} lines of G-code`);
```

---

## 3. Native Rust Engine (`dry-core`)

```rust
use dry_core::sdk::DesignBuilder;
use dry_core::resolve::resolve;
use dry_core::engine::simulate;
use dry_core::emit::{emit, EmitParams, FirmwareFlavor};

fn main() {
    let builder = DesignBuilder::new()
        .geometry(0.6, 0.2)
        .extruder(true)
        .point(0.0, 0.0, 0.2)
        .point(10.0, 0.0, 0.2)
        .point(10.0, 10.0, 0.2)
        .point(0.0, 10.0, 0.2)
        .point(0.0, 0.0, 0.2);

    let ops = builder.build();
    let toolpath = resolve(&ops, &Default::default()).expect("resolve succeeds");
    let metrics = simulate(&toolpath);
    println!("Print time: {:.2}s", metrics.total_time_s);

    let params = EmitParams {
        flavor: FirmwareFlavor::Marlin,
        ..Default::default()
    };
    let gcode_lines = emit(&toolpath, &params).expect("emit succeeds");
}
```
