# Pilot guide: authoring (generate → verify → emit)

**For:** a researcher or advanced maker generating custom toolpaths in Python or TypeScript.
**Goal:** go from an authored design to validated g-code in a few minutes.
**You'll use:** the Dry SDK (`generate`), `verify` (safety), and `gcode` (`emit`).

The runnable scripts are [`examples/authoring.py`](../../examples/authoring.py) and
[`examples/authoring.ts`](../../examples/authoring.ts).

## Python

Setup (once, in a virtualenv):

```sh
cd py && maturin develop
```

Author a design — a line, a quarter arc (`G3`) about the origin, then a line — then verify and emit:

```python
import dry

design = (dry.Design()
          .geometry(width=0.6, height=0.2)      # bead width/height
          .extruder(on=True)
          .point(10, 0, 0.2)                     # start
          .arc(cx=0, cy=0, x=0, y=10)            # quarter arc -> (0, 10)
          .point(0, 20, 0.2))                    # finish with a line

# verify BEFORE emitting — bounds is a CSV string "x0,x1,y0,y1,z0,z1" (mm)
report = design.verify(bounds="0,200,0,200,0,200")
assert not [f for f in report["findings"] if f["severity"] == "error"]

print(design.simulate())                         # metrics dict
print("\n".join(design.gcode()))                 # motion g-code
```

Running `python examples/authoring.py` prints:

```
verify: 0 finding(s), 0 error(s)
simulate: 2 segments, 1.54s, 1.283mm filament
g-code:
G1 F1000 X10 Y0 Z0.2 E0
G3 X0 Y10 I-10 J0 E0.783673
G1 Y20 E0.498902
```

## TypeScript

```sh
cd sdk/ts && npm ci && npm run build
```

The same design, mirrored ([`examples/authoring.ts`](../../examples/authoring.ts)):

```ts
import { Design } from '@dry/sdk';

const design = new Design()
  .geometry(0.6, 0.2)
  .extruder(true)
  .point(10, 0, 0.2)
  .arc({ cx: 0, cy: 0, x: 0, y: 10 })
  .point(0, 20, 0.2);

const report = design.verify('generic', 0, 0, '0,200,0,200,0,200'); // positional args; bounds is CSV
if (report.findings.some((f) => f.severity === 'error')) throw new Error('failed verification');

console.log(design.simulate());
console.log(design.gcode().join('\n'));
```

## What to try next

- Add a `verify` limit that fails (e.g. `bounds="0,5,0,5,0,5"`) and watch the `bounds` finding appear.
- Inspect the IR with `design.ir()` (Python) / `design.ir()` (TS), or write `DRY1` with `design.binary()`.
- Validate output on a real machine before printing — Dry's `verify` enforces only the documented rule
  catalog ([`11-profiles-and-reports.md`](../11-profiles-and-reports.md)), not machine-specific safety.

> Known edges (see [`14-known-limitations.md`](../14-known-limitations.md)): `verify`/`gcode` take some
> limits as **comma-strings** today, and `verify` reflects exactly the rule catalog — nothing more.
