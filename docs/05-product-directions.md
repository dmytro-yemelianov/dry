# Product directions and expanded uses

This note preserves the product discussion around Dry's next possible shapes and widens it into a
more complete opportunity map. The order follows the original discussion:

1. slicer app vs CAD/add-on/workbench,
2. post-slicer review and Klipper optimization,
3. reverse engineering and inference from existing G-code,
4. time-series analysis with LLM-assisted explanation.

The central positioning: Dry should not begin as a conventional slicer clone. Its stronger role is a
manufacturing compiler: it accepts authored geometry or machine motion, lifts it into typed IR, runs
analysis and verification, then emits or rewrites target machine programs.

## 1. Slicer app, CAD add-on, and broader uses

### Recommendation

Build Dry first as a CAD-connected toolpath workbench and programmable manufacturing layer. A full
mesh slicer can become one frontend later, but it is not the best first wedge.

Dry is already suited to:

- authored paths,
- typed IR,
- deterministic lowering,
- simulation,
- verification,
- optimization passes,
- g-code emit,
- Python/TS/wasm bindings,
- web preview,
- streaming binary archives.

A conventional slicer additionally needs:

- robust mesh import and repair,
- planar slicing,
- polygon offsetting,
- support generation,
- infill generation,
- seam planning,
- overhang and bridge detection,
- printer/filament/material profile management,
- model placement and object UI,
- cooling and time heuristics,
- multi-object scheduling,
- 3MF project import/export,
- a large compatibility matrix.

That surface is large enough to hide Dry's differentiator. The differentiator is not "another slicer";
it is the typed, inspectable, programmable manufacturing compiler underneath slicing.

### Best first products

#### CAD-connected toolpath workbench

The strongest fit is a CAD/plugin workflow:

```text
CAD feature / sketch / curve / surface
  -> Dry L1 path dialect
  -> Dry L2 motion IR
  -> simulate / verify / optimize
  -> target g-code or robot code
```

Useful host targets:

- Fusion 360: approachable scripting/add-in ecosystem and good early prototyping target.
- Onshape: strong cloud/parametric workflow, good for FeatureScript-style generation.
- SolidWorks: valuable industrially, but heavier Windows/add-in surface.
- Rhino/Grasshopper: very natural for procedural geometry and custom toolpaths.
- Blender: useful for artistic/robotic deposition and mesh-to-path experiments.
- FreeCAD: open source and practical for early plugin development.

#### Programmable slicer

This is not "upload STL and slice everything" first. It is a path-first environment:

```text
define exact paths, channels, non-planar moves, tool orientation, constraints
  -> compile
  -> preview
  -> verify
  -> emit
```

This fits:

- vase modes,
- custom infill,
- lattices,
- gradient material/process experiments,
- non-planar FFF,
- 4/5-axis printing,
- toolpath research,
- educational slicing/toolpath demos.

#### Hybrid slicer later

Once the compiler layer is valuable, mesh slicing can be integrated by using existing geometry and
polygon libraries for conventional layers. Dry can own:

- IR,
- profiles,
- verification,
- optimization,
- non-planar passes,
- channel scheduling,
- target emit.

### Obvious uses

- Parametric g-code generation.
- Non-planar toolpath authoring.
- 5-axis or rotary FFF experiments.
- Custom infill and special vase modes.
- Toolpath preview and simulation.
- Machine-safety verification.
- CLI/SDK backend for slicers, CAD tools and web apps.
- Educational toolpath and slicing workbench.

### Less obvious uses

- Robotic extrusion: clay, concrete, food, silicone, composites.
- Multi-material and gradient deposition.
- Toolpath QA for generated files.
- Manufacturing lint in CI.
- Agent/generative-design compiler target: generate Dry IR instead of raw g-code.
- G-code normalization and rewrite layer.
- Dry IR as an interchange format between CAD, slicers, robots and machines.
- Research platform for experimental deposition strategies.
- Browser-based print-debugging viewer.
- Calibration data analysis and profile tuning.
- Fleet policy checks before jobs are accepted by machines.

