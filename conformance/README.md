# Conformance

Dry's correctness is **bootstrapped from the FullControl fork** and gated on reproducing it. This
directory will hold the five corpora (exported from the fork by a one-time script — task **P0.4**) and
the runner that diffs Dry's engine output against them (task **P0.5**). See `../docs/03-conformance.md`
for the strategy, the per-phase parity gates, and the float/determinism discipline.

```
conformance/
  golden/      full g-code + plot for representative designs (numbers normalised)   [from the fork]
  gcode/       per-design byte-identical Marlin/Klipper/Duet output                  [from the fork]
  gallery/     28 fixtures: 27 registry designs + Overhang Plus metrics/g-code       [from the fork]
  profiles/    ~695 device profiles: init data + start/end procedures                [from the fork]
  roundtrip/   emit(parse(g)) == g fixtures + simulate-metric parity                 [from the fork]
  vectors/     Dry IR v0 vectors: JSON + DRY0/DRY1 + metrics + g-code + design       [NOT from the fork]
  runner/      diffs Dry output vs each corpus; native + wasm matrix                 [P0.5]
  export.*     the one-time fork → corpora export script                             [P0.4]
```

**The corpora are the oracle.** Once present, every implementation task is "make corpus N pass" with a
green diff as the definition of done. Nothing in the engine is considered correct until it matches the
fork on these fixtures (then, in later phases, Dry goes beyond what the fork can do — non-planar, 5-axis,
CNC/laser — where the fork is no longer an oracle and new fixtures are authored).

**`vectors/` is the exception, and the home of everything the fork cannot judge.** Its seeds are
hand-authored in typed Rust (`crates/core/tests/spec_vectors.rs`, regenerated with `UPDATE_VECTORS=1`)
and its committed artifacts are this engine's own output. **No fork oracle backs any of them**, but
they are not all equally unbacked, and the difference matters:

| Artifact | What backs it |
|---|---|
| `input.json`, `expected.dry0`, `expected.dry1` | `tools/validate_vectors.py` — a second, `dry-core`-free codec written from `docs/10` + `spec/dry-ir-v0.schema.json` with nothing but the Python stdlib. It re-decodes and re-encodes all 13 vectors and compares semantically, so a wrong encoding is caught by something that never ran Dry's codec. |
| `expected.gcode`, `metrics.json`, `design.json` | Nothing outside Dry. The validator only sha256s them against `MANIFEST.json`, which is Dry's own hash of Dry's own output — a drift gate, not an oracle. |

Having no fork oracle is not a weakness of the corpus: this is the only honest place for a 5-axis or
non-planar fixture, because FullControl is 3-axis and a fixture placed in `gallery/` would inherit an
oracle's authority by association. Each vector's `description` states what does and does not back it;
`five_axis_drape` (the oriented design) is the worked example.

`design.json` is the L1 op list plus the `ResolveParams` a vector's IR was resolved under, present
only on vectors authored at the design tier (today: `five_axis_drape`). It exists so a binding that
builds outside this workspace can drive the same design and diff its own g-code and metrics against
the committed ones — `web/smoke.cjs` does. It is **not** part of the IR v0 codec contract: it has no
schema in `spec/`, and a format-conformant reader is not required to consume it (spec §9).
