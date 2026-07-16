# Conformance

Dry's correctness is **bootstrapped from the FullControl fork** and gated on reproducing it. This
directory will hold the five corpora (exported from the fork by a one-time script — task **P0.4**) and
the runner that diffs Dry's engine output against them (task **P0.5**). See `../docs/03-conformance.md`
for the strategy, the per-phase parity gates, and the float/determinism discipline.

```
conformance/
  golden/      full g-code + plot for representative designs (numbers normalised)   [from the fork]
  gcode/       per-design byte-identical Marlin/Klipper/Duet output                  [from the fork]
  gallery/     26 exported gallery fixtures + expected metrics/invariants/g-code     [from the fork]
  profiles/    ~695 device profiles: init data + start/end procedures                [from the fork]
  roundtrip/   emit(parse(g)) == g fixtures + simulate-metric parity                 [from the fork]
  runner/      diffs Dry output vs each corpus; native + wasm matrix                 [P0.5]
  export.*     the one-time fork → corpora export script                             [P0.4]
```

**The corpora are the oracle.** Once present, every implementation task is "make corpus N pass" with a
green diff as the definition of done. Nothing in the engine is considered correct until it matches the
fork on these fixtures (then, in later phases, Dry goes beyond what the fork can do — non-planar, 5-axis,
CNC/laser — where the fork is no longer an oracle and new fixtures are authored).
