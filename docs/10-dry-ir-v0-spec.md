# Dry IR v0 — specification

This is the **normative** specification of the Dry IR **v0** public contract: the L2 motion toolpath, its
JSON wire form, and the two binary encodings `DRY0` and `DRY1`. It is written so that an independent
implementation can read and write Dry files **without** depending on `dry-core`. The companion
machine-readable schema is [`../spec/dry-ir-v0.schema.json`](../spec/dry-ir-v0.schema.json); the public
conformance vectors are under [`../conformance/vectors/`](../conformance/vectors/).

Conformance is defined **semantically** (§9), not by cross-language byte-identity. The reference engine
(`dry-core`) guarantees byte-stable output for its own encoder at fixed settings; that property is
enforced by a drift gate, not promised across implementations (DEFLATE and float formatting are
implementation-defined).

Key words **MUST**, **MUST NOT**, **SHOULD**, **MAY** are used in the RFC 2119 sense.

---

## 1. Scope

Dry IR v0 describes a **resolved L2 toolpath**: an ordered stream of machine-agnostic moves with absolute
state. It is the level `simulate` / `verify` / `optimize` / `emit` operate on. This document covers:

- the value model and units (§2);
- the data model — `Toolpath`, `Meta`, `Segment`, `SegmentKind` (§3);
- the JSON wire form (§4);
- the `DRY0` columnar binary encoding (§5);
- the `DRY1` chunked streaming binary encoding (§6);
- version semantics (§7) and the compatibility policy (§8);
- the conformance model (§9), known inconsistencies (§10), and error modes (§11).

Out of scope for v0: the L0/L1 authoring dialects, the design pass framework, target/dialect emission
rules (g-code is an *output*, not part of the IR contract), and machine/material profiles.

## 2. Value model and units

All quantities are IEEE-754 binary64 (`f64`). Typed quantities — length, feedrate, volume, time, flow,
area, angle — are **unit-typed** in the engine but serialize **transparently** as bare JSON numbers and
as raw little-endian `f64` bits in binary. The canonical length unit is the **millimetre**; feedrate is
**mm/min** (g-code `F` semantics); volume is **mm³**; time is **seconds**.

There are no `NaN` or infinite values in a conforming toolpath. Implementations **MUST** preserve the
exact `f64` bit pattern of every quantity through a round trip (§9).

<!-- docs-gen:start ir-core-model -->
## 3. Data model

### 3.1 `Toolpath`

| Field | Type | Notes |
|---|---|---|
| `version` | `u32` | IR schema version. v0 ⇒ `0`. Always present on the JSON wire (even when `0`). |
| `meta` | `Meta` \| absent | Optional self-describing header (§3.2). Omitted entirely when absent. |
| `segments` | array of `Segment` | Ordered move stream. MAY be empty. |

### 3.2 `Meta`

Optional provenance + declared invariants. Every field is omitted when empty, so a header-free toolpath
has no `meta` key at all.

| Field | Type | Notes |
|---|---|---|
| `generator` | string | Producing tool + version, e.g. `"dry 0.2.0"`. Omitted when absent. |
| `units` | string | Length unit of coordinates, e.g. `"mm"`. Omitted when absent. |
| `source_hash` | string | Hex content hash of the source design. Omitted when absent. |
| `invariants` | array of string | Declared contract names the toolpath claims to satisfy. Omitted when empty. |

### 3.3 `Segment`

One resolved move from `start` to `end` (absolute coordinates). An axis component is `null` when
undefined before it is first set (e.g. the very first positioning move).

| Field | Type | Presence | Notes |
|---|---|---|---|
| `start` | `[number\|null; 3]` | always | absolute start `(x, y, z)` |
| `end` | `[number\|null; 3]` | always | absolute end `(x, y, z)` |
| `travel` | bool | always | non-extruding positioning move |
| `speed` | number | always | feedrate (mm/min) |
| `length` | number | always | true path length (arc length for arcs; `0` for a pure position) |
| `volume` | number | always | deposited material volume (mm³) |
| `filament` | number | always | feedstock length consumed (mm) |
| `width` | number \| null | always (nullable) | bead width |
| `height` | number \| null | always (nullable) | layer height |
| `kind` | string enum | omitted ⇒ `"line"` | see §3.4 |
| `centre` | `[number; 2]` \| null | omitted when null | arc centre `(cx, cy)`; present only for `arc` |
| `clockwise` | bool | omitted ⇒ `false` | arc direction: `true` ⇒ G2, `false` ⇒ G3 |
| `temperature` | number | omitted when unset | nozzle temperature (°C) |
| `fan` | number | omitted when unset | part-cooling fan (0..1) |
| `flow` | number | omitted when unset | flow multiplier; default `1.0` is omitted |
| `tool` | `u32` | omitted when unset | active tool index |
| `dwell_s` | number | omitted when unset | dwell duration (s); present only for `dwell` |
| `manual_gcode` | string | omitted when unset | verbatim g-code; present only for `manualgcode` (see §10) |
| `orientation` | `[number; 3]` | omitted when unset | tool-direction unit vector `(i,j,k)`; `null`/absent ⇒ +Z (3-axis) |
| `control_points` | `[[number; 3]]` | omitted when unset | spline control points; present only for `spline` |

