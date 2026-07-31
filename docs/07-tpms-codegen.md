# TPMS Code Generator

Dry now includes a TPMS contour code generator in `sdk/ts/src/generators/tpms.ts` and browser examples
in `web/tpms-engine.js` (which delegates Op generation to the Rust engine over wasm). It is a toolpath
generator, not a mesh/STL exporter: each Z layer is sampled from an
implicit TPMS field, sliced with marching squares, stitched into polylines, and emitted as Dry L1 ops.

The equation set follows the common nodal approximations used by nTop's TPMS equation reference and the
MIT-licensed RegionTPMS notebooks. Ken Brakke's Surface Evolver TPMS catalog is useful for naming and
family orientation, especially for Schoen surfaces such as I-WP and F-RD.

Sources:

- `https://support.ntop.com/hc/en-us/articles/360053267814-What-equations-are-used-to-create-the-TPMS-types`
- `https://github.com/metudust/RegionTPMS`
- `https://kenbrakke.com/evolver/examples/periodic/periodic.html`

## Surfaces

Supported `surface` values:

| Key | Surface |
|---|---|
| `gyroid` | Gyroid |
| `schwarz-p` | Schwarz primitive |
| `schwarz-d` | Schwarz diamond |
| `iwp` | Schoen I-WP |
| `neovius` | Neovius |
| `fischer-koch-s` | Fischer-Koch S |
| `fischer-koch-y` | Fischer-Koch Y |
| `frd` | Schoen F-RD |
| `lidinoid` | Lidinoid |
| `split-p` | Split P |

## API

```ts
import { tpms, tpmsOps } from '@dry/sdk';

const d = tpms({
  surface: 'gyroid',
  isoLevel: 0,
  cellsX: 2,
  cellsY: 2,
  cellsZ: 2,
  cellSize: 12,
  samplesPerCell: 18,
  layerHeight: 0.28,
  beadWidth: 0.45,
  beadHeight: 0.28,
  perimeter: true,
  adaptive: true,
  adaptiveMaxLayerHeight: 0.28,
  maxFieldSamples: 6_000_000,
});

const ops = tpmsOps({ surface: 'frd', cellsX: 1, cellsY: 1, cellsZ: 1 });
```

## Decisions

- `cellSize` maps one TPMS period to `2*pi` in each axis.
- `cellsX/Y/Z` repeats the implicit field by period count.
- `isoLevel` selects `f(x,y,z)=isoLevel`; nonzero values create offset-like nodal variants, not exact
  constant-thickness shells.
- `samplesPerCell` controls XY contour resolution; higher values improve fidelity and increase op count.
- `layerHeight` controls Z sampling and resulting print height; the default is 0.28 mm and `beadHeight`
  defaults to the same value so previewed layers do not show artificial vertical gaps.
- **Every layer declares the bead height it occupies.** `resolve` reads `Op::Geometry`'s height as
  deposited volume, so a layer that does not span the nominal `layerHeight` — a layer inserted by
  `adaptive`, or the top layer clamped to the block height — emits a fresh `Op::Geometry` for the gap
  it really occupies. `beadHeight` is preserved as a *ratio* (`gap × beadHeight / layerHeight`), so a
  deliberate squish survives on every layer; a nominal-height layer declares exactly the configured
  `beadHeight`, and so does the first layer, which has nothing beneath it to measure against (`z0` is
  the nozzle Z, not a guaranteed plate gap). A top-layer remainder below 1% of `layerHeight` is a
  slicing artifact rather than a printable layer and is merged into the layer below, which keeps the
  block's full height without a second bead for it.
- `adaptive` inserts extra Z slices in intervals that are too tall or change contour topology/length
  sharply. `adaptiveMinLayerHeight`, `adaptiveMaxLayerHeight`, and the delta thresholds bound this pass.
- `maxFieldSamples` is a preflight budget for marching-squares work. It rejects runaway combinations of
  large `cellsX/Y/Z`, high `samplesPerCell`, small `layerHeight`, and adaptive slicing before generation
  starts; set it to `Infinity` only for trusted offline jobs.
- `perimeter` adds a rectangular single-wall loop on every layer so the generated contours can be
  previewed as bounded infill inside a printable volume.
- Path stitching is graph-based on quantized segment endpoints, followed by nearest-neighbor path ordering
  to reduce travels.

## Limits

- This is contour-sliced single-wall surface printing. It does not yet generate volumetric TPMS sheet
  thickness, solid/skeletal offsets, support strategy, or exact porosity targeting.
- Ambiguous marching-squares cells are resolved by the cell-center sign. This is deterministic but not a
  topology-preserving mesher.
- FFF printability still depends on nozzle, material, cooling and unsupported spans. Dry's verifier can
  catch machine/process contracts, but it does not yet predict surface self-support.
