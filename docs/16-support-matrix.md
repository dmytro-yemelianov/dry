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

“Supported” import means the documented motion/state subset is modeled and conformance-tested.
Firmware commands outside that subset are preserved and surfaced as `unmodeled-gcode` warnings for
manual review; they are not silently treated as motion. This holds for vendor commands whose
parameters are not g-code words at all — barewords, quoted strings, base64 — so stock Bambu Lab and
Prusa start g-code imports (`docs/14-known-limitations.md` states what still fails loudly).

## File formats

| Format | Read | Write | Level |
|---|---|---|---|
| Dry IR JSON | ✅ | ✅ | Supported (spec: [`10`](10-dry-ir-v0-spec.md)) |
| `DRY0` (columnar binary) | ✅ | ✅ | Supported |
| `DRY1` (chunked streaming binary) | ✅ | ✅ | Supported (the bounded-memory path) |
| Slicer g-code | ✅ (import) | ✅ (emit / rewrite) | Supported (Marlin/Klipper/Duet) |
| Profiles (v1 JSON) | ✅ | — | Supported (spec: [`11`](11-profiles-and-reports.md)) |
| 3MF Toolpath (XML) | ✅ (import) | ✅ (export) | Experimental (core API only; documented lossiness) |
| STEP-NC (XML sidecar) | — | ✅ (`emit --step-nc`) | Experimental (schema-light; no XSD validation; motion transcription, not machining intent) |
| Mesh | — | — | Out of scope |

## Targets

| Target | Level |
|---|---|
| FFF g-code (3-axis) | Supported |
| FFF 5-axis / non-planar (rotary emit, toolframe) | Experimental (no kinematics/collision validation) |
| CNC RS-274 (`emit --format rs274`) | Experimental (rect/circle pocket+profile via `dry generate pocket`; RS-274 program frame from `machine.cnc`, which is the *only* way it commands the spindle — a per-segment `power` channel is refused, not merged; not validated against a physical controller) |
| GRBL (`emit --format grbl`) | Experimental (dialect scaffolding: word emission + Dry-parser round-trip only; `dry generate pocket` output emits as bare motion with no program frame. The one flavor that renders the per-segment spindle/laser `power` channel, as modal `S` with `M3`/`M5`; never validated against a real controller) |
| KRL robot (`emit --format robot-krl`) | Experimental (dialect scaffolding: word emission + Dry-parser round-trip only; no program frame, and no rendering for the spindle/laser `power` channel — a toolpath carrying it is refused; never validated against a real controller) |
| GRBL laser mode (`emit_grbl_laser`, core API) | Prototype (renders `M3`/`M4`/`M5` and scaled `S`; core API only — no CLI flag, no emit flavor, no conformance golden, never on a controller) |
| Plasma / waterjet (`emit_plasma_waterjet`, core API) | Prototype (pierce dwell, torch on/off, tangential lead-in/lead-out; core API only — no CLI flag, no emit flavor, no conformance golden, never on a controller) |
| RS-274 canned cycles (`DrillCycle`, `PeckDrillCycle`, core API) | Prototype (`G81`/`G83` blocks; core API only — not reachable from `emit --format rs274`, no conformance golden) |

## Platforms (release artifacts)

| Artifact | Platforms | Level |
|---|---|---|
| CLI binaries | linux x86_64, macOS aarch64 + x86_64, Windows x86_64 | Supported |
| Python wheels | manylinux x86_64, macOS arm64, Windows amd64 (cp39-abi3); sdist elsewhere | Supported |
| npm package (`@dry/sdk`) | any Node 18+ (wasm engine) | Supported |
| Browser / wasm demo | modern browsers | Experimental |

See [`12-releasing.md`](12-releasing.md) for install-without-source instructions.

## Commercial licensing

The software behavior below is separate from the BUSL-1.1 legal boundary. Evaluation mode is fully
functional for evaluation and for production use that qualifies for the Additional Use Grant; use
beyond one User, one Production Machine, or as a Competing Service requires a commercial license.
The binary does not meter that cap or phone home.

| Feature | Level |
|---|---|
| `dry license activate` / `dry license status`, offline Ed25519 token verification | Supported |
| Eval mode (no license / expired past grace: full functionality, stderr banner, report stamps) | Supported |
| `LicenseStamp` on report envelopes (`verify`, `review-gcode`, `compare`, `explain`, `rewrite-gcode`) | Supported (golden-stable: absent unless a license is active) |
| `dry upload` license gate (refuses without a Valid/Grace license, before any network call) | Supported |
| License-issuer Cloudflare Worker (Lemon Squeezy webhook → signed token → email + D1 audit log) | Experimental (test-mode only; no production key ceremony or live deploy yet) |

## Workflows

| Workflow | Level | Guide |
|---|---|---|
| Authoring: generate → verify → emit (Python/TS) | Supported | [`pilots/authoring.md`](pilots/authoring.md) |
| Post-slicer: review → trace → rewrite (CLI) | Supported | [`pilots/post-slicer-review.md`](pilots/post-slicer-review.md) |
| SDK integration: reproduce a vector | Supported | [`pilots/sdk-integration.md`](pilots/sdk-integration.md) |
| Optimize (merge collinear / arc-fit / travel-reorder) | Supported | [`15-cli-cookbook.md`](15-cli-cookbook.md) |
| G-code forensics: slicer detection, feature attribution, layer/estimate | Experimental (first cut) | [`15-cli-cookbook.md`](15-cli-cookbook.md) |
| Forensics: infill angle/spacing, extrusion-multiplier recovery, resonance | Out of scope (planned) | — |

<!-- docs-gen:start supported-profile-matrix -->
## Supported profile matrix

A small, curated set of **supported** machine/material/firmware profiles (authored clean-room — see the
provenance ledger, [`17`](17-provenance-and-licensing.md)) lives under
[`../conformance/profile-matrix/`](../conformance/profile-matrix). Each is schema-valid and drift-gated
through the review pipeline (`crates/core/tests/profile_matrix.rs`), with a committed golden `review.json`.

| Entry | Firmware | Material | Envelope (mm) | Min nozzle |
|---|---|---|---|---|
| `marlin-pla-i3` | Marlin | PLA | 220×220×250 | 190 °C |
| `marlin-petg-i3` | Marlin | PETG | 220×220×250 | 220 °C |
| `klipper-pla-corexy` | Klipper | PLA | 350×350×250 | 190 °C |
| `klipper-abs-corexy` | Klipper | ABS | 350×350×250 | 230 °C |
| `duet-petg-cartesian` | Duet | PETG | 300×300×300 | 220 °C |
| `duet-abs-corexy` | Duet | ABS | 256×256×256 | 230 °C |

The goldens double as a demonstration: reviewing the same 210 °C sample (`examples/part.gcode`) is clean
under the PLA profiles but correctly raises `cold-extrusion` under the PETG/ABS profiles — the
material-temperature contract at work. Use these as starting points; copy and tune for your machine.

<!-- docs-gen:end supported-profile-matrix -->
## Support expectations

Pre-1.0: fixes target the latest release and `main`. Report bugs via GitHub issues; security/safety issues
privately ([`SECURITY.md`](../SECURITY.md)). There is no SLA — pilots should validate output on real
hardware (`verify` enforces only the documented rule catalog).
