# Support matrix

What Dry supports today and at what level. This is the "what you can rely on" companion to
[`14-known-limitations.md`](14-known-limitations.md) (the caveats) and
[`09-customer-readiness.md`](09-customer-readiness.md) (the segment fit).

**Levels:** **Supported** = covered by conformance gates / tests and intended for use · **Experimental** =
present and usable, but not gated for production · **Out of scope** = not provided.

## Firmware / dialects (g-code emission & import)

| Flavor | Emit | Import / review / trace / rewrite | Level |
|---|---|---|---|
| Marlin | ✅ | ✅ | Supported |
| Klipper | ✅ | ✅ | Supported |
| Duet | ✅ | ✅ | Supported |
| Other firmware | falls back to Marlin-style | best-effort | Experimental |

## File formats

| Format | Read | Write | Level |
|---|---|---|---|
| Dry IR JSON | ✅ | ✅ | Supported (spec: [`10`](10-dry-ir-v0-spec.md)) |
| `DRY0` (columnar binary) | ✅ | ✅ | Supported |
| `DRY1` (chunked streaming binary) | ✅ | ✅ | Supported (the bounded-memory path) |
| Slicer g-code | ✅ (import) | ✅ (emit / rewrite) | Supported (Marlin/Klipper/Duet) |
| Profiles (v1 JSON) | ✅ | — | Supported (spec: [`11`](11-profiles-and-reports.md)) |
| 3MF / STEP-NC / mesh | — | — | Out of scope |

## Targets

| Target | Level |
|---|---|
| FFF g-code (3-axis) | Supported |
| FFF 5-axis / non-planar (rotary emit, toolframe) | Experimental (no kinematics/collision validation) |
| CNC / laser / robot | Out of scope (architecture anticipates them; no dialects yet) |

## Platforms (release artifacts)

| Artifact | Platforms | Level |
|---|---|---|
| CLI binaries | linux x86_64, macOS aarch64 + x86_64, Windows x86_64 | Supported |
| Python wheels | manylinux x86_64, macOS arm64, Windows amd64 (cp39-abi3); sdist elsewhere | Supported |
| npm package (`@dry/sdk`) | any Node 18+ (wasm engine) | Supported |
| Browser / wasm demo | modern browsers | Experimental |

See [`12-releasing.md`](12-releasing.md) for install-without-source instructions.

## Workflows

| Workflow | Level | Guide |
|---|---|---|
| Authoring: generate → verify → emit (Python/TS) | Supported | [`pilots/authoring.md`](pilots/authoring.md) |
| Post-slicer: review → trace → rewrite (CLI) | Supported | [`pilots/post-slicer-review.md`](pilots/post-slicer-review.md) |
| SDK integration: reproduce a vector | Supported | [`pilots/sdk-integration.md`](pilots/sdk-integration.md) |
| Optimize (merge collinear / arc-fit / travel-reorder) | Supported | [`15-cli-cookbook.md`](15-cli-cookbook.md) |
| G-code forensics: slicer detection, feature attribution, layer/estimate | Experimental (first cut) | [`15-cli-cookbook.md`](15-cli-cookbook.md) |
| Forensics: infill angle/spacing, extrusion-multiplier recovery, resonance | Out of scope (planned) | — |

## Support expectations

Pre-1.0: fixes target the latest release and `main`. Report bugs via GitHub issues; security/safety issues
privately ([`SECURITY.md`](../SECURITY.md)). There is no SLA — pilots should validate output on real
hardware (`verify` enforces only the documented rule catalog).