On the JSON wire, fields marked "omitted when unset/null" carry serde `skip_serializing_if`, so a
motion-only toolpath serializes without any channel keys. `start`, `end`, `width`, `height` are always
present but MAY be `null` (per component for the arrays).

### 3.4 `SegmentKind`

The eight resolved primitives. **The on-wire encoding differs by format** (this asymmetry is normative
for v0 — see §10):

| Variant | JSON string | `DRY0` dictionary string | `DRY1` tag (`u8`) |
|---|---|---|---|
| Line | `"line"` | `line` | `0` |
| Arc | `"arc"` | `arc` | `1` |
| Spline | `"spline"` | `spline` | `2` |
| Dwell | `"dwell"` | `dwell` | `3` |
| Retract | `"retract"` | `retract` | `4` |
| Unretract | `"unretract"` | `unretract` | `5` |
| Deposit | `"deposit"` | `deposit` | `6` |
| ManualGcode | `"manualgcode"` | `manual_gcode` | `7` |

A reader **MUST** reject any other JSON string, dictionary string, or tag (§11). An external
implementation **MUST** map all three encodings of a given row to the same logical kind.

## 4. JSON wire form

The JSON wire form is the canonical, human-readable representation.

- **Object** with keys `version`, optional `meta`, `segments` (in that order from the reference encoder).
- **Numbers** are JSON numbers; the reference encoder emits the shortest round-tripping decimal for each
  `f64`. An independent encoder MAY format differently; conformance is by numeric value (§9), not text.
- **Field order** from the reference encoder follows Rust struct declaration order. It is **not** a
  conformance requirement; readers **MUST NOT** depend on key order.
- **Omission**: `meta` and all `skip_serializing_if` fields (§3.3) are absent when unset. `version` is
  always present. `start`/`end`/`width`/`height` are always present (possibly `null`).
- **Unknown object keys**: readers **MUST** ignore unknown object keys (forward-compatibility, §8).
- **Unknown enum strings**: readers **MUST** reject an unknown `kind` string.

A minimal empty toolpath is exactly:

```json
{"version":0,"segments":[]}
```

<!-- docs-gen:end ir-core-model -->
## 5. `DRY0` — columnar binary encoding

`DRY0` is a compact, lossless, **struct-of-arrays** archive: a small plaintext header, then a
DEFLATE-compressed body in which each field is a contiguous column. Columnar layout puts like-valued data
adjacent (a constant feedrate or bead width collapses under compression). All integers are little-endian.

### 5.1 Header (uncompressed, 17 bytes)

| Offset | Field | Type | Value |
|---|---|---|---|
| 0 | magic | 4 bytes | `DRY0` |
| 4 | `enc_ver` | `u8` | `1` (current); `0` legacy — accepted, has no `manual_gcode` column |
| 5 | `ir_ver` | `u32` | `Toolpath.version` |
| 9 | `n` | `u32` | segment count |
| 13 | `body_len` | `u32` | uncompressed body length (the inflate bound) |
| 17 | body | bytes | DEFLATE stream; inflates to exactly `body_len` bytes |

### 5.2 Body (after inflate), in order

Let `n` = segment count. A **bitmap** is `ceil(n/8)` bytes, bit `i` stored at byte `i/8`, LSB-first
(`bit 0` ⇒ `0x01`). A **nullable f64 column** is a validity bitmap followed by `n × f64` (8 bytes each,
LE); absent entries hold a `0.0` placeholder. A **dense f64 column** is `n × f64` with no bitmap.

1. `travel` — bitmap (`n` bits)
2. `clockwise` — bitmap (`n` bits)
3. nullable f64 columns, in order: `start.x`, `start.y`, `start.z`, `end.x`, `end.y`, `end.z`,
   `width`, `height`, `centre.x`, `centre.y`
