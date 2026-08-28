# Chapter 3: Multi-Axis CAM & Subtractive Manufacturing

## 1. 5-Axis Non-Planar Toolpaths & Surface Draping

Traditional 3D printing operates in planar $2.5\text{D}$ slices ($+Z$ normal). Dry treats the toolframe orientation as a first-class property of every segment:

```python
# Command a 5-axis toolhead orientation vector (i, j, k)
design.orient(i=0.7071, j=0.0, k=0.7071).point(x=50.0, y=50.0, z=15.0)
```

### Kinematics Solvers
Dry provides analytic forward and inverse kinematics for standard machine topologies:
* **AB Head-Head**: $B = \text{atan2}(i, k)$, $A = \text{atan2}(j, \sqrt{i^2 + k^2})$
* **BC Table-Table**: Reference five-axis model with singular cone hold at pole $k = \pm 1$.

```python
# Emit 5-axis G-code
gcode_lines = design.gcode(five_axis=True, rotary_axes="bc")
```

---

## 2. 2.5D Subtractive CNC Milling

Dry supports contour-parallel rectangular and circular pocketing with automated stepover calculation, multi-pass depth slicing, safe clearance planes, and separate plunge/cutting feedrates:

```python
design = dry.Design()
design.pocket({
    "shape": "rect",
    "x": 10.0, "y": 10.0,
    "width": 80.0, "height": 50.0,
    "toolDiameter": 6.0,
    "depth": 8.0,
    "depthPerPass": 2.0,
    "stepover": 0.45,  # 45% of tool diameter
    "cutFeed": 1500.0,
    "plungeFeed": 400.0,
    "safeZ": 5.0,
})
```

Output formats:
* **RS-274 / LinuxCNC**: Standard industrial G-code (`G21 G17 G90 G54 T1 M6 S12000 M3 ... M5 M30`).
* **ISO 14649 STEP-NC**: Semantic XML sidecars preserving geometry and process intent.

---

## 3. Industrial Robotics (KUKA KRL)

Dry generates compliant KUKA Robot Language (KRL) modules:
* `DEF / END` program structure.
* Modal `$VEL.CP` velocity control in $\text{m/s}$.
* Standard `{E6POS: X ..., Y ..., Z ..., A ..., B ..., C ...}` coordinates with ZYX-Euler tool orientations.
* Verified with independent ANTLR grammar parsers.
