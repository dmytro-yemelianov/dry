# Dry

**Toolpath compiler infrastructure** — a typed, units-aware, multi-level intermediate representation
(the **Dry IR**) and a Rust engine for *algorithmic* machine toolpaths, with thin authoring front-ends
in several languages. Think **LLVM/MLIR for machine motion**.

A design is a *program* that produces motion + process intent. Dry lowers that intent through a typed
IR — design → path → motion → target — and `simulate`s, `verify`s, `optimise`s, `emit`s and `parse`s it,
the way a compiler lowers a program. The IR is the product; authoring **languages** (Python / TypeScript
/ Rust) and target **machines** (FFF g-code, CNC, laser, robot) are interchangeable front-ends and
back-ends.

> **Status: planning / foundations (Phase 0).** This repository starts from the specification in
> [`docs/`](docs/). Nothing is built yet — see the roadmap and the task backlog.

## Why

No released standard represents *algorithmic, arc-native, non-planar, variable-width* toolpaths with
provenance, invariants and a multi-language story (the survey: `docs/` references the FullControl fork's
prior-art study). g-code is the lossy target; slicer IRs are planar/polyline/internal; STEP-NC is
subtractive; 3MF Toolpath is unreleased and linear-only. Dry fills that gap and interoperates with the
rest.

## Architecture (one screen)

```
 authoring SDKs            the engine (Rust → native + wasm)           targets
 Python │ TS │ Rust  ─►  L0 design → L1 path → L2 motion → L3 target ─► FFF g-code · CNC · laser · robot
   (emit Dry IR)           lower · simulate · verify · optimise · emit · parse · reverse        · 3MF
```

- **Multi-level IR** with progressive lowering (MLIR-style dialects).
- **Toolframe** (position + orientation) so **non-planar and 5-axis are native**, not bolted on.
- **Units are types** (Length/Speed/Volume/Flow/Temperature) — mixed units are a compile error.
- **Pure functional core**: `design(params) → IR`; deposition/state is an explicit pass, not authoring
  state. Deterministic, content-addressable, streamable (columnar / Arrow-style) to millions of moves.
- **Provenance + invariants are first-class** — designs declare contracts the compiler enforces.

Full detail in [`docs/01-architecture.md`](docs/01-architecture.md).

## Documentation

| | |
|---|---|
| [`docs/00-vision-and-scope.md`](docs/00-vision-and-scope.md) | thesis, scope, success criteria, the honest risk |
| [`docs/01-architecture.md`](docs/01-architecture.md) | the IR dialects, toolframe, units/channels, passes, engine, SDKs, targets, reuse map |
| [`docs/02-roadmap.md`](docs/02-roadmap.md) | phases P0–P6 with exit gates; risk register; critical path |
| [`docs/03-conformance.md`](docs/03-conformance.md) | bootstrapping correctness from the FullControl fork; the parity gates |
| [`docs/04-tasks.md`](docs/04-tasks.md) | the sized, dependency-ordered backlog + the immediate next 5 |

## Bootstrapped from FullControl

Dry is a clean-slate re-layering, but not a blank page: it bootstraps from the **FullControl fork**
(`dmytro-yemelianov/fullcontrol`) — its Rust kernel, hardened IR, optimisation passes, ~695 device
profiles, ~906 tests and 27-design gallery become Dry's reference implementation seeds and **conformance
corpora** (`docs/03-conformance.md`). Every phase is gated on reproducing the fork's output.

## Licence

**Apache-2.0** (see [`LICENSE`](LICENSE) / [`NOTICE`](NOTICE)). Dry is an **independent, clean-room**
implementation: FullControl (GPLv3) is used only as design *inspiration* and a dev/CI *behavioural
oracle* — never copied into Dry's source, never shipped or linked into a release. That separation is
what makes the permissive licence available and lets the **Dry IR be an open standard**. See
[`docs/CLEANROOM.md`](docs/CLEANROOM.md). *(Not legal advice — confirm with counsel before a public
release.)*

## Quickstart

CLI (over a Dry IR file):
```
cargo run -p dry-cli --bin dry -- emit conformance/gcode/square.json   # motion g-code
```

Python — author a design, the Rust engine resolves + emits it:
```python
import dry
d = (dry.Design()
     .geometry(width=0.6, height=0.2).extruder(on=True)
     .point(10, 0, 0.2).arc(cx=0, cy=0, x=0, y=10).point(0, 20, 0.2))   # a line, a G3 arc, a line
print("\n".join(d.gcode()))   # -> G1 ... / G3 X0 Y10 I-10 J0 E0.783673 / G1 ...
print(d.simulate())           # -> {time, distances, material, peak flow, ...}
```
Build the module: `cd py && maturin develop` (in a venv).

Browser / wasm — the *same* Rust engine, compiled to wasm, resolving a design client-side:
```bash
bash web/build.sh        # -> web/pkg/ (wasm + JS glue)
python3 -m http.server   # then open http://localhost:8000/web/  (design picker + canvas + g-code + metrics)
```
The wasm output is byte-identical to the CLI and the Python SDK — proven in CI, where Node runs the
wasm engine against the conformance oracle (`web/smoke.cjs`).

## Repository layout

```
docs/            the specification, roadmap, conformance plan, task backlog
crates/
  core/          the dependency-light Dry IR + engine (no PyO3/numpy)  [done: ir/resolve/simulate/emit]
  cli/           the `dry` command (inspect/simulate/emit; +optimise)  [done: inspect/simulate/emit]
  wasm/          the wasm-bindgen binding                              [done]
web/             the browser demo (build.sh, index.html, node smoke)   [done]
py/              the PyO3 binding + Python authoring SDK (`dry`)        [done]
sdk/
  ts/            the TypeScript authoring SDK                          [P4.1]
conformance/     corpora generated by the FullControl oracle + tests   [done: simulate + gcode]
```