4. dense f64 columns, in order: `speed`, `length`, `volume`, `filament`
5. nullable f64 columns: `temperature`, `fan`, `flow`, `dwell_s`
6. `tool` — nullable `u32` column: validity bitmap then `n × u32` (absent ⇒ `0`)
7. `orientation` — nullable vec3 column: validity bitmap then `n × (3 × f64)` (absent ⇒ `[0,0,0]`)
8. `control_points` — validity bitmap (`n` bits); then, for each *valid* segment in order:
   `u32 count`, then `count × (3 × f64)` `(x,y,z)`
9. `manual_gcode` *(only when `enc_ver == 1`)* — validity bitmap (`n` bits); then, for each *valid*
   segment in order: `u32 byte-length`, then that many UTF-8 bytes
10. **kind dictionary**: `u32 dict_len`; then `dict_len ×` (`u32 str_len`, `str_len` UTF-8 bytes) using
    the **`DRY0` dictionary strings** of §3.4; then `n × u32` dictionary indices (one per segment)
11. **meta trailer**: `u8 present` (`0`/`1`); if `1`: `u32 json_len`, then `json_len` UTF-8 bytes of the
    `Meta` header serialized as JSON

Note the column order differs from the row order in `DRY1` (§6.3): here `control_points` precedes
`manual_gcode`; `manual_gcode` precedes the kind dictionary.

## 6. `DRY1` — chunked streaming binary encoding

`DRY1` keeps the same values but stores compressed **row** chunks, so a reader can decode one bounded
block at a time. The reference encoder uses a block size of `512` segments.

### 6.1 Header (uncompressed)

| Field | Type | Value |
|---|---|---|
| magic | 4 bytes | `DRY1` |
| `enc_ver` | `u8` | `2` (current); `1` legacy — accepted, no `manual_gcode` |
| `ir_ver` | `u32` | `Toolpath.version` |
| `n` | `u32` | total segment count |
| `block_size` | `u32` | segments per block (MUST be ≥ 1) |
| meta | `u8 present`; if `1`: `u32 json_len` + JSON bytes | the `Meta` header |

### 6.2 Blocks (repeated until `n` segments are read)

| Field | Type | Notes |
|---|---|---|
| `block_n` | `u32` | segments in this block (MUST be `≥1` and `≤` remaining) |
| `body_len` | `u32` | uncompressed block length (the inflate bound) |
| `deflate_len` | `u32` | compressed byte length that follows |
| body | `deflate_len` bytes | DEFLATE stream; inflates to exactly `body_len` bytes of `block_n` rows |

After inflating a block, a reader **MUST** consume exactly `body_len` bytes across `block_n` rows; trailing
bytes are an error.

### 6.3 Segment row layout, in order

Each row begins with a `u32` **flags** word and a `u8` **kind tag** (§3.4), then only the present fields.
A field is present iff its flag bit is set. Always-present fields (`speed`, `length`, `volume`,
`filament`) carry no flag.

Flag bits:

| Bit | Field | Bit | Field |
|---|---|---|---|
| 0 | `travel` | 10 | `centre` (2×f64) |
| 1 | `clockwise` | 11 | `temperature` |
| 2 | `start.x` | 12 | `fan` |
| 3 | `start.y` | 13 | `flow` |
| 4 | `start.z` | 14 | `dwell_s` |
| 5 | `end.x` | 15 | `tool` (u32) |
| 6 | `end.y` | 16 | `orientation` (3×f64) |
| 7 | `end.z` | 17 | `control_points` |
| 8 | `width` | 18 | `manual_gcode` *(enc_ver 2 only)* |
| 9 | `height` | | |

`travel` and `clockwise` are pure flag bits (no payload). The known-flags mask is bits `0..=18`
(`0x7FFFF`) for `enc_ver 2`, and `0..=17` (`0x3FFFF`) for legacy `enc_ver 1`. Any set bit outside the
mask is an error (§11).

Payload order within a row (each present only if its flag is set, except the four dense fields):

1. `start.x`, `start.y`, `start.z` (f64 each)
2. `end.x`, `end.y`, `end.z` (f64 each)
3. `width`, `height` (f64 each)
4. `centre` → 2 × f64 `(cx, cy)`
5. **dense**: `speed`, `length`, `volume`, `filament` (f64 each, always)
6. `temperature`, `fan`, `flow`, `dwell_s` (f64 each)
7. `manual_gcode` → `u32 byte-length` + UTF-8 bytes
8. `tool` → `u32`
9. `orientation` → 3 × f64
10. `control_points` → `u32 count` + `count × (3 × f64)`

