# Chapter 2: Algorithmic Authoring & Computational Geometry

## 1. Fluent Design Construction (Python & TypeScript)

Dry provides identical, high-ergonomics fluent builders across both Python and TypeScript:

### Python Example
```python
import dry

design = (
    dry.Design()
    .geometry(width=0.6, height=0.2)
    .temperature(215)
    .extruder(True)
    .speed(dry.mm_s(60))
    .point(10, 10, 0.2)
    .arc(cx=20, cy=10, x=30, y=10, clockwise=True)
    .clothoid(corner_x=40, corner_y=10, blend=5.0, x=40, y=30)
)
```

### TypeScript Example
```typescript
import { Design, mm, mm_s } from '@dry/sdk';

const design = new Design()
  .geometry(0.6, 0.2)
  .temperature(215)
  .extruder(true)
  .speed(mm_s(60))
  .point(10, 10, 0.2)
  .arc({ cx: 20, cy: 10, x: 30, y: 10, clockwise: true })
  .clothoid({ corner_x: 40, corner_y: 10, blend: 5.0, x: 40, y: 30 });
```

---

## 2. Advanced Geometric Generators

### Continuous Z Spiral Vases
In vase mode, Z increases continuously per move rather than stepping at discrete layer boundaries. This eliminates the "Z-seam" defect and provides constant extruder backpressure.

### Triply Periodic Minimal Surfaces (TPMS)
TPMS are non-self-intersecting implicit surfaces with zero mean curvature. Dry generates continuous toolpaths directly from the analytical field equation:

$$\cos(x)\sin(y) + \cos(y)\sin(z) + \cos(z)\sin(x) = c$$

Supported families: `gyroid`, `schwarz-p`, `schwarz-d`, `iwp`, `neovius`, `lidinoid`, `split-p`.

```python
gcode = dry.tpms_gcode({
    "surface": "gyroid",
    "cellSize": 15.0,
    "cellsX": 3, "cellsY": 3, "cellsZ": 3,
    "wallThickness": 0.45,
})
```

### Auxetic Star-Polygon Lattices
Metamaterials exhibiting negative Poisson's ratio ($\nu < 0$) authored as continuous single-stroke extrusion passes, maximizing mechanical strength while minimizing retractions.
