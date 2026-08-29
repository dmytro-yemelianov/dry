# Star-Polygon Lattice Code Generator

This records how the star-polygon lattice research literature and the public
`star_polygon_lattice_colab.py` notebook path recipe were turned into Dry code. The source paper is
Soyarslan, Gleadall, Yan, Argeso and Sozumert, "Elastostatics of star-polygon tile-based architectured
planar lattices". The current generator is a Dry implementation of the notebook's print-walk semantics,
not the earlier motif approximation.

## What Is Implemented

The TypeScript SDK has `starPolygonLattice(...)` and `starPolygonLatticeOps(...)` in
`sdk/ts/src/generators/starPolygonLattice.ts`. The browser gallery has the same generator shape in
`web/lattice-research.js`, with four visible examples under `Research lattices`.

The implemented families are:

| Family | Topology | Star sides | alphaSPL | alphaUL | Basis |
|---|---:|---:|---:|---:|---|
| `M1` | `4 . 4*alpha . 4**alpha` | 4 | 90 deg | 135 deg | triangular |
| `M2` | `3 . 6*alpha . 6**alpha` | 6 | 120 deg | 150 deg | triangular |
| `M3` | `6 . 3*alpha . 3**alpha` | 3 | 60 deg | 120 deg | triangular |
| `M4` | `3 . 3*alpha . 3 . 3**alpha` | 3 | 60 deg | 120 deg | square |

The browser controls map directly to the notebook defaults: `segLength = 4.33`, `units_x = 10`,
`units_y = 3`, `EW = 0.5`, `EH = 0.2`, `layers = 2`, `start_x = 30`, and `start_y = 30`.

## Geometry Helpers

The SDK still exposes the paper metadata and dent-radius helper for analysis and future property
estimation. For star-point radius `R`, dent radius `r`, `phi = pi / n`, and star angle `alpha`, the
helper uses:

```text
r / R = tan(alpha / 2) / (sin(phi) + tan(alpha / 2) cos(phi))
```

This gives the expected limits:

- `alpha = 0`: dents collapse toward the center.
- `alpha = alphaSPL`: `r / R = cos(pi / n)`, the star-polygon limit.
- `alpha = alphaUL`: `r / R = 1`, the uniqueness-limit convex case.

The paper notes symmetry around `alphaUL`; `normalizeStarPolygonAlpha(...)` accepts `0..2*alphaUL` and
mirrors values above `alphaUL` back into the unique geometry range. The print-path generator itself uses
the raw notebook-style `alphaDeg` value because mirroring would change the emitted path relative to the
original Colab recipe.

## Toolpath Decisions

The notebook uses family-specific polar strut walks, explicit `Extruder(on=False/True)` transitions,
row copies, layer copies, and a start offset. Those semantics are now reflected directly in Dry L1 ops.

Dry-specific authoring decisions:

- Emit Dry L1 ops (`geometry`, `temperature`, `speed`, optional `flow`, `extruder`, `move`) rather than
  custom engine logic.
- Build `M1`..`M4` from the notebook's polar step formulas instead of regular star loops.
- Preserve notebook travel transitions as Dry `extruder` ops; travel points still inherit missing axes
  just like FullControl partial points.
- Preserve the notebook's three printed return lines by default so layer transitions match the original
  steady-state assumption.
- Copy layers by `layerHeight` and then apply the notebook's `0.8 * EH` first-layer Z offset.
- For `M4`, round odd `rows` up to an even width by default, matching the notebook's row-pair strategy.

## Known Limits

This is a print-path generator, not a finite-element reproduction of every selected primitive unit cell
in Figure 1.

- The paper's homogenization, relative density, stiffness and Poisson-ratio calculations are not
  implemented.
- FullControl printer initialization, primer lines, bed temperature and fan settings are not authored as
  Dry ops yet. The Dry path starts with a travel move to the first lattice point, then enables extrusion.
- Plot annotations from the notebook are intentionally omitted from Dry L1.
- Reverse-engineering relative density, property curves and alpha sweeps remains separate from path
  compatibility.

## Next Work

- Add optional bed/fan/primer process ops once Dry has those channels.
- Add optional per-junction speed/flow/retraction strategy once Dry has a retraction/process dialect.
- Add alpha sweeps and a small property-estimation layer for relative density and expected auxetic
  regimes.
- Add Blockly-level parameter blocks for family, alpha, strut length, unit-cell count and process settings if the
  visual authoring page should expose this generator directly.
