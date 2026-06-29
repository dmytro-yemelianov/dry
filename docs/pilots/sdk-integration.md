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

## Reproduce a vector — Python

```python
import json, dry

ir = json.load(open("conformance/vectors/minimal_line/input.json"))
# round-trip the IR through your stack, then emit via Dry and compare to the golden:
got = dry.Design  # … your code produces/consumes IR; emit through the engine and diff vs expected.gcode
```

The Python/TS SDKs author *designs* (L1) that resolve to this same L2 IR, so an integrator typically:
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
# OK — 12 vectors validated independently (JSON + DRY0 + DRY1 …) with no dry-core.
```

## Acceptance

Your integration is wired in correctly when it can **read and write at least one vector** and reproduce
its expected output without depending on Dry's internals — exactly what `validate_vectors.py` demonstrates.
Version/compatibility rules for upgrading across releases are in [`10-dry-ir-v0-spec.md`](../10-dry-ir-v0-spec.md) §7–8.