## 2. Post-slicer review and Klipper optimization

This is one of the most practical near-term uses. Dry can sit between existing slicers and the printer
instead of replacing either.

```text
PrusaSlicer / OrcaSlicer / Cura / SuperSlicer
  -> generated .gcode
  -> Dry review/import
  -> report, reject, annotate, or rewrite
  -> Klipper / Moonraker / printer
```

### Review-only mode

This should come first because it builds trust without changing machine behavior.

Dry can detect:

- max volumetric flow violations,
- feedrates above a printer/material profile,
- tiny segment density that overloads firmware planning,
- travel moves with extrusion,
- cold extrusion or temperature mismatch,
- high-speed moves in small-feature regions,
- bridge/overhang risk,
- cooling-limited regions,
- excessive fan/temp state churn,
- discontinuities in extrusion,
- layer-time violations,
- out-of-bounds moves,
- suspicious retract/prime behavior,
- non-monotonic Z where a process expects monotonic layers.

Possible command:

```bash
dry review part.gcode --profile voron24-abs.json
```

Possible upload-hook path:

```text
Moonraker upload
  -> Dry review
  -> accept / warn / reject
  -> optional annotated report
  -> print
```

### Rewrite and optimization mode

Rewriting is more dangerous and should be profile-driven. Dry could adjust:

- feedrates by region,
- volumetric-flow-limited speeds,
- travel speeds,
- `M204` acceleration values,
- Klipper `SET_VELOCITY_LIMIT`,
- fan and temperature schedules,
- redundant state changes,
- tiny collinear segment runs,
- safe arc/spline simplification,
- path ordering in regions where semantics are known,
- comments/metadata for later trace analysis.

Optimization modes should be explicit:

- `safe`: no behavioral rewrite except redundant state cleanup and reporting.
- `balanced`: speed up only where constraints are clearly under limit.
- `max`: use calibrated machine envelopes and rewrite aggressively, still gated by verification.

Potential commands:

```bash
dry review part.gcode --profile voron24-abs.json
dry optimize-gcode part.gcode --profile voron24-abs.json --mode safe -o part.safe.gcode
dry optimize-gcode part.gcode --profile voron24-abs.json --mode max -o part.max.gcode
```

### What "push printing to max" requires

Dry needs a machine model, not only a parser:

- motion limits: velocity, acceleration, square-corner velocity, jerk-like limits,
- axis-specific acceleration and resonance constraints,
- input shaper frequencies and damping,
- hotend max volumetric flow per material/temperature,
- nozzle diameter and line-width constraints,
- cooling capacity and minimum layer-time policy,
- bed adhesion and first-layer limits,
- pressure advance and extrusion dynamics,
- retraction/deretraction behavior,
- fan/temperature response latency,
- object/material/profile metadata,
- empirical calibration results.

Dry should keep the deterministic verifier as the authority. Optimizers propose changes; verification
decides whether the result is within the declared profile.

## 3. Reverse engineering and inference from G-code

Dry can become a G-code decompiler and slicer-behavior analyzer. This should be framed as inference,
not perfect reconstruction: G-code loses CAD intent, source feature names, and many slicer decisions.

```text
G-code parser
  -> motion stream
  -> Dry IR
  -> geometric segmentation
  -> feature classification
  -> rule inference
  -> review / explain / optimize / rewrite
```

### What can be inferred

- layer height,
- likely line width,
- extrusion multiplier,
- volumetric flow profile,
- perimeters vs infill vs top/bottom/bridge/support,
- seam positions and seam strategy,
- infill angle, spacing, density and pattern periodicity,
- speed rules by feature type,
- acceleration and jerk/square-corner assumptions when encoded,
- cooling and temperature policy,
- retraction/deretraction policy,
- travel optimization patterns,
- tiny-segment hotspots,
- conservative and aggressive print zones.