Note: in `DRY1` rows `manual_gcode` precedes `tool`/`orientation`/`control_points`; in `DRY0` columns it
follows `control_points`. Both orders are normative for their respective formats.

## 7. Version semantics

There are **three independent version axes**. A reader **MUST NOT** conflate them:

| Axis | Field | v0 value | Legacy value still accepted |
|---|---|---|---|
| IR schema version | `Toolpath.version` (`ir_ver`) | `0` | — |
| `DRY0` encoding version | header `enc_ver` | `1` | `0` (no `manual_gcode` column) |
| `DRY1` encoding version | header `enc_ver` | `2` | `1` (no `manual_gcode`) |

The IR schema version is carried unchanged through both binary headers, so a binary file records both its
encoding version and the IR schema version it transports.

## 8. Versioning and compatibility policy

Dry IR follows **SemVer** at the IR-schema level:

- **Minor** (backward-compatible): adding an optional field (omitted-when-unset), or adding a new
  `SegmentKind`. Old readers ignore unknown JSON keys; binary encodings gate new fields behind new flag
  bits / columns guarded by a new `enc_ver`.
- **Major** (breaking): removing, renaming, or retyping a field; changing a default; changing the meaning
  of an existing field or encoding.

Reader rules (current behavior, normative for v0):

- Unknown **JSON object keys** are ignored (forward-compatibility).
- Unknown **`SegmentKind`** strings / dictionary entries / tags are rejected (§11).
- Unknown **`enc_ver`** is rejected.
- Unknown **`DRY1` flag bits** outside the known mask are rejected.

**Compatibility promise:** a valid v0 file MUST keep decoding under future releases unless a major bump
explicitly migrates it. This is enforced by `frozen: true` conformance vectors (§9) that a regression
test MUST always decode.

## 9. Conformance model

Because DEFLATE output (miniz_oxide level 8 in the reference) and canonical-JSON float formatting are
implementation-defined, conformance is **semantic**, not cross-language byte-identity.

**Semantic equality** of two toolpaths: equal `version`; equal `meta` (field-by-field); equal segment
count; and for each segment, exact `f64` bit-equality of every numeric quantity, and structural equality
of every option / enum / array (including per-component `null` of `start`/`end`).

An implementation **conforms** if, for every published vector:

- **JSON**: it parses `input.json` to a toolpath semantically equal to the reference, and re-serializes
  to a toolpath that re-parses to a semantically equal toolpath.
- **`DRY0` / `DRY1`**: it decodes the reference `expected.dry0` / `expected.dry1` to a semantically equal
  toolpath, **and** its own encode→decode round-trips losslessly (semantic equality).

Byte-identity is **not** required across implementations. Within one implementation at fixed settings,
encoding is deterministic; the reference engine's byte-stability is enforced by the `dry-core` drift gate
(`crates/core/tests/spec_vectors.rs`), not by this contract.

`metrics.json` and `expected.gcode` in each vector are **published reference outputs** of `simulate` /
`emit`. They are not part of the codec contract and are not required to be re-derived by a
format-conformant reader.

## 10. Known inconsistencies (normative for v0)

The `ManualGcode` kind is encoded three different ways (§3.4): JSON `"manualgcode"`, `DRY0` dictionary
`"manual_gcode"`, `DRY1` tag `7`. This asymmetry is a wart, frozen into v0 for compatibility. An external
implementation **MUST** honor all three. Unifying the spellings is a **major** change deferred to a
future IR version; it MUST NOT be silently "fixed" by a v0 reader/writer.

## 11. Error modes (documented failures)

A conforming reader **MUST** reject the following, each exercised by a vector under
`conformance/vectors/_negative/`:

| Condition | Required rejection |
|---|---|
| Wrong magic (not `DRY0`/`DRY1`) | bad-magic error |
| Unsupported `enc_ver` | unsupported-version error |
| Unknown `SegmentKind` (JSON string, `DRY0` dict entry, or `DRY1` tag) | bad-kind error |
| `DRY1` flag bit outside the known mask | unsupported-flags error |
| Truncated body (inflate short, or a column/row runs past the buffer) | truncated error |
| Inflated body length ≠ declared `body_len` | bad-compression error |
| Non-UTF-8 string bytes | bad-utf8 error |
| `DRY1` `block_size == 0`, or `block_n == 0`/`> remaining` | error |

The error *taxonomy* (names) is implementation-defined; the **requirement to reject** is normative.
