# Star-Polygon Lattice Code Generator

This records how `/Users/dmytro/Downloads/lattice-research.pdf` was turned into Dry code. The source
paper is Soyarslan, Gleadall, Yan, Argeso and Sozumert, "Elastostatics of star-polygon tile-based
architectured planar lattices". The implementation is clean-room: it uses the paper's published geometry
and manufacturing facts, not the linked FullControl model source.

## What Is Implemented

The TypeScript SDK now has `starPolygonLattice(...)` and `starPolygonLatticeOps(...)` in
`sdk/ts/src/generators/starPolygonLattice.ts`. The browser gallery has the same generator shape in
`web/lattice-research.js`, with four visible examples under `Research lattices`.

The implemented families are:

| Family | Topology | Star sides | alphaSPL | alphaUL | Basis |
|---|---:|---:|---:|---:|---|
| `M1` | `4 . 4*alpha . 4**alpha` | 4 | 90 deg | 135 deg | triangular |
| `M2` | `3 . 6*alpha . 6**alpha` | 6 | 120 deg | 150 deg | triangular |
| `M3` | `6 . 3*alpha . 3**alpha` | 3 | 60 deg | 120 deg | triangular |
| `M4` | `3 . 3*alpha . 3 . 3**alpha` | 3 | 60 deg | 120 deg | square |

For `M1`..`M3`, the generator uses the paper's direct lattice vectors:

```text
a1 = LUC cos(pi/3) e1 + LUC sin(pi/3) e2
a2 = -LUC cos(pi/3) e1 + LUC sin(pi/3) e2
```

For `M4`, it uses:

```text
a1 = LUC e1
a2 = LUC e2
```

## Geometry Model

Each unit-cell motif is represented as a regular star-shaped `n`-gon with alternating star points and
dents. For star-point radius `R`, dent radius `r`, `phi = pi / n`, and star angle `alpha`, the generator
uses:

```text
r / R = tan(alpha / 2) / (sin(phi) + tan(alpha / 2) cos(phi))
```

This gives the expected limits:

- `alpha = 0`: dents collapse toward the center.
- `alpha = alphaSPL`: `r / R = cos(pi / n)`, the star-polygon limit.
- `alpha = alphaUL`: `r / R = 1`, the uniqueness-limit convex case.

The paper notes symmetry around `alphaUL`; the generator accepts `0..2*alphaUL` and mirrors values above
`alphaUL` back into the unique geometry range while flipping handedness.

## Toolpath Decisions

The paper states that the printed specimens used parametric FullControl procedures, continuous paths
where possible, odd width counts for complete periodic patterns, 0.5 mm extrusion width, 0.167 mm layer
height, three layers, 210 C nozzle temperature, and 1000 mm/min print speed. Those become the generator
defaults.

Dry-specific authoring decisions:

- Emit Dry L1 ops (`geometry`, `temperature`, `speed`, optional `flow`, `extruder`, `move`) rather than
  custom engine logic.
- Generate closed star-polygon loops for each unit-cell center and nearest-point struts between adjacent
  motifs.
- Use family-specific connector neighborhoods so `M1` is square-star dominated, `M2` is hex-star
  dominated, `M3` is tri-star on triangular basis, and `M4` is tri-star on square basis.
- Reorder paths with a nearest-neighbor heuristic and rotate closed loops to reduce non-printing travel.
- Reverse path order on alternating layers to keep the layer transition shorter.
- Force odd `cols` by default for `M2`, `M3` and `M4`, matching the paper's note about odd width counts.

## Known Limits

This is a practical code generator for Dry, not a finite-element reproduction of every selected primitive
unit cell in Figure 1.

- The paper's homogenization, relative density, stiffness and Poisson-ratio calculations are not
  implemented.
- The bespoke FullControl retraction/variable-speed strategies for odd junctions are not copied. Dry has
  speed and flow channels, so those can be added as clean-room heuristics later.
- The current inter-cell struts are generated from nearest star points; exact figure-level primitive
  cell coordinates would be a separate reverse-engineering pass.

## Next Work

- Add an Euler-trail path planner for each family so connected strut graphs can print with fewer travels.
- Add optional per-junction speed/flow/retraction strategy once Dry has a retraction/process dialect.
- Add alpha sweeps and a small property-estimation layer for relative density and expected auxetic
  regimes.
- Add Blockly-level parameter blocks for family, alpha, unit-cell count and process settings if the
  visual authoring page should expose this generator directly.
