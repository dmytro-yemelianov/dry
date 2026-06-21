# TPMS Code Generator

Dry now includes a TPMS contour code generator in `sdk/ts/src/generators/tpms.ts` and browser examples
in `web/tpms.js`. It is a toolpath generator, not a mesh/STL exporter: each Z layer is sampled from an
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
  layerHeight: 0.8,
  beadWidth: 0.45,
  beadHeight: 0.24,
});

const ops = tpmsOps({ surface: 'frd', cellsX: 1, cellsY: 1, cellsZ: 1 });
```

## Decisions

- `cellSize` maps one TPMS period to `2*pi` in each axis.
- `cellsX/Y/Z` repeats the implicit field by period count.
- `isoLevel` selects `f(x,y,z)=isoLevel`; nonzero values create offset-like nodal variants, not exact
  constant-thickness shells.
- `samplesPerCell` controls XY contour resolution; higher values improve fidelity and increase op count.
- `layerHeight` controls Z sampling and resulting print height.
- Path stitching is graph-based on quantized segment endpoints, followed by nearest-neighbor path ordering
  to reduce travels.

## Limits

- This is contour-sliced single-wall surface printing. It does not yet generate volumetric TPMS sheet
  thickness, solid/skeletal offsets, support strategy, or exact porosity targeting.
- Ambiguous marching-squares cells are resolved by the cell-center sign. This is deterministic but not a
  topology-preserving mesher.
- FFF printability still depends on nozzle, material, cooling and unsupported spans. Dry's verifier can
  catch machine/process contracts, but it does not yet predict surface self-support.
