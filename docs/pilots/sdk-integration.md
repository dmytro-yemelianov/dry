# Pilot guide: SDK integration (embed Dry, reproduce a vector)

**For:** an integrator embedding Dry in their own stack (Python, TypeScript, or the CLI as a subprocess).
**Goal:** call Dry from your code and prove you get the **same deterministic output** as the published
conformance vectors — the acceptance bar for "Dry is wired in correctly".
**You'll use:** the public IR contract ([`10-dry-ir-v0-spec.md`](../10-dry-ir-v0-spec.md)) and the public
vectors under [`conformance/vectors/`](../../conformance/vectors).

## The contract

Dry IR v0 is a published contract: a JSON wire form plus the `DRY0`/`DRY1` binary encodings, with a
[JSON Schema](../../spec/dry-ir-v0.schema.json) and a curated [vector set](../../conformance/vectors).
Conformance is **semantic** (exact `f64` equality, structural equality), not cross-language byte-identity
(§9 of the spec). Each vector ships its input IR plus expected `metrics` and `g-code`.

## Reproduce a vector — CLI

The fastest smoke test: emit a vector's IR and confirm it matches the published golden g-code byte-for-byte.

```sh
dry emit conformance/vectors/minimal_line/input.json
# G1 F1500 X10 Y0 Z0.2 E0.3326

diff <(dry emit conformance/vectors/minimal_line/input.json) \
     conformance/vectors/minimal_line/expected.gcode && echo "MATCH"
# MATCH
```

If your integration shells out to `dry`, this is your end-to-end check per vector.

## Gate an authored design — Python

The published vectors start at the resolved L2 IR, while the Python SDK currently starts at the L1
`Design` API. Use the CLI recipe above when you need to ingest an existing L2 vector byte-for-byte. For a
Python SDK integration, commit the expected output for the design you author and compare it directly:

```python
import dry

design = (
    dry.Design()
    .geometry(width=0.4, height=0.2)
    .speed(1500)
    .extruder(on=True)
    .point(0, 0, 0.2)
    .point(10, 0, 0.2)
)

expected = [
    "G1 F1500 X0 Y0 Z0.2 E0",
    "G1 X10 E0.332601",
]
got = design.gcode()
assert got == expected, f"g-code drifted:\nexpected={expected!r}\ngot={got!r}"
print("MATCH")
```

The Python/TS SDKs author *designs* (L1) that resolve through the same engine to L2 IR, so an integrator typically:
1. authors or ingests a design,
2. resolves to IR (`design.ir()`),
3. emits (`design.gcode()`),
4. checks the IR/g-code against a committed vector for the path they care about.

## Validate the contract independently (no `dry-core`)

The repo ships a second, dependency-free implementation of the codec that re-decodes/encodes every vector
and validates it against the schema — proof the spec is self-sufficient, and a template for your own
reader/writer:

```sh
pip install -r tools/requirements.txt
python tools/validate_vectors.py conformance/vectors
# OK — 13 vectors validated independently (JSON + DRY0 + DRY1 …) with no dry-core.
```

## Acceptance

Your raw-IR integration is wired in correctly when it can **read and write at least one vector** and reproduce
its expected output without depending on Dry's internals — exactly what `validate_vectors.py` demonstrates.
An authoring-SDK integration is wired in correctly when a representative L1 design reproduces its committed
G-code golden output, as in the Python check above.
Version/compatibility rules for upgrading across releases are in [`10-dry-ir-v0-spec.md`](../10-dry-ir-v0-spec.md) §7–8.
