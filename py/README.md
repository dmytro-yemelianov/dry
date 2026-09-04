# DryMachina Python SDK (`dry`)

[![PyPI](https://img.shields.io/badge/python-3.9+-blue.svg)](https://www.python.org/)
[![License: BUSL-1.1](https://img.shields.io/badge/License-BUSL--1.1-blue.svg)](../LICENSE)
[![Engine: Rust](https://img.shields.io/badge/engine-Rust-orange.svg)](../crates/core)

Python authoring SDK and PyO3 native extension for **DryMachina** — the typed, deterministic toolpath compiler for additive, subtractive (CNC), and multi-axis robotics motion.

---

## 1. Quickstart

### Installation
```bash
# In your virtual environment:
pip install maturin
maturin develop -m py/Cargo.toml
```

### Basic Authoring & Simulation
```python
import dry

# Build a parametric toolpath with arc-native moves
design = (
    dry.Design()
    .geometry(width=0.6, height=0.2)
    .extruder(True)
    .point(0, 0, 0.2)
    .point(50, 0, 0.2)
    .arc(cx=50, cy=25, x=50, y=50) # G3 arc
    .point(0, 50, 0.2)
)

# 1. Simulate cycle metrics
metrics = design.simulate()
print(f"Time: {metrics['total_time_s']:.1f}s, Segments: {metrics['segment_count']}")

# 2. Verify against safety contracts
report = design.verify(bounds=[[0, 200], [0, 200], [0, 200]], max_feedrate=18000)
print(f"Findings: {len(report.findings)}")

# 3. Emit G-code (supports Marlin, Klipper, GRBL, RS274, KRL, etc.)
gcode_lines = design.gcode(flavor="klipper")
print("\n".join(gcode_lines[:5]))
```

---

## 2. Advanced CAM Features

### CNC Pocket Milling & S-Curves
```python
# Create rectangular pocket with helical ramp descent
pocket_ops = dry.pocket_ops(
    width=60.0,
    height=40.0,
    depth=5.0,
    tool_diameter=6.0,
    stepover_percent=45.0,
    depth_per_pass=2.5,
)
```

### Parametric CNC Lathe Turning & Facing
```python
facing_ops = dry.lathe_facing_ops(
    stock_diameter=50.0,
    start_z=2.0,
    target_z=0.0,
    step_depth=1.0,
    feedrate=150.0,
    spindle_rpm=1200.0,
)
```

### 5-Axis Multi-Axis Toolframe Drape
```python
# Direct STEP B-Rep solid slicing with exact surface normals
ops = dry.slice_step_solid(
    step_data=open("model.step", "rb").read(),
    layer_height=0.4,
    z_min=0.0,
    z_max=25.0,
)
```

---

## 3. Testing

```bash
pytest py/tests/ -v
```

---

## License

Licensed under the **Business Source License 1.1 (BUSL-1.1)**. The DryMachina v0.10.0
terms convert to MIT on 2030-09-05; see [LICENSE](../LICENSE) and [NOTICE](../NOTICE).