Slicer comments help a lot. Without comments, classification becomes probabilistic.

### Signal and geometry methods

Useful methods:

- Path segmentation by extrusion state, Z, comments, speed and continuity.
- Curvature and corner-angle analysis.
- Segment-length and segment-duration histograms.
- Volumetric flow `dV/dt`.
- Changeloint detection for speed/flow/fan/temp policies.
- Rasterizing layers for image-space analysis.
- Hough/Radon transforms for infill angle and spacing.
- 2D FFT for repeated layer patterns and infill periodicity.
- STFT/wavelets for local, non-stationary behavior.
- Treating `x + iy` as a complex path signal for frequency analysis.
- Clustering windows by geometry, flow and speed features.
- Comparing multiple G-code files from varied slicer settings to infer rule deltas.

Fourier analysis is useful, but not sufficient by itself. It should be one tool inside a hybrid
geometric/statistical analyzer.

### Manufacturing forensics commands

```bash
dry decompile part.gcode -o inferred.dry.json
dry explain part.gcode --profile voron24.json
dry fingerprint part.gcode
dry compare stock.gcode tuned.gcode
dry infer-rules part.gcode --with-comments
```

Example findings:

- "Outer walls are capped near 90 mm/s; infill reaches 280 mm/s."
- "Layer 42 is flow-limited around 24.8 mm3/s."
- "The slicer reduces speed on small perimeters below 18 mm radius."
- "This region emits 8,400 segments/minute and may be planner-heavy."
- "Dominant Y-axis excitation appears around 43 Hz."
- "Bridge regions use higher fan and lower flow than adjacent infill."

### Limits and care

- Exact CAD intent is mostly unrecoverable.
- Firmware-side behavior may not be encoded in G-code.
- Feature classification without comments is probabilistic.
- Proprietary slicer behavior should be inferred for interoperability and review, not cloned as a
  private implementation.
- Rewrites must preserve semantic boundaries such as pauses, tool changes, filament changes and custom
  macros.

## 4. Time-series analysis with LLM assistance

LLMs should be used around time-series analysis, not as the numerical engine.

Correct architecture:

```text
G-code
  -> deterministic parser
  -> motion/time series
  -> signal/statistical analysis
  -> compact feature summaries
  -> LLM explains, compares, hypothesizes, recommends
  -> deterministic verifier gates any rewrite
```

Do not feed raw million-point traces to an LLM and ask it to analyze them. Generate structured signals
and summaries first:

- `x(t), y(t), z(t), e(t)`,
- speed,
- acceleration,
- jerk estimate,
- volumetric flow,
- segment duration,
- segment length,
- segment rate,
- curvature,
- extrusion on/off state,
- fan/temp/channel states,
- layer index,
- feature classification,
- frequency summaries,
- anomaly windows,
- changepoints,
- risk scores.

The LLM is useful for:

- explaining why a print is slow,
- translating analysis into profile changes,
- identifying likely slicer rules,
- comparing two G-code files,
- generating a human-readable risk report,
- suggesting calibration experiments,
- grouping patterns into semantic regions,
- proposing optimization policies for deterministic verification.

Example feature summary passed to an LLM:

```json
{
  "window": "layer_38_120s_145s",
  "feature": "infill",
  "max_flow_mm3_s": 21.5,
  "mean_speed_mm_s": 242.0,
  "segment_rate_hz": 140.0,
  "dominant_freq_hz": [43.2, 86.1],
  "risk": ["planner_load", "flow_limit"]
}
```

Example report output:

```text
Layer 38:
- Infill runs at 280 mm/s but is flow-limited to about 21.5 mm3/s.
- Outer walls cap near 90 mm/s.
- Small arcs produce high segment density in a 40-second window.
- Recommended safe action: merge collinear micro-segments and cap infill to 240 mm/s unless hotend
  calibration confirms more than 24 mm3/s.
```

Potential commands:

```bash
dry trace-gcode part.gcode --window-s 5 > trace.json
dry analyze part.gcode --profile voron24.json
dry explain part.gcode --llm
dry compare stock.gcode tuned.gcode
dry optimize-gcode part.gcode --mode safe
```

The boundary is important: deterministic code owns math and safety; the LLM explains, prioritizes and
proposes. Dry's verifier decides whether a rewrite is valid.

## Missing areas to add before these products

### G-code import

This is the most important missing component for post-slicer work.

Required:

- streaming parser for `G0/G1/G2/G3/G4`,
- modal state reconstruction,
- absolute/relative XYZ and E,
- feedrate state,
- units,
- arc center modes,
- extrusion mode,
- comments and slicer metadata,
- common M-codes,
- Klipper macros and extension points,
- pause/tool-change/filament-change barriers,
- unknown-command preservation.

The importer should produce a lossless-enough motion stream with attached raw lines and modal context,
not only simplified geometry.

### Machine/profile schema

Optimization needs explicit profiles:

- printer kinematics,
- firmware flavor,
- axis limits,
- extruder limits,
- hotend flow curves,
- material settings,
- cooling model,
- pressure advance,
- input shaper data,
- macro semantics,
- safety policies.

Profiles should be versioned and serializable so reports and rewrites are reproducible.

Current implementation starts with schema version 1 in `dry-core`:

- `firmware.flavor` for dialect context (`klipper`, `marlin`, `duet`, etc.),
- `machine.build_volume` and `machine.feedrate_range`,
- `material.filament_diameter`, `material.max_volumetric_flow_mm3_s`, and
  `material.min_nozzle_temperature_c`,
- `process.line_width`, `process.layer_height`, and `process.monotonic_z`.

`dry review-gcode --profile profile.json` uses those fields both to lift slicer G-code into Dry IR and
to build verifier contracts. Explicit CLI flags remain overrides, so a one-off review can tighten or
relax a profile limit without editing the profile file.

### Trace storage

Time-series and comparison workflows need an efficient trace format:

- current JSON summaries from `dry trace-gcode` with fixed window IDs, print/travel/dwell time,
  material/distance totals, peak flow/feedrate and source G-code line ranges,
- Parquet or Arrow for offline analysis,
- JSON summaries for UI and LLM context,
- stable window IDs,
- links back to source G-code line ranges,
- optional screenshots or layer rasters.

### Optimization safety gates

Before automatic rewrites:

- round-trip parser/serializer tests,
- no-loss preservation of unknown lines,
- semantic barriers around macros/tool changes/pauses,
- before/after simulation comparison,
- profile-bound verification,
- visual diff,
- conservative default mode.

### UI surfaces

Useful UIs:

- web report for one G-code file,
- layer timeline with risk bands,
- speed/flow/frequency plots,
- g-code line inspector,
- before/after compare,
- upload hook result page,
- CAD plugin panel,
- profile editor.

### Data and calibration

"Max" printing requires empirical data:

- hotend flow tests,
- pressure advance tests,
- resonance/input-shaper data,
- cooling bridge/overhang tests,
- first-layer adhesion limits,
- material-specific envelopes,
- failed-print annotations.

Dry should treat calibration results as profile inputs, not hard-coded guesses.

## Suggested sequencing

1. G-code parser with lossless modal state and source-line mapping.
2. Review-only reports for existing slicer output.
3. Machine/profile schema and basic Klipper/Moonraker upload hook.
4. Trace export and compare tooling.
5. Conservative rewrite mode for redundant states and safe feed caps.
6. Slicer-behavior inference and feature classification.
7. LLM-assisted explanations from structured summaries.
8. CAD/plugin workbench for authored paths.
9. Aggressive optimization modes backed by calibration data.
10. Hybrid mesh slicing as a later frontend, not the core identity.
