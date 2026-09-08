# APT-CL lift and IRBCAM/APT emission — dialect contract

- **Status:** Promoted normative contract (promoted from draft 1; incorporated adversarial critique)
- **Workstream:** D1.x target dialects; FM1 claims `FM1.APT.*`, `FM1.IRBCAM.*`
- **Governing decisions:** ADR 0001 (assurance layers, relation vocabulary — [docs/adr/0001-formal-assurance-constitution.md](docs/adr/0001-formal-assurance-constitution.md)), ADR 0002 (ingress/emission gates, refuse-never-clamp — [docs/adr/0002-numeric-ingress-and-emission-gates.md](docs/adr/0002-numeric-ingress-and-emission-gates.md)), plan §6.6 ([docs/21-mathematical-assurance-plan.md](docs/21-mathematical-assurance-plan.md)) and §13 stop/go gates ([docs/21-mathematical-assurance-plan.md](docs/21-mathematical-assurance-plan.md)).

> **Reading order.** The mandated section numbering below puts the data model (§4) before the observation set (§7) and the theorem inventory (§8). The document was *written* the other way round: §7 fixes what a reader of the output is allowed to recover, §6 fixes what never crosses the boundary, §8 fixes what is proved about the remainder, and only then do §4 (data model), §5 (definitions) and §3 (code placement) follow as the smallest structures that make those theorems provable and refinable. Where a §4/§5 choice looks arbitrary, its reason is in §7 or §8.

> **Provenance note on §2.** External-format statements are tagged working assumptions (`⚠F-n`) that the rest of the document depends on and that are slated for mechanical fact verification (open decision OD-0, §12). Every definition in §5 is parameterised so that a corrected fact changes a constant or a sign, not a theorem.

---

## 1. Goal, non-goals, flows

### 1.1 Goal

Carry a CAM-authored motion program into IRBCAM through Dry's L2 motion dialect such that:

1. every observation IRBCAM will act on (§7.2, `O_irbcam`) is either carried faithfully or the program is refused with a located diagnostic;
2. every input construct is in exactly one of three classes — **modeled**, **inert-preserved** (recorded, never lowered to motion), **refused** — with no fourth "silently dropped" class (ADR 0002 §4, `docs/adr/0002-numeric-ingress-and-emission-gates.md:45-49`);
3. every correctness statement is placed on one ADR 0001 layer (abstract / numeric / implementation / physical) and uses exactly one relation from the closed set (`docs/adr/0001-formal-assurance-constitution.md:40-53`).

### 1.2 Non-goals

- Proving that IRBCAM, a robot controller, or a robot behaves in any way. Everything downstream of the emitted file is Layer D (physical) and is recorded as qualification evidence, never as a theorem (ADR 0001 layer 4; ADR 0002 §5 `docs/adr/0002-numeric-ingress-and-emission-gates.md:51-55`).
- Robot reachability, collision, singularity handling, or 6-DOF joint solutions. `Segment.orientation` is a tool-direction vector, not a pose (`crates/core/src/ir.rs:119-122`); the sixth DOF is a declared constant (§5.3).
- Modelling APT cutter compensation, transformations, or canned cycles other than plain drilling. These are **refused**, not approximated (§6).
- Extrusion semantics on a robot. The Cura flow carries motion; how (or whether) the extrusion channels are represented in IRBCAM is an owner decision (§12 OD-1) and, whatever is chosen, a **declared loss** (§7.4).
- Replacing the Fusion 360 Dry post (`integrations/fusion360/dry_fusion_postprocessor.cps`). The Fusion flow below uses Autodesk's stock APT post, not that file (see §3.6 for what happens to it).

### 1.3 The two end-to-end flows

**Flow A — Fusion 360 → APT-CL → dry → IRBCAM**

```
Fusion 360 (Manufacture) ──post: Autodesk apt.cps──► part.apt        (L3 source dialect: APT-CL text)
part.apt ── dry import-apt ──► part.ir.json                          (L2, meta.invariants ⊇ ["imported-from-apt"])
part.ir.json ── dry review-apt / dry verify ──► report                (findings incl. unmodeled-apt)
part.ir.json ── dry emit --format irbcam ──► part.irbcam.json        (L3 target dialect: IRBCAM target list)
part.irbcam.json ── IRBCAM import (Layer D, manual) ──► robot program
```

**Flow B — Cura → G-code → dry → IRBCAM**

```
Cura ──► part.gcode (Marlin flavor)                                   (L3 source dialect: RS-274/FFF g-code)
part.gcode ── dry import-gcode ──► part.ir.json                       (existing lifter, crates/core/src/gcode/lift.rs:434-560)
part.ir.json ── dry emit --format irbcam --irbcam-extrusion <policy> ──► part.irbcam.json
```

Flow B adds **no** new ingress path: the existing G-code lifter is the L3→L2 step, exactly as `docs/01-architecture.md:131-133` positions `parse(bytes, dialect) -> ir@L2`. Flow B's only new element is the IRBCAM lowering and its extrusion policy (§5.8, §12 OD-1). The two flows therefore share one emitter, and every emitter theorem in §8 covers both.

**Round trip (assurance only, not a user flow).** L2 → APT (`dry emit --format apt`) → `dry import-apt` → L2 is what the observational round-trip claim (§7.3, claim A7) is stated over. The APT emitter exists to make that claim refinable in-repo; whether it also ships as a user-facing target is OD-10 (§12).

---

## 2. External format facts — as assumed by this draft (`⚠` = unverified, see OD-0)

Each row is a working assumption. The column "used by" names the definition that consumes it, so that a corrected fact can be propagated mechanically.

### 2.1 IRBCAM target list (native import format)

| Tag | Assumption | Used by |
|---|---|---|
| ⚠F-1 | IRBCAM imports a **target list** in JSON and in CSV; both carry the same logical record set. | §4 (columns), §5.9, OD-5 |
| ⚠F-2 | A target carries a position `x, y, z` in **millimetres**. | §5.1, §5.9 |
| ⚠F-3 | A target carries an orientation as **ZYZ Euler angles `rz1, ry, rz2`** in **degrees** (configurable to radians via `AngleUnit`), `R = Rz(rz1)·Ry(ry)·Rz(rz2)`, applied to the tool frame; the tool axis is the frame's Z axis. | §5.3 |
| ⚠F-4 | The sign convention of the tool Z axis (toward or away from the work) — **unknown to this draft**; parameterised as `σ ∈ {+1, −1}` in §5.3 (OD-6). | §5.3 |
| ⚠F-5 | A target carries a `velocity` in **mm/s**; the literal value **`-1` means rapid** (controller-defined speed). | §5.2 |
| ⚠F-6 | A target carries a `type` ∈ {`0`, `1`}: `0` = ordinary (linear) target, `1` = **arc midpoint**; a type-1 target must be immediately followed by a type-0 endpoint, and the triple (previous target, midpoint, endpoint) defines the circular move. | §5.4, §8 (I3, I8) |
| ⚠F-7 | Tool number and spindle speed are **sparse vectors** of `(targetIndex, value)` pairs (`toolNumber`, `spindleSpeed`), each applying from that index onward; indices are 0-based, strictly increasing, and less than the target count. | §5.7, §8 (I2, I3) |
| ⚠F-8 | Spindle speed is in **RPM**; tool number is a non-negative integer. | §5.7 |
| ⚠F-9 | IRBCAM requires **at least 6 decimal places** on numeric fields (position, angles, velocity). | §5.10 |
| ⚠F-10 | IRBCAM has **no dwell / wait target**. | §5.9, §7.4 |
| ⚠F-11 | IRBCAM's plugin API (QML/JS) is **read-only** with respect to targets: a plugin can read the loaded target list but not write it. | §10.4, §13 |

### 2.2 APT-CL as specified in ISO 4343:2000(E) and written by Autodesk's stock `apt.cps` (Fusion 360)

| Tag | Source & Citation / Specification | Used by |
|---|---|---|
| ✓F-12 | **Statement set:** `PARTNO` (ISO 4343 Cl. 5.37), `MACHIN` (Cl. 5.28), `UNITS/MM` or `UNITS/INCHES` (Cl. 5.59), `PPRINT/…` (Cl. 5.39), `LOADTL/n[,…]` (Cl. 5.27), `SPINDL/RPM,n,CLW\|CCLW` and `SPINDL/OFF` (Cl. 5.51), `COOLNT/ON\|OFF\|…` (Cl. 5.10), `FEDRAT/MMPM,f` or `FEDRAT/IPM,f` (Cl. 5.17 `PERMIN` parameterization), `RAPID` (Cl. 5.42), `MULTAX/ON\|OFF` (Cl. 5.32), `GOTO/x,y,z[,i,j,k]` (Cl. 5.21), `MOVARC/cx,cy,cz,nx,ny,nz,r,θ` immediately followed by `GOTO/x,y,z[,i,j,k]` (arc endpoint; Autodesk/APT CAM processor extension), `DELAY/t` (Cl. 5.13 `DWELL` duration in seconds), `CYCLE/DRILL,…` … `CYCLE/OFF` (Cl. 10.8), `CUTCOM/LEFT\|RIGHT\|OFF` (Cl. 5.11), `INSERT/…` (Cl. 5.24), `FINI` (Cl. 5.18). | §4, §5, §6 |
| ✓F-13 | **`RAPID` one-shot semantics:** ISO 4343:2000 Clause 5.42 states verbatim: *"This command specifies that the following motion shall be performed at a rapid traverse rate. Subsequent motions revert back to the last specified feed velocity (see 5.17.2)."* Verified: `RAPID` is strictly a one-shot modal prefix applying to the next motion statement only. | §5.2 |
| ✓F-14 | **`MULTAX` state machine:** ISO 4343:2000 Clause 5.32 states: *"This command specifies whether multiaxis positioning is required or not. If ON, the NC processor produces multiaxis motion data. If OFF, the NC processor produces 3-axis motion data."* Under `MULTAX/ON` a `GOTO` carries six values (control point + tool-axis unit direction vector `i, j, k`, Cl. 5.21); under `MULTAX/OFF` three values (`x, y, z`). If the parameter is omitted (bare `MULTAX`), default is `ON`. | §5.6 |
| ⚠F-15 | **`MOVARC` circular motion:** `MOVARC` gives centre, plane normal, radius and **positive sweep in degrees**; direction is the right-hand rule about the normal; the endpoint is the `GOTO` that follows immediately. (Autodesk/APT CAM extension; circular interpolation in ISO 4343 Cl. 5.31 / vendor posts). | §5.4 |
| ✓F-16 | **`CYCLE/DRILL`:** ISO 4343:2000 Clause 10.8.7 defines `CYCLE / DRILL, [ON | OFF |] DEPTH, a, [PERMIN | PERREV | FPT,] b, CLEAR, c [, RAPTO, d] [, RETURN, e]`. Minor words used by `apt.cps`: depth, feed unit (`MMPM`/`IPM`), feed, `RAPTO` clearance distance, optional `DWELL` seconds. Each subsequent `GOTO` is a hole location; `CYCLE/OFF` ends the cycle. All other canned cycles (Clause 10.8.2 `BORE`, 10.8.4 `BRKCHP`, 10.8.6 `CSINK`, 10.8.8 `DEEP`, 10.8.12 `FACE`, 10.8.15 `REAM`, 10.8.19 `TAP`, etc.) are explicitly refused (`[cycle-unsupported]`). | §5.5 |
| ✓F-17 | **Continuation and comments:** ISO 4343:2000 Clause 4.2 specifies that a statement may continue onto the next line after a trailing `$`, and `$$` begins a line comment. | §3.2, §11 |

### 2.3 APT-CL variants from CATIA / Siemens NX (read, not written, by this spec)

| Tag | Source & Citation / Specification | Used by |
|---|---|---|
| ⚠F-18 | Arcs are written as `CIRCLE/cx,cy,cz,i,j,k,r[,…]` **followed by the linearised `GOTO` points** that lie on the circle; there is no sweep or endpoint word on the `CIRCLE` record itself (CATIA / Siemens NX post convention). | §5.4 (CIRCLE policy), OD-9 |
| ✓F-19 | **`DWELL` specification:** ISO 4343:2000 Clause 5.13 specifies `DELAY / [DWELL | REV,] a`, where `DWELL` explicitly specifies duration `a` in seconds (the default if omitted). CATIA/NX convention `DWELL/t` is equivalent. | §5.9 |
| ✓F-20 | **`SPINDL` syntax variants:** ISO 4343:2000 Clause 5.51 specifies `SPINDL / [ON | OFF | [RPM | SFM | SMM,] a [, CLW | CCLW | ORIENT, b]]`. Defaults to `RPM`. Variants `SPINDL/n,CLW`, `SPINDL/RPM,n,CLW`, `SPINDL/ON`, `SPINDL/OFF` all conform. | §5.7 |

---

## 3. Architecture placement

### 3.1 Dialect positions

| Artifact | Dry dialect level | Direction | Module |
|---|---|---|---|
| APT-CL text | L3 (target/source dialect) | lift → L2 (`parse` in `docs/01-architecture.md:131`), lower ← L2 (`emit`) | `crates/core/src/apt.rs` (scanner/parser), `crates/core/src/apt/lift.rs` (lifter), `crates/core/src/emit/apt.rs` (emitter) |
| IRBCAM target list JSON / CSV | L3 (target dialect) | lower ← L2 only | `crates/core/src/emit/irbcam.rs` |
| Dry IR v0 | L2 | hub | unchanged (`crates/core/src/ir.rs:67-157`) |
| L1 | — | recovered by `reverse` if wanted (`crates/core/src/reverse.rs:23`) | unchanged |

APT-CL is lifted to **L2, not L1**, for the reason the integrations map already documents: L1 has no rapid-vs-feed distinction (`Op::Move` is a travel iff the extruder channel is off, `crates/core/src/resolve.rs:47-123`), whereas `Segment.travel`, `speed`, `power`, `tool`, `orientation`, `dwell_s` and `centre/clockwise` (`crates/core/src/ir.rs:67-126`) are exactly the observations §7 needs. The module split mirrors `crates/core/src/gcode.rs` + `gcode/lift.rs` (parser module declaring `mod lift;` and re-exporting the import API, `crates/core/src/gcode.rs:7-13`).

### 3.2 Core API surface (new)

Following the naming precedent of the G-code family (`crates/core/src/lib.rs:89-94`):

```rust
// crates/core/src/apt.rs
pub struct AptParser<R: Read> { .. }               // statement scanner: `$` continuation, `$$` comments (⚠F-17)
pub struct ParsedAptStatement { source_line: usize, line_count: usize, raw: String, record: AptRecord }
pub enum AptRecord { Motion(Box<AptMotion>), State(AptState), Process(AptProcess), Inert { major: String }, Refused { major: String, reason: AptRefusalReason }, Comment, Empty }
pub struct AptParseError { source_line: usize, code: AptErrorCode, message: String }
pub fn parse_apt_statements(source: &str, limits: &AptImportLimits) -> Result<Vec<ParsedAptStatement>, AptParseError>;

// crates/core/src/apt/lift.rs
#[derive(Clone, Copy, PartialEq)]
pub struct AptImportParams {
    pub version: u32,
    pub assume_units: Option<AptUnits>,          // None ⇒ a program without UNITS/ is refused (§6 R-UNITS)
    pub unknown_major_words: UnknownMajorWordPolicy, // Refuse (default) | Preserve
    pub arc_tolerance_rel: f64,                   // default 1e-6, same idiom as gcode/lift.rs:1000 and verify.rs:679
}
pub struct AptImportLimits { pub max_source_bytes, pub max_statements, pub max_line_bytes, pub max_continuation_lines, pub max_values_per_statement, pub max_cycle_holes: usize } // Default pinned in §6.6
pub struct AptImportError { pub source_line: usize, pub code: AptErrorCode, pub message: String } // Display: "line {n}: [{code}] {msg}"
pub struct ImportedApt {
    pub toolpath: Toolpath,
    pub source_lines: Vec<String>,
    pub segment_source_lines: Vec<usize>,          // many-to-one (cycle expansion), 1-based
    pub statement_segments: Vec<Option<std::ops::Range<usize>>>, // per statement: contiguous segment range it produced
    pub unmodeled_statements: Vec<UnmodeledApt>,   // inert-preserved statements (§6.2 class I)
    pub advisories: Vec<AptAdvisory>,              // e.g. CIRCLE annotation ignored, spindle direction dropped
}
pub struct UnmodeledApt { pub source_line: usize, pub major: String, pub raw: String }
pub fn import_apt(source: &str, params: &AptImportParams) -> Result<Toolpath, AptImportError>;
pub fn import_apt_with_map(source: &str, params: &AptImportParams) -> Result<ImportedApt, AptImportError>;
pub fn import_apt_reader<R: Read>(reader: R, params: &AptImportParams) -> Result<Toolpath, AptImportError>;
pub fn import_apt_reader_with_map<R: Read>(reader: R, params: &AptImportParams) -> Result<ImportedApt, AptImportError>;
pub fn import_apt_reader_with_limits<R: Read>(reader: R, params: &AptImportParams, limits: &AptImportLimits) -> Result<ImportedApt, AptImportError>;
pub fn import_parsed_apt_with_map<I: IntoIterator<Item = ParsedAptStatement>>(statements: I, params: &AptImportParams) -> Result<ImportedApt, AptImportError>;

// crates/core/src/emit/irbcam.rs
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Debug)]
pub enum AngleUnit { #[default] Deg, Rad }

#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct IrbcamFrame {                         // the dialect frame, KrlFrame pattern (crates/core/src/emit/krl.rs:211-227)
    pub spin_deg: f64,                            // rz2 constant (§5.3); default 0.0 (OD-2)
    pub angle_unit: AngleUnit,                    // Deg (default) | Rad
    pub tool_z_toward_work: bool,                 // σ (⚠F-4, OD-6); fixed once the fact is known, not a user knob
    pub rapid: RapidEncoding,                     // MinusOne (default) | Explicit (OD-4)
    pub dwell: DwellPolicy,                       // Refuse (default) | Drop (OD-8)
    pub extrusion: ExtrusionCarry,                // Refuse (default) | MotionOnly | ToolToggle { extrude_tool, travel_tool } | SpindleRate (OD-1)
    pub decimals: u8,                             // 6..=17, default 6 (⚠F-9, §5.10)
    pub layout: IrbcamLayout,                     // Json (default) | Csv (OD-5)
}
impl IrbcamFrame { pub fn validate(&self) -> Result<(), String>; }
pub struct IrbcamEmitStats { pub targets: usize, pub arcs: usize, pub dropped_dwells: usize, pub dropped_poseless: usize, pub extrusion_segments_lowered_motion_only: usize }
pub fn emit_irbcam_to_writer<I, W>(segments: I, params: &EmitParams, writer: &mut W) -> Result<IrbcamEmitStats, CodecError>
where I: IntoIterator<Item = Result<Segment, CodecError>>, W: std::io::Write;

// crates/core/src/emit/apt.rs
#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AptFrame { pub partno: Option<String>, pub machin: Option<String>, pub decimals: u8 /* default 6 */ }
pub fn emit_apt_to_writer<I, W>(segments: I, params: &EmitParams, writer: &mut W) -> Result<(), CodecError>;
```

Deviation, stated: `emit_irbcam_to_writer` returns `IrbcamEmitStats` rather than `()` (the convention at `crates/core/src/emit/rapid.rs:115-123`, `crates/core/src/emit/krl.rs:398-405`). The reason is ADR 0002 §4: the only policies that do not refuse a dwell or an extrusion segment *drop* it, and a drop is admissible only if it is loud. The stats are the loudness; the CLI prints them to stderr and `--irbcam-strict` turns any nonzero drop count into exit 1.

`EmitParams` (`crates/core/src/emit/gcode.rs:87-117`) gains `irbcam_frame: IrbcamFrame` and `apt_frame: AptFrame`, both `#[serde(default)]`, read only under their flavor — the `krl_frame` pattern (`crates/core/src/emit/gcode.rs:108`).

### 3.3 Flavor registration

`FirmwareFlavor` (`crates/core/src/emit/gcode.rs:7-28`) gains three variants; the single wire-name parser `FirmwareFlavor::named` (`crates/core/src/emit/gcode.rs:41-58`) gains their names; an unknown name stays an error.

| Variant | `named()` accepts | `has_extruder` | `is_cnc` | `is_robot` | CLI `EmitOutputFormat` (`crates/cli/src/main.rs:34-54`) |
|---|---|---|---|---|---|
| `Irbcam` | `irbcam`, `irbcam-json` | false | false | **true** | `Irbcam` (alias `irbcam-json`) |
| `IrbcamCsv` | `irbcam-csv` | false | false | **true** | `IrbcamCsv` |
| `Apt` | `apt`, `apt-cl`, `aptcl` | false | false | false | `Apt` (alias `apt-cl`) |

Dispatch: a new arm at the top of `emit_stream_to_writer` (`crates/core/src/emit/gcode.rs:242`) routes the three flavors to their renderers **before** the existing robot arm at `:251`, because that arm refuses the `power` channel for KRL/RAPID (`:254-263`) and IRBCAM *does* render it (§5.7). The exhaustive dwell `match` (`crates/core/src/emit/gcode.rs:483-497`) gains the three variants in the "unreachable — refused" arm alongside `RobotKrl | Rapid` (`:494`). `emit/mod.rs` declares `mod irbcam; mod apt;` and re-exports `emit_irbcam_to_writer, IrbcamFrame, IrbcamEmitStats, emit_apt_to_writer, AptFrame` (pattern `crates/core/src/emit/mod.rs:36-39`); `lib.rs` re-exports them from the crate root (`crates/core/src/lib.rs:70`) and the APT import family beside the G-code one (`:89`).

`every_flavor_has_a_name_and_an_unknown_one_is_refused` (`crates/core/tests/cnc_industrial_flavors.rs:279`) gains the three name sets.

**Prerequisite fix (necessary, §9 R-17).** `Profile::emit_params` carries its own flavor `match` ending in `_ => FirmwareFlavor::Marlin` (`crates/core/src/profile/mod.rs:484-491`). A profile with `firmware.flavor: "irbcam"` would therefore silently emit Marlin — the exact substitution `named()` was written to eliminate (`crates/core/src/emit/gcode.rs:31-40`). The slice routes that `match` through `FirmwareFlavor::named` (unknown ⇒ profile validation error) and lists the new names in `spec/dry-profile-v1.schema.json`'s `firmware.flavor` description.

### 3.4 CLI

| Command / flag | Behaviour | Precedent |
|---|---|---|
| `dry import-apt <file> [--profile P] [--assume-units mm\|inches] [--unknown-major-words refuse\|preserve] [--arc-tolerance-rel ε] [-o OUT]` | `import_apt_reader_with_limits` → `Toolpath::to_json` → stdout/`--out`; `die("cannot import {file}: {e}")` | `Cmd::ImportGcode` (`crates/cli/src/main.rs:522-540`, arm `:1963-1988`) |
| `dry review-apt <file> [--profile P] [--json]` | `import_apt_reader_with_map` + `simulate` + `verify` + `ReviewReport::build` + `add_unmodeled_apt` (new, mirrors `report.rs:122`) + advisories; exit 1 on errors | `Cmd::ReviewGcode` (`crates/cli/src/main.rs:541`, arm `:1990`) |
| `dry emit --format irbcam\|irbcam-csv\|apt` | three new `EmitOutputFormat` arms in the flavor override `match` (`crates/cli/src/main.rs:1562-1571`) | existing |
| `--irbcam-spin-deg F`, `--irbcam-angle-unit deg\|rad`, `--irbcam-rapid minus-one\|explicit`, `--irbcam-dwell refuse\|drop`, `--irbcam-extrusion refuse\|motion-only\|tool-toggle=E,T\|spindle-rate`, `--irbcam-decimals N`, `--irbcam-strict` | populate `IrbcamFrame`; validated before the first byte (§6.5) | `--rotary-axes`, `--step-nc` (`crates/cli/src/main.rs:461-494`) |
| `--apt-partno S`, `--apt-machin S` | populate `AptFrame` | — |

Output is staged and renamed on `Ok` exactly as every streaming emitter (`stage_atomic` `:1201`, `commit_atomic` `:1224`, `write_program` `:1237`); `emit_leaves_no_file_behind_when_the_program_is_refused` (`crates/cli/tests/cli.rs:3000`) is extended to the three formats. `IrbcamEmitStats` is printed to stderr after a successful commit.

Profile plumbing (a `machine.irbcam` block) is **deferred** like KRL's (`docs/22-krl-emit.md:267-307` "No profile plumbing"); flags are the v0 surface.

### 3.5 Binding parity obligations

`conformance/capability-parity.toml` (`:1-40` header; `check_capability_parity.py:68` completeness check) gains two rows; every cell below is a commitment the ledger will enforce in both directions.

| id | core | cli | python | wasm | ts |
|---|---|---|---|---|---|
| `apt-import` | `crates/core/src/apt/lift.rs` `pub fn import_apt_with_map` reachable | `crates/cli/src/main.rs` `Cmd::ImportApt` reachable | `py/src/lib.rs` `import_apt_json` **absent** — note: "Python has no G-code import either (`_native` registration `py/src/lib.rs:663-694`); APT import lands with the import-binding parity slice, not before it" | `crates/wasm/src/lib.rs` `import_apt_to_ir` **absent** — note: "IRBCAM is a desktop consumer; the browser inspection path (`import_gcode_to_ir`, `:463`) has no APT user yet" | `sdk/ts/src/engine.ts` `importApt` **absent** — same note |
| `irbcam-emit` | `crates/core/src/emit/irbcam.rs` `pub fn emit_irbcam_to_writer` reachable | `crates/cli/src/main.rs` `EmitOutputFormat::Irbcam` reachable | `py/src/lib.rs` `FirmwareFlavor::named` reachable (via `resolve_gcode(flavor="irbcam")`, `py/src/lib.rs:94-112`) | `crates/wasm/src/lib.rs` `FirmwareFlavor::named` reachable (`:80`) | `sdk/ts/src/engine.ts` `'irbcam'` reachable — the hand-maintained `FirmwareFlavor` union (`sdk/ts/src/engine.ts:200-215`) gains `'irbcam' \| 'irbcam-csv' \| 'apt'` |

Honest caveat recorded in `docs/14-known-limitations.md` (parity table, `:59-110` region): through the bindings, `irbcam` is reachable only from an **L1 design** (`resolve_gcode`), with `IrbcamFrame` at its defaults — the same limitation KRL's `krl_frame` has (`docs/22-krl-emit.md:267-307`). The imported-L2 path (Flows A and B) is CLI-only in v0.

### 3.6 What happens to `integrations/`

- `integrations/mastercam_nx/dry_apt_cl_converter.py`, its test and README are **removed** and replaced by a README pointing at `dry import-apt` / `dry emit --format irbcam`. The converter is superseded rather than fixed: it never emits `extruder on` so every `GOTO` lowers to a travel at `travel_speed` and `FEDRAT` is discarded (`dry_apt_cl_converter.py:88`, resolve semantics `crates/core/src/resolve.rs:47-56`); it maps `SPINDL` to `power` (`:62`) then selects `rs274`/`marlin` (`:108`), both of which refuse the channel (`crates/core/src/emit/gcode.rs:421`), so any file with `SPINDL` fails; it treats bare `MULTAX` as OFF (`:46`, contradicts ⚠F-14); and it performs no verification despite its README (`integrations/mastercam_nx/README.md:22`). Recorded under `### Removed` in `CHANGELOG.md` `[Unreleased]` (`CHANGELOG.md:10`).
- `integrations/fusion360/README.md:12-14` claims the `.cps` "evaluates machine safety contracts"; the post declares `dryVerification` (`dry_fusion_postprocessor.cps:26`) and never reads it, and `onCircular` (`:74-79`) writes an XY `I`/`J` arc regardless of plane. The README is corrected to say the post is an unverified RS-274 writer, and Flow A is documented as the supported Fusion path. Changing the `.cps` itself is out of scope (§13).
- `integrations/robodk/dry_robodk_bridge.py:43-69` consumes L2 dicts (`end`, `orientation`, `speed`, `travel`); nothing here changes L2, so it is unaffected.

### 3.7 Documentation obligations

`docs/28-apt-irbcam-dialects.md` (this document, honest-boundary shape of `docs/22-krl-emit.md:11-70, 267-307`), a Targets row each for IRBCAM and APT in `docs/16-support-matrix.md:38`, the parity table in `docs/14-known-limitations.md`, the regeneration list in `CONTRIBUTING.md:46-62`, `docs/11-profiles-and-reports.md` rule catalogue (new rule ids, §11), and `CHANGELOG.md [Unreleased]` (`### Added`, `### Removed`).

---

## 4. Data model mapping

Legend for the **loss** column (ADR 0001 relations, `docs/adr/0001-formal-assurance-constitution.md:40-51`): `exact` = carried structurally; `trace-exact` = same ordered abstract trace, structure not recovered; `approximate(ε)` = carried up to a named tolerance; `observational` = recoverable on `O` but not equal; `rejection` = never crosses; `inert` = preserved as text, reported, never motion; `loss` = declared loss (§7.4), admissible only under an explicit policy and counted.

### 4.1 Motion and state

| APT-CL (source, ⚠F-12..20) | L2 `Segment` (`crates/core/src/ir.rs:67-126`) | IRBCAM target (⚠F-2..8) | APT→L2 | L2→IRBCAM |
|---|---|---|---|---|
| `UNITS/MM` \| `UNITS/INCHES` | all `Length` in mm (`crates/core/src/units.rs:109-135`); scale κ ∈ {1, 127/5} | positions mm | `exact` (ℚ); f64 boundary `FM1.F64.APT.UNITS.INCH_SCALE` | — |
| `GOTO/x,y,z` (feed) | `kind: Line`, `end`, `travel: false`, `speed` mm/min | `x,y,z`, `velocity = speed/60`, `type 0` | `exact` | `exact` (ℚ) |
| `RAPID` + `GOTO/…` | `travel: true`, `speed: 0` (§5.2: rapid carries no speed) | `velocity = -1` | `exact` | `exact` (ℚ); travel speed if any: `loss` under `RapidEncoding::MinusOne` |
| `GOTO/x,y,z,i,j,k` under `MULTAX/ON` | `orientation: Some(unit)`; `None` iff exactly `(0,0,1)` | `rz1, ry` from direction; `rz2 = spin_deg` | `exact` (normalised, ℚ² sum checked) | direction `exact` (ℝ); `approximate(5·10⁻⁷ deg)` at 6 decimals; spin: not an observation |
| `FEDRAT/MMPM,f` \| `FEDRAT/IPM,f` \| `FEDRAT/f` | `speed` (mm/min, modal until changed) | `velocity` (mm/s) | `exact` (ℚ) | `exact` (ℚ) |
| `MOVARC/cx,cy,cz,0,0,±1,r,θ` + `GOTO/ex,ey,ez[,i,j,k]` | `kind: Arc`, `centre: [cx,cy]`, `clockwise := nz<0`, `end`, `length = hypot(r·θ, Δz)` | midpoint target `type 1` then endpoint `type 0` (§5.4) | centre/end/direction `exact`; on-circle & sweep consistency `rejection(P_arc)`; `length` `approximate` (`FM1.F64.APT.ARC.LENGTH`) | `exact` (ℝ) for midpoint on circle; `approximate` at render; Δz ≠ 0 ⇒ `rejection` |
| `MOVARC` with normal ∉ {±Z} | — | — | `rejection` (v0; OD-3) | — |
| `CIRCLE/…` + linearised `GOTO`s (⚠F-18) | the `GOTO`s lift as `Line`s exactly as written; `CIRCLE` becomes an advisory | line targets | `exact` for the chords; arc-ness `observational` loss (advisory `apt-circle-annotation-ignored`; OD-9) | `exact` |
| `CYCLE/DRILL,…` … `GOTO/P` … `CYCLE/OFF` | 3–4 segments per hole (§5.5) | targets | `trace-exact` (expansion is a definition); cycle *structure* not recovered by the APT lower (`observational`) | `exact` |
| `CYCLE/<anything but DRILL>` | — | — | `rejection` | — |
| `DELAY/t` (⚠F-12) \| `DWELL/t` (⚠F-19) | `kind: Dwell`, `dwell_s: Some(t)`, `start == end`, `travel: true`, `length 0` (pattern `gcode/lift.rs:744-782`) | **none** (⚠F-10) | `exact` | `rejection` (default) or counted `loss` (`DwellPolicy::Drop`) |
| `LOADTL/n[,LENGTH,l,…]` | `tool: Some(n)` | `toolNumber` sparse entry `(idx, n)` | `n` `exact`; minor words `loss` (advisory) | `exact` |
| `SPINDL/RPM,n,CLW\|CCLW` (⚠F-12, F-20) | `power: Some(n)` | `spindleSpeed` sparse entry `(idx, n)` | `n` `exact`; CLW direction `loss` (advisory `apt-spindle-direction-dropped`); CCLW emitted to dialects without reverse spindle support: `rejection` (`[spindle-direction-unsupported]`) | `exact` (CLW); CCLW refused if reverse unsupported |
| `SPINDL/OFF` | `power: Some(0.0)` (commanded off, `ir.rs:105-112`) | `spindleSpeed` entry `(idx, 0)` | `exact` | `exact` |
| `SPINDL/ON` with a prior RPM | `power: Some(last)` | entry | `exact` | `exact` |
| `SPINDL/ON` with no prior RPM | — | — | `rejection` | — |

### 4.2 Non-motion statements and L2 kinds with no target

| Source construct | Class | Rationale |
|---|---|---|
| `PARTNO`, `PPRINT`, `MACHIN`, `FINI`, `COOLNT/*`, `LOADTL` minor words, `SPINDL` direction word, `CIRCLE` records | **inert-preserved** → `unmodeled_statements` / `advisories`, surfaced as rule `unmodeled-apt` (Warning) | none changes the geometric meaning of a following `GOTO` |
| `CUTCOM/LEFT\|RIGHT` (any state other than `OFF`) | **refused** `[cutcom-on]` | the controller would offset every following point by an unknown radius; carrying the points unshifted is silently wrong (ADR 0002 §4) |
| `INSERT/…` | **refused** `[insert-opaque]` | post-specific verbatim text; `manual_gcode` is defined as g-code (`docs/10-dry-ir-v0-spec.md:67-99`), so there is no channel that can carry it honestly (cf. KRL refusing `ManualGcode`, `crates/core/src/emit/krl.rs:455-469`) |
| `TRANS`, `MATRIX`, `ORIGIN`, `ROTABL`, `ROTHED`, `TRACUT`, `COPY`, `INDEX`, `MODE`, `CYCLE/TAP\|BORE\|REAM\|DEEP\|BRKCHP\|…` | **refused** `[major-word-hazardous]` / `[cycle-unsupported]` | each changes coordinate interpretation or expands to motion this spec does not define |
| any MAJOR word not in §6.2's closed lists | **refused** `[major-word-unknown]` by default; `Preserve` policy demotes to inert-preserved | an unknown word may be a transformation; refuse-never-guess (OD-7) |
| L2 `ManualGcode` | refused by both lowerings | as KRL |
| L2 `Spline` | flattened by `SplineFlatteningIterator` (`crates/core/src/emit/spline.rs:7`) before either lowering | inherited `approximate` contract, not restated here |
| L2 `Retract` / `Unretract` / `Deposit` | refused under `ExtrusionCarry::Refuse`; counted drop otherwise (`dropped_poseless`) | pose-less segments have no target (cf. `krl.rs:560-580`) |
| L2 `volume`, `filament`, `width`, `height`, `flow`, `temperature`, `fan` | not in `O_irbcam`; refused unless an `ExtrusionCarry` policy is explicit | §5.8, OD-1 |
| L2 `Meta` | lift stamps `Meta { generator: "dry apt importer", units: "mm", invariants: ["imported-from-apt"] }` (pattern `gcode/lift.rs:546-556`); lower writes `PPRINT/dry <version>` and `PARTNO` from `AptFrame` | `Meta` is not an observation |

---

## 5. Semantics — definitions

All definitions are over exact rationals ℚ unless marked ℝ; ℚ definitions are what the Lean modules in §8 formalise verbatim, ℝ definitions live in `noncomputable section`s with the binary64 refinement carried by the §8.4 boundary inventories, never inside the theorem (plan §3.1 rule "never silently models an IEEE-754 operation as exact real arithmetic"; `formal/Dry/Language/Common.lean:3-17` design note).

### 5.1 Units and feed normalisation

```
κ(UNITS/MM)      := 1
κ(UNITS/INCHES)  := 127/5                          -- 25.4 exactly in ℚ
scale_len(v)     := κ · v                          -- every coordinate, radius, RAPTO, depth, LOADTL length
feed(FEDRAT/MMPM,f)  := f                          -- mm/min, independent of UNITS
feed(FEDRAT/IPM,f)   := (127/5) · f                -- mm/min
feed(FEDRAT/f)       := κ · f                      -- per-minute in the program's UNITS
velocity_irbcam(speed_mm_min) := speed_mm_min / 60 -- mm/s, ℚ exact
```

Preconditions (else refuse, §6): `f` finite and `≥ 0` (a negative feed is refused exactly as `gcode/lift.rs:716-721`); a `FEDRAT` with any other unit word (`PERREV`, …) is refused `[feed-unit-unsupported]`. **Unit statement absent** ⇒ refuse `[unit-missing]` unless `AptImportParams::assume_units` is set; `UNITS/` after the first motion statement ⇒ refuse `[unit-late]` (a mid-program unit change would make earlier and later points incommensurable).

### 5.2 Rapid encoding

```
enc(travel : Bool, speed : ℚ) : ℚ := if travel then -1 else speed / 60       -- RapidEncoding::MinusOne
dec(w : ℚ) : Bool × ℚ            := if w = -1 then (true, 0) else (false, 60 · w)
```

Preconditions for `enc`: `¬travel ⇒ speed > 0` (a feed move whose L2 speed is `0`, i.e. "not stated by this file" per `gcode/lift.rs:733-739`, is refused `[irbcam-feed-unstated]` — there is no velocity to write and `-1` would turn a cut into a rapid). Theorem I1 (§8): `dec(enc(t, s)) = (t, if t then 0 else s)`. The travel speed of a travel move is therefore **not** recoverable under `MinusOne`; it is a declared loss (§7.4) and the reason `RapidEncoding::Explicit` exists (`enc' := speed/60` for travels with `speed > 0`, under which the *travel flag* becomes the loss instead — OD-4). For `RapidEncoding::Explicit`, every travel segment must carry an explicit `speed > 0`: if a travel move has `speed = 0` (unstated travel speed, as produced by APT `RAPID`), the emitter cannot guess a velocity and must refuse `[irbcam-travel-speed-unstated]`.

APT side: `RAPID` sets a one-shot flag consumed by the next `GOTO` (⚠F-13); the lifted segment has `travel := true, speed := 0`. `RAPID` immediately before `MOVARC` is refused `[rapid-arc]`. The APT lower writes `RAPID` before every `travel` segment's `GOTO` and `FEDRAT/MMPM,s` before a feed `GOTO`/`MOVARC` whenever `s` differs from the last written feed.

### 5.3 Direction vector ↔ ZYZ Euler (ℝ)

Let `d = (i, j, k)` be the L2 unit tool-direction (`None ⇒ (0,0,1)`, `crates/core/src/ir.rs:119-122`), and `σ ∈ {+1, −1}` the tool-Z sign convention (⚠F-4). Define `d' := σ·d` and

```
dir(rz1, ry, rz2) := Rz(rz1) · Ry(ry) · Rz(rz2) · e_z = (cos rz1 · sin ry,  sin rz1 · sin ry,  cos ry)
euler(d')         := (rz1, ry, rz2) where
                       ry  := arccos(k')                     ∈ [0, π]
                       rz1 := atan2(j', i')  if i'² + j'² ≠ 0 else 0
                       rz2 := spin                           -- IrbcamFrame::spin_deg in radians; the free sixth DOF
render_angle(θ, unit) := match unit with
                         | AngleUnit::Deg => 180·θ/π         -- render boundary: degrees
                         | AngleUnit::Rad => θ               -- render boundary: radians
```

The emitted angles are converted through `render_angle(·, frame.angle_unit)` before numeric formatting (default: `AngleUnit::Deg`).

Theorems (§8 I6, I7): `dir(rz1, ry, s) = dir(rz1, ry, 0)` for all `s` (spin-independence — the choice of `rz2` cannot change where the tool points), and `dir(euler(d')) = d'` for unit `d'` (round trip). The pole `i' = j' = 0` is canonicalised to `rz1 = 0`; a reader recovers `d'` exactly there too. `unit_orientation` (`crates/core/src/emit/kinematics.rs:91-101`) normalises a non-unit vector and refuses a zero/non-finite one; the IRBCAM renderer calls it, so `k' ∈ [−1, 1]` and `arccos` is total on its input.

Read-back (`lift_irbcam`, abstract): `d := σ · dir(rz1, ry, ·)`; `orientation := None` iff `d = (0,0,1)`.

### 5.4 Arcs

**APT → L2 (MOVARC, ⚠F-15).** Given running position `s = (sx, sy, sz)`, `MOVARC/cx,cy,cz,nx,ny,nz,r,θ` followed by `GOTO/ex,ey,ez[,i,j,k]`:

```
n̂ := normalise(nx, ny, nz)                 -- refuse zero/non-finite [arc-normal-degenerate]
require n̂ ∈ {(0,0,1), (0,0,−1)}             -- else refuse [arc-normal-unsupported] (OD-3)
clockwise := (n̂ = (0,0,−1))                 -- right-hand rule about +Z is CCW = G3 = clockwise=false (ir.rs:87-89)
require r > 0, 0 < θ < 360                  -- θ = 360 is a full turn: refuse [arc-full-turn] (unemittable: gcode.rs:526-532, rapid.rs:91-95, krl.rs:673-678)
require | |s_xy − c_xy|² − r² | ≤ τ_r ∧ | |e_xy − c_xy|² − r² | ≤ τ_r     -- on-circle, τ_r := arc_tolerance_rel · max(r², 1)  (ℚ, squared form)
require sweep(s_xy, e_xy, c_xy, clockwise) ≈ θ within τ_θ                  -- consistency of the redundant θ word (ℝ; τ_θ := 1e-6·max(θ,1) deg)
centre := (cx, cy); end := (ex, ey, ez); kind := Arc
length := hypot(r · θ_rad, ez − sz)                                        -- ℝ; boundary FM1.F64.APT.ARC.LENGTH
```

`cz` is checked finite and otherwise **not observed** (with `n̂ = ±Z` the projected geometry does not depend on it). A helical arc (`ez ≠ sz`) is *lifted* (L2 supports the Z rise, `crates/core/src/resolve.rs:79-86`; `verify`'s `arc-length` rule uses `hypot`, `crates/core/src/verify.rs:953-973`) and later **refused by the IRBCAM lowering** (below) — matching RAPID/KRL behaviour and leaving linearisation to OD-3.

**L2 → IRBCAM (midpoint, ⚠F-6).** For `Arc` with start `s`, end `e`, centre `c`, direction `clockwise`:

```
require e_z = s_z                                                -- else refuse [irbcam-arc-helical]
require (e_x, e_y) ≠ (s_x, s_y)                                  -- else refuse [irbcam-arc-full-turn]
ρ   := |s_xy − c_xy|;   require ρ > 0                            -- [irbcam-arc-zero-radius]
φ_s := atan2(s_y − c_y, s_x − c_x);  φ_e := atan2(e_y − c_y, e_x − c_x)
Δ   := signed_sweep(φ_s, φ_e, clockwise) ∈ (−2π, 0) ∪ (0, 2π)    -- as rapid.rs:36-51 / krl.rs signed_sweep
m   := (c_x + ρ·cos(φ_s + Δ/2),  c_y + ρ·sin(φ_s + Δ/2),  s_z)
targets := [ target(m, orientation, velocity, type 1),  target(e, orientation, velocity, type 0) ]
```

Theorem I8 (§8, ℝ): `|m − c| = ρ` and the arc from `s` through `m` to `e` with direction `clockwise` has sweep `Δ` (equivalently `m` is the unique point at half the signed sweep on the start radius); with `Δ ∉ {0, ±2π}` the three points are distinct, so the circle IRBCAM fits through them is the circle the IR described **up to the radius consistency of `e`**, which is `verify`'s rule (`ARC_RADIUS_TOLERANCE_MM`, `crates/core/src/verify.rs:679`), not emit's — the same boundary `docs/22-krl-emit.md:260-266` records for KRL.

**L2 → APT.** `MOVARC/c_x, c_y, s_z, 0, 0, (if clockwise then −1 else 1), ρ, deg(|Δ|)` then `GOTO/e[,d]`. Helical L2 arcs *are* emittable to APT (`e_z ≠ s_z` written on the `GOTO`).

**CIRCLE (CATIA/NX, ⚠F-18).** The `CIRCLE` record is recorded as advisory `apt-circle-annotation-ignored { source_line, centre, radius }`; the following `GOTO`s lift as `Line`s exactly. This is not chording — the chords are what the source wrote. Reconstructing an `Arc` from on-circle points is an inference and is OD-9.

### 5.5 Cycle expansion (DRILL only, ⚠F-16)

State: `cycle : Option { depth : ℚ, feed : ℚ (mm/min after §5.1), rapto : ℚ, dwell : Option ℚ }`. `CYCLE/DRILL, depth, MMPM|IPM, f, RAPTO, r [, DWELL, t]` sets it (any other minor word ⇒ refuse `[cycle-minor-word]`; `depth ≤ 0`, `f ≤ 0`, `r < 0`, `t < 0` ⇒ refuse `[cycle-domain]`); `CYCLE/OFF` clears it; `CYCLE/OFF` while inactive ⇒ refuse `[cycle-off-inactive]`; `RAPID`, `MOVARC`, `FEDRAT` while active ⇒ refuse `[cycle-statement-inside]`. While active, `GOTO/P[,a]` with tool axis `a` (§5.6; `(0,0,1)` under `MULTAX/OFF`) expands to, in order:

```
s1 := Line  cur → P + r·a        travel, speed 0            -- rapid to the RAPTO point
s2 := Line  P + r·a → P − depth·a   feed at f                 -- plunge along −a
s3 := Dwell t at P − depth·a     (only if dwell = some t)
s4 := Line  P − depth·a → P + r·a   travel, speed 0            -- retract
cur := P + r·a
```

All arithmetic is ℚ-exact for rational `a` (⚠: `a` is decimal text, hence rational). Theorem A3 (§8, `trace-exact`): the expansion is a deterministic function of `(cycle, P, a)`, produces exactly 3 or 4 segments, is order-preserving across holes, and `CYCLE/OFF` restores plain `GOTO` semantics.

**Cycle entry plane clearance check:** The entry position `cur` before any cycle hole must lie at or above the clearance plane defined by the hole surface position `P` and clearance offset `r` along the tool axis `a`, satisfying `(cur − (P + r·a)) · a ≥ −ε` (for vertical `a = (0,0,1)`: `cur_z ≥ P_z + r − ε`). If `cur` violates this clearance condition (i.e. starts below the `RAPTO` clearance plane), the importer refuses `[cycle-clearance-violation]` rather than executing a rapid `s1` traversal through part geometry or fixtures. `s1` is a straight rapid move; this spec does not synthesise dog-leg clearance moves it was not given.

### 5.6 MULTAX state machine (⚠F-14)

```
state.multax : Bool := false;  state.axis : Vec3 := (0,0,1)
MULTAX | MULTAX/ON  ⇒ multax := true
MULTAX/OFF          ⇒ multax := false;  axis := (0,0,1)
GOTO with 3 values  ⇒ axis unchanged
GOTO with 6 values  ⇒ require multax           -- else refuse [multax-inconsistent]
                       axis := normalise(i,j,k)  -- zero/non-finite ⇒ refuse [tlaxis-degenerate]
GOTO with any other value count ⇒ refuse [goto-arity]  (multi-point GOTO lists are ambiguous under MULTAX and unsupported)
orientation(segment) := if axis = (0,0,1) then None else Some(axis)
```

`orientation := None` iff the axis is exactly `+Z`, regardless of the MULTAX flag, so a `MULTAX/ON` file whose tools are all vertical lifts byte-identically to a 3-axis file (the convention the kernel map recommends and the g-code lifter's `None` already means, `gcode/lift.rs:633-701`). The MULTAX flag itself is not an observation (§7.1). The APT lower writes `MULTAX/ON` once, before the first motion, iff any segment has `Some(orientation)`, and then six values on every `GOTO` (`(0,0,1)` for `None`); otherwise no `MULTAX` and three values.

### 5.7 Tool and spindle sparse vectors (⚠F-7, F-8, F-12, F-20)

L2 carries `tool : Option u32` and `power : Option f64` per segment; the lifter sets them modally (`LOADTL/n` ⇒ `tool := n`, `n` must be a non-negative integer literal else `[loadtl-domain]`; `SPINDL` per §4.1, `n` finite and `≥ 0` else `[spindle-domain]`). Lowering to IRBCAM:

```
compress(v : List (Option α)) : List (ℕ × α) := [(i, a) | i < |v|, v[i] = some a, (i = 0 ∨ v[i−1] ≠ some a)]
require ∀ i > 0, v[i−1] = some _ → v[i] ≠ none          -- Some→None is unrepresentable: refuse [irbcam-channel-uncommanded]
expand(n, entries) : List (Option α)                     -- modal replay, none before the first entry
```

Theorem I2 (§8, `exact`): `expand(|v|, compress(v)) = v` for every `v` satisfying the precondition, and `compress` yields strictly increasing indices `< |v|` (the well-formedness I3 needs). With `ToolToggle`, `tool` is *replaced* (not merged) by `if travel then travel_tool else extrude_tool` before compression; with `SpindleRate`, `power` is replaced by `volume · speed / (60 · length)` (mm³/s, ℚ; requires `length > 0 ∧ speed > 0` on extruding segments else `[irbcam-rate-undefined]`). If `ToolToggle` is applied to a stream that already commands `tool: Some(_)`, or if `SpindleRate` is applied to a stream that already commands `power: Some(_)`, attempting to overwrite the existing channel refuses `[irbcam-channel-collision]` rather than silently clobbering upstream channel data (ADR 0002 §4).

### 5.8 Extrusion channels (Flow B)

`ExtrusionCarry::Refuse` (default): any segment with `volume ≠ 0 ∨ filament ≠ 0 ∨ width ∨ height ∨ flow ∨ temperature ∨ fan ∨ kind ∈ {Retract, Unretract, Deposit}` refuses `[irbcam-extrusion-unrepresentable]`. `MotionOnly`: those fields are ignored, pose-less kinds produce no target and are counted (`dropped_poseless`), and `extrusion_segments_lowered_motion_only` counts the rest. `ToolToggle`/`SpindleRate`: §5.7; if the stream already defines `tool` or `power` respectively, the channel conflict refuses `[irbcam-channel-collision]`. Under every policy the fields are outside `O_irbcam` (§7.2) — the policy chooses *which* declared loss, never whether there is one (OD-1).

### 5.9 Dwell (⚠F-10, F-12, F-19)

Lift: `DELAY/t` and `DWELL/t` ⇒ `Dwell` segment (`dwell_s := t`, `t` finite `≥ 0`; `t = 0` ⇒ no segment, as `gcode/lift.rs:745-748` lifts a duration-less `G4` to nothing); the APT lower writes `DELAY/t`. IRBCAM: `DwellPolicy::Refuse` (default) ⇒ `[irbcam-dwell-unrepresentable]`; `Drop` ⇒ no target, `dropped_dwells += 1`. No encoding of a dwell as a zero-length move is defined — a zero-length target has no duration, so it would be a fabricated observation.

### 5.10 Number rendering (⚠F-9)

`irb_num(v, p) := format!("{v:.p$}")` after `num_checked`-style finiteness refusal (`crates/core/src/emit/gcode.rs:199-209`), **without** the trailing-zero trimming `num()` does (`:183-191`), so every field carries exactly `p ≥ 6` decimals. `p = 6` makes the render `approximate(ε = 5·10⁻⁷)` in each field's unit (mm, deg, mm/s); `p = 17` makes it injective on finite binary64 and hence `bit-exact` recoverable. CSV uses the same function per cell. APT numbers use the same rule with `AptFrame::decimals`. This is the boundary `FM1.F64.IRBCAM.RENDER.DECIMALS`; the abstract models (§8) exclude text rendering exactly as `CodecInverse` excludes JSON syntax (`formal/Dry/Semantics/CodecInverse.lean:4-14`).

---

## 6. Refusal predicates (ADR 0002 §4: refuse; never clamp, chord, or drop silently)

### 6.1 Numeric ingress (H1 discipline)

- **R-NUM-1** Every numeric token is parsed with the finite gate of `gcode.rs:575-590`: non-finite ⇒ refuse `[non-finite]` on every statement, modeled or inert. `1e400`, `nan`, `inf` never enter the IR.
- **R-NUM-2** Every *computed* quantity — scaled coordinate, `P ± d·a`, arc centre, arc length, hypot — is built through `Length::try_mm` (`crates/core/src/units.rs:129-135`) via a `checked_mm` wrapper naming source line and field (`gcode/lift.rs:939-941`); `Length::mm` (`:122`) is never called on lifted data. Necessary because κ = 25.4 and squared deltas overflow finite inputs (`units.rs:109-120` doc).
- **R-NUM-3** Emission: every rendered field passes the finiteness check before formatting (`num_checked`, `gcode.rs:199-209`; RAPID's per-value loop `rapid.rs:70-85` is the precedent for a non-`num()` formatter).

### 6.2 MAJOR-word partition (closed; every APT statement lands in exactly one class)

- **Modeled:** `UNITS`, `MULTAX`, `FEDRAT`, `RAPID`, `GOTO`, `MOVARC`, `CIRCLE` (advisory + chords), `CYCLE` (`DRILL`/`OFF`), `DELAY`, `DWELL`, `LOADTL`, `SPINDL`.
- **Inert-preserved (class I):** `PARTNO`, `PPRINT`, `MACHIN`, `FINI`, `COOLNT`, `CLRSRF`, `REMARK`, `UNITS` duplicates with the same value.
- **Refused-hazardous (class H):** `CUTCOM` (other than `OFF`), `INSERT`, `TRANS`, `MATRIX`, `ORIGIN`, `ROTABL`, `ROTHED`, `TRACUT`, `COPY`, `INDEX`, `MODE`, `CYCLE/<not DRILL|OFF>`, `FEDRAT/PERREV`, `SPINDL` with a non-numeric speed, `GOTO` with a value count ∉ {3, 6}.
- **Unknown:** anything else ⇒ `[major-word-unknown]` under `UnknownMajorWordPolicy::Refuse` (default); `Preserve` moves it to class I with a finding (OD-7).

Refusal soundness (theorem A4): for every input satisfying any predicate `P` in class H, unknown-under-Refuse, R-NUM-1/2 or §5's `require` lines, `lift` returns `Err` — `rejects(P)` in ADR 0001's sense. The classes are exhaustive by construction (the `match` on the major word has no wildcard that lowers to motion).

### 6.3 Geometry

`[arc-normal-unsupported]`, `[arc-normal-degenerate]`, `[arc-full-turn]`, `[arc-off-circle]`, `[arc-sweep-mismatch]`, `[rapid-arc]`, `[tlaxis-degenerate]`, `[multax-inconsistent]`, `[goto-arity]` — defined in §5.4/§5.6. Rationale for refusing rather than linearising non-`±Z` normals: L2 `centre` is XY-only (`ir.rs:84-86`), so any other plane would have to be *chorded*, and chording changes the observable trace; a tolerance-bearing linearisation is an `approximate(ε)` claim that needs its own boundary inventory — deferred to OD-3 rather than smuggled in.

### 6.4 Cycles

`[cycle-unsupported]` (TAP, BORE, REAM, DEEP/peck, BRKCHP, …), `[cycle-minor-word]`, `[cycle-domain]`, `[cycle-off-inactive]`, `[cycle-statement-inside]`, `[cycle-clearance-violation]` — §5.5. Tapping and boring encode spindle/feed synchronisation and dwell-at-bottom retract behaviour that no L2 kind expresses; expanding them would fabricate motion. Entry position below the `RAPTO` clearance plane refuses `[cycle-clearance-violation]` to prevent rapid traversal collisions through stock.

### 6.5 Emission (IRBCAM and APT)

Validated **before the first byte** (E4 precedent `gcode.rs:293-299`, `rapid.rs:124-129`): `IrbcamFrame::validate` (`spin_deg` finite; `angle_unit` valid discriminant; `decimals ∈ 6..=17`; `ToolToggle` tools distinct; `cnc_frame` present ⇒ refuse as RAPID does at `rapid.rs:124`). Per-segment: `[irbcam-feed-unstated]`, `[irbcam-travel-speed-unstated]` (under `RapidEncoding::Explicit` when a travel segment has `speed = 0`), `[irbcam-channel-collision]` (under `ToolToggle`/`SpindleRate` if `tool`/`power` is already populated), `[spindle-direction-unsupported]` (when CCLW is emitted to dialects without reverse spindle support), `[irbcam-arc-helical]`, `[irbcam-arc-full-turn]`, `[irbcam-arc-zero-radius]`, `[irbcam-dwell-unrepresentable]`, `[irbcam-extrusion-unrepresentable]`, `[irbcam-channel-uncommanded]`, `[irbcam-rate-undefined]`, `[irbcam-manual-gcode]`, `[irbcam-power-domain]` (finite `≥ 0`, checked on every flavor first, `gcode.rs:407-420`), `[irbcam-orientation-degenerate]` (from `unit_orientation`), arc without explicit end X and Y (`gcode.rs:526-532`). The APT emitter shares all but the dwell/helical ones and adds `[apt-manual-gcode]`, `[apt-poseless]`.

Whole-module rule (E5): `emit_normalized_span_lines` refuses `RobotKrl` (`gcode/lift.rs:278-285`); the slice extends that guard to `Rapid`, `Irbcam`, `IrbcamCsv`, `Apt` — an IRBCAM JSON document or an APT program with a `PARTNO`/`FINI` frame cannot be spliced span-by-span into a g-code file.

### 6.6 Resource limits (H1 hardening)

`AptImportLimits::default()`: `max_source_bytes = 256 MiB`, `max_statements = 8·10⁶`, `max_line_bytes = 4096`, `max_continuation_lines = 64`, `max_values_per_statement = 64`, `max_cycle_holes = 10⁶`. Exceeding any ⇒ `[limit-exceeded]` naming field, limit and actual (the `CodecError::LimitExceeded` shape, `crates/core/src/codec/error.rs:22-27`; `DecodeLimits` precedent `crates/core/src/codec/mod.rs:102`). Limits are checked while streaming, so a hostile file is refused before it is materialised.

---

## 7. Observation sets and the round-trip claim

### 7.1 `O_apt` — what `dry import-apt` recovers from an APT program (and what `--format apt` must write)

For each motion segment in order: `kind ∈ {Line, Arc}`; `start`/`end` (mm, per axis, `None` inheritance); `travel`; `speed` for feed segments (mm/min); for arcs `centre` (XY) and `clockwise`; `orientation` as a direction (`None ≡ +Z`); `tool` and `power` as per-segment modal values; each `Dwell` with `dwell_s`. **Not in `O_apt`:** `length`, `volume`, `filament`, `width`, `height`, `flow`, `temperature`, `fan` (recomputed or absent); the *placement* of `RAPID`/`FEDRAT`/`MULTAX`/`CYCLE` statements (only their per-segment effect); `MOVARC`'s `cz` and the redundant `θ`; `CIRCLE` annotations; every class-I statement's text; spindle direction; `LOADTL` minor words; number spelling.

### 7.2 `O_irbcam` — what a reader of the target list recovers

For each target in order: position (mm); tool direction (from `rz1, ry`, `None ≡ +Z` after `σ`); rapid-vs-feed (`velocity = -1`); velocity (mm/s) for feed targets; arc as (midpoint `type 1`, endpoint `type 0`) triples with the previous target; `toolNumber` and `spindleSpeed` as modal values by index. **Not in `O_irbcam`:** `rz2` (constant, §5.3); travel speed (under `MinusOne`); dwells; every extrusion field; `manual_gcode`; spline control points (flattened); L2 `length`; `Meta`.

### 7.3 The round-trip claims

- **A7 (APT):** for every `x ∈ L2_apt` — well-formed (`Dry.Language.L2.Toolpath.WellFormed`, `formal/Dry/Language/WellFormed.lean:90-99`), kinds ⊆ {Line, Arc, Dwell}, no extrusion fields, `speed > 0` on feed segments, explicit end X/Y on arcs — `lift_apt(lower_apt(x)) ≡obs(O_apt) x`. Relation `observational`, **never `exact`**: `length` is recomputed (irrational for arcs), `Meta` differs, the statement placement is normalised.
- **I5 (IRBCAM):** for every `x ∈ L2_irbcam` (additionally: no dwells unless `Drop`, no helical arcs, `tool`/`power` never `Some → None`), `lift_irbcam(lower_irbcam(x)) ≡obs(O_irbcam) x`, where `lift_irbcam` is the **abstract read-back defined in Lean** (§8, `Dry.Semantics.IrbcamLower.liftIrbcam`), not a shipped Rust importer. It is the specification of "declared recoverable observations" plan §6.6 asks for (`docs/21-mathematical-assurance-plan.md:383`); the QML read-back plugin (§13) would be its Layer-D instrument.

### 7.4 Declared losses (why no claim above is `exact`)

| Loss | Where | Why unavoidable | How it is made loud |
|---|---|---|---|
| Dwell | L2 → IRBCAM | no IRBCAM target (⚠F-10) | refused by default; `dropped_dwells` under `Drop` |
| Spindle direction (`CLW`/`CCLW`) | APT → L2 / Emission | `power` is a scalar (`ir.rs:105-112`); dialects without reverse spindle support cannot run CCLW safely (`gcode.rs:399-407` comment) | CLW direction dropped with advisory `apt-spindle-direction-dropped`; CCLW emitted to dialects without reverse spindle support is refused `[spindle-direction-unsupported]` |
| `LOADTL` body length and other minor words | APT → L2 | L2 `tool` is an index; tool geometry is a profile concern | advisory `apt-loadtl-minor-words-dropped` |
| Extrusion channels (`volume`, `filament`, `width`, `height`, `flow`, `temperature`, `fan`, pose-less kinds) | L2 → IRBCAM | robot machining target has no material axis | refused by default; policy + counts (OD-1); channel collisions refuse `[irbcam-channel-collision]` |
| Travel speed | L2 → IRBCAM (`MinusOne`) | `-1` is the only rapid encoding (⚠F-5) | documented; `Explicit` swaps it for the travel flag (OD-4), refusing unstated travel speeds with `[irbcam-travel-speed-unstated]` |
| `rz2` | L2 → IRBCAM | L2 has no roll | constant, spin-independence theorem I6; units parameterized via `AngleUnit` |
| `CYCLE`/`MULTAX`/`RAPID`/`FEDRAT` statement structure | APT round trip | L2 is the resolved trace | trace-exact expansion (A3), observational round trip (A7) |
| `CIRCLE` arc-ness | APT (CATIA/NX) → L2 | source is already chorded | advisory (OD-9) |

---

## 8. Formal assurance plan — theorem inventory

### 8.1 Layer placement rules applied here

- Abstract (Layer A): ℚ via `Dry.Language.Number` (`formal/Dry/Language/Common.lean:14-17`) for everything algebraic; ℝ in `noncomputable section` only for Euler angles and arc midpoints (as `Dry.Geometry.PlanarTransform`, `Dry.Numeric.Orientation`).
- Numeric (Layer B): every ℝ theorem and every f64 arithmetic step is listed in a boundary inventory (§8.4) with a classification and status; `numeric = pending` until bounded — no ℝ result is presented as an f64 result (`docs/21-mathematical-assurance-plan.md:842-858`).
- Implementation (Layer C): Lean-generated fixtures via `native_decide` + `main : IO Unit` JSON render (`formal/Dry/Tests/OrientationContractFixtures.lean:102, 152`), snapshotted under `proofs/fixtures/`, consumed by `crates/core/tests/*_refinement.rs`, checked by `tools/check_proof_fixtures.py`; a claim is `scope = "implementation"` only when abstract proved ∧ numeric ∈ {bounded, not-applicable} ∧ refinement checked (`docs/adr/0001-formal-assurance-constitution.md:63-67`).
- Physical (Layer D): §10.4; never a claim.

Registry mechanics: each claim below is a `[[claim]]` in `proofs/claims.toml` (schema `proofs/claims.schema.json`; theorem/lean_source/proof_method present iff `abstract = "proved"`, `:74-97`), linked to exactly one `[[clause]]` in `proofs/spec-claim-links.toml` (validator `tools/validate_spec_claim_links.py:84-115`), rendered into `docs/assurance/01-assurance-sitemap.md` by `tools/generate_assurance_report.py --check` (CI `formal-assurance`, `.github/workflows/ci.yml:479`). Until the Lean lands, a claim is registered with `abstract = "specified"` and **no** theorem field.

### 8.2 New Lean modules (module path = namespace; imported from `formal/Dry.lean`, tests not)

| Module | Namespace | Domain | Contents |
|---|---|---|---|
| `formal/Dry/Language/Apt.lean` | `Dry.Language.Apt` | ℚ | `Units`, `Statement` inductive (§6.2 classes as constructors), `Program := List Statement`; `deriving DecidableEq, Repr` |
| `formal/Dry/Language/Irbcam.lean` | `Dry.Language.Irbcam` | ℚ | `Target {x y z rz1 ry rz2 velocity : Number, kind : TargetKind}`, `Program {targets, toolNumber : List (Nat × Nat), spindleSpeed : List (Nat × Number)}`, `WellFormed` (indices strictly increasing & `< targets.length`; `kind ∈ {linear, midpoint}`; every `midpoint` immediately followed by a `linear`; last target not `midpoint`; all numbers finite; `velocity = -1 ∨ velocity > 0`), `validate : Except Failure Unit`, `validate_success_iff`, `invalid_rejected`, `Evaluates.deterministic` (shape of `WellFormed.lean:209-238`) |
| `formal/Dry/Semantics/AptLift.lean` | `Dry.Semantics.AptLift` | ℚ (+ abstract `arcLength` parameter) | `State`, `step`, `lift : AptImportParams → Program → Except Refusal L2.Toolpath`, `expandCycle`, `scale`, `normaliseFeed` |
| `formal/Dry/Semantics/AptLower.lean` | `Dry.Semantics.AptLower` | ℚ | `lower : L2.Toolpath → Except Refusal Apt.Program`, `Obs` projection for `O_apt` |
| `formal/Dry/Semantics/IrbcamLower.lean` | `Dry.Semantics.IrbcamLower` | ℚ (+ abstract `midpoint` parameter) | `encodeVelocity/decodeVelocity`, `compress/expand`, `lower : IrbcamFrame → L2.Toolpath → Except Refusal Irbcam.Program`, `liftIrbcam`, `Obs` projection for `O_irbcam` |
| `formal/Dry/Geometry/ZyzEuler.lean` | `Dry.Geometry.ZyzEuler` | ℝ, noncomputable | `direction`, `euler`, `Rz`, `Ry` |
| `formal/Dry/Geometry/ArcMidpoint.lean` | `Dry.Geometry.ArcMidpoint` | ℝ, noncomputable | `signedSweep`, `midpoint` (also models `rapid.rs:58` / `krl.rs:655`, which have no Lean model today) |
| `formal/Dry/Tests/AptLiftFixtures.lean`, `formal/Dry/Tests/IrbcamLowerFixtures.lean` | `Dry.Tests.*` | ℚ | fixtures, `modelChecks`, `theorem …Checks : modelChecks = true := by native_decide`, JSON `main` |

Every module header states what it does **not** model (text syntax, binary64, Rust, IRBCAM/robot behaviour), per the convention at `formal/Dry/Semantics/CodecInverse.lean:4-14`.

### 8.3 Theorem inventory

| # | Claim id | Informal statement | Relation | Domain | Lean theorem (module) | Numeric | Implementation refinement (Layer C) |
|---|---|---|---|---|---|---|---|
| A1 | `FM1.APT.UNITS.NORMALIZATION` | `scale κ` and `normaliseFeed` are the exact ℚ maps of §5.1; `UNITS/INCHES` scales by exactly 127/5; `MMPM` feed is unit-independent; `velocity = speed/60` | `exact` | ℚ | `AptLift.units_scale_exact` | `pending` (`FM1.F64.APT.UNITS.INCH_SCALE`, `FM1.F64.IRBCAM.VELOCITY.DIV60`) | fixture cases: mm/inch × MMPM/IPM/bare; `crates/core/tests/apt_lift_refinement.rs` compares lifted `speed`/coordinates to Lean rationals converted by an independent adapter |
| A2 | `FM1.APT.MULTAX.STATE` | the axis/orientation of every lifted segment is the §5.6 function of the statement prefix; bare `MULTAX` = `ON`; `+Z` lifts to `None` | `trace-exact` | ℚ | `AptLift.multax_axis_deterministic` | `pending` (`FM1.F64.APT.TLAXIS.NORMALISE`) | fixtures incl. bare `MULTAX`, `OFF` reset, 3-value `GOTO` after 6-value |
| A3 | `FM1.APT.CYCLE.EXPANSION` | `expandCycle` yields exactly the 3/4 §5.5 segments per hole, order-preserving, `CYCLE/OFF` restores `GOTO` | `trace-exact` | ℚ | `AptLift.cycle_expansion_trace` | `pending` (`FM1.F64.APT.CYCLE.AXIS_OFFSET`) | fixtures: 1 and 3 holes, with/without `DWELL`, tilted axis |
| A4 | `FM1.APT.LIFT.REJECTION` | for every program satisfying a §6 predicate `P` (including `cycle-clearance-violation`), `lift = .error _`; for every program in the modeled subset with no predicate satisfied, `lift = .ok _` | `rejection` | ℚ + `nonFinite` | `AptLift.lift_rejects_iff` | `not-applicable` | negative fixtures per code (`[cutcom-on]`, `[multax-inconsistent]`, `[arc-normal-unsupported]`, …); Rust test asserts the same `AptErrorCode` |
| A5 | `FM1.APT.LIFT.DETERMINISTIC` | `Evaluates` is functional | `exact` | ℚ | `AptLift.Evaluates.deterministic` | `not-applicable` | repeated-observation check in the refinement test (pattern `feature_refinement.rs`) |
| A6 | `FM1.APT.MOVARC.STRUCTURE` | for an accepted `MOVARC`+`GOTO`, the lifted `Arc` has `centre = (cx, cy)`, `clockwise = (nz < 0)`, `end = GOTO`; acceptance implies the on-circle predicate with tolerance τ | `exact` (structure) with `rejection` sub-lemma | ℚ (squared distances) | `AptLift.movarc_structure` | `pending` (`FM1.F64.APT.ARC.ENDPOINT_RADIUS`, `FM1.F64.APT.ARC.SWEEP_CHECK`, `FM1.F64.APT.ARC.LENGTH`) | fixtures: CW/CCW quarter arcs, off-circle refusal, full-turn refusal |
| A7 | `FM1.APT.ROUNDTRIP.OBSERVATIONAL` | `∀ x ∈ L2_apt, Obs(lift(lower x)) = Obs(x)` | `observational` (`O_apt`) | ℚ | `AptLower.lift_lower_obs` | `not-applicable` (text excluded) | `crates/core/tests/apt_roundtrip.rs`: emit `apt` → import → project `O_apt` → compare, over the IR vector corpus and §10 goldens; `decimals = 17` for the exactness leg |
| A8 | `FM1.APT.LIFT.NATIVE.CORPUS` | selected Lean-generated APT programs lift in Rust to the Lean-predicted `O_apt` projection / error code | `observational` | ℚ observed via f64 | `Tests.AptLiftFixtures.aptLiftFixtureChecks` (`native_decide`) | `not-applicable` | `scope = implementation`; `proofs/fixtures/apt-lift-refinement-v0.json` + schema; `tools/check_proof_fixtures.py` entry |
| I1 | `FM1.IRBCAM.RAPID.ENCODING` | `decodeVelocity (encodeVelocity t s) = (t, if t then 0 else s)` for `¬t → s > 0`; `Explicit` requires `s > 0` on travels else `[irbcam-travel-speed-unstated]` | `exact` | ℚ | `IrbcamLower.decode_encode_velocity` | `pending` (`FM1.F64.IRBCAM.VELOCITY.DIV60`) | fixtures: rapid, feed, `Explicit` policy |
| I2 | `FM1.IRBCAM.SPARSE.CHANNELS` | `expand n (compress v) = v`; `compress` indices strictly increasing and `< n` | `exact` | ℚ / ℕ | `IrbcamLower.expand_compress_sparse` | `not-applicable` | fixtures incl. `None` prefix, change at index 0, `Some→None` refusal |
| I3a | `FM1.IRBCAM.WELL_FORMED.REJECTION` | `validate p = .ok () ↔ p.WellFormed` | `rejection` | ℚ | `Irbcam.Validation.validate_success_iff` | `not-applicable` | `tools/validate_irbcam_export.py` (dry-core-free, §10.3) re-implements the predicate over the emitted JSON |
| I3b | `FM1.IRBCAM.EXPORT.WELL_FORMED` | `x.WellFormed → (lower f x = .ok p) → p.WellFormed` (sparse indices in range & increasing, `kind ∈ {0,1}`, midpoint always followed by endpoint, velocities `-1` or `> 0`) | `invariant-preservation` (`I` = IRBCAM well-formedness) | ℚ | `IrbcamLower.lower_wellFormed` | `not-applicable` | every golden and fixture passes `validate_irbcam_export.py` |
| I4 | `FM1.IRBCAM.LOWER.REJECTION` | `lower` errors exactly on §6.5's predicates (dwell under `Refuse`, unstated feed, unstated travel speed under `Explicit`, channel collision under `ToolToggle`/`SpindleRate`, spindle direction unsupported for CCLW, helical/full-turn/zero-radius arc, `Some→None` channel, extrusion under `Refuse`, manual g-code) | `rejection` | ℚ | `IrbcamLower.lower_rejects_iff` | `not-applicable` | Rust `irbcam_program_structure.rs` refusal tests with matching codes |
| I5 | `FM1.IRBCAM.LOWER.OBSERVATIONAL` | `∀ x ∈ L2_irbcam, Obs(liftIrbcam(lower f x)) = Obs(x)` | `observational` (`O_irbcam`) | ℚ (midpoint abstract) | `IrbcamLower.lift_lower_obs` | `not-applicable` | fixtures: Rust emits JSON for Lean seeds; test projects `O_irbcam` from the JSON (positions, `-1`, sparse replay) and compares to Lean |
| I6 | `FM1.IRBCAM.ZYZ.SPIN_INDEPENDENCE` | `direction rz1 ry s = direction rz1 ry 0` | `exact` | ℝ | `ZyzEuler.direction_spin_independent` | `not-applicable` (no arithmetic depends on `rz2`) | Rust test: two emits differing only in `spin_deg` differ only in the `rz2` column |
| I7 | `FM1.IRBCAM.ZYZ.DIRECTION_ROUNDTRIP` | unit `d → direction (euler d) = d`; pole canonical `rz1 = 0` | `exact` | ℝ | `ZyzEuler.direction_euler_roundtrip` | `pending` (`FM1.F64.IRBCAM.EULER.ACOS`, `.ATAN2`, `.DEGREES`) | `crates/core/tests/irbcam_lower_refinement.rs`: for the six-vector orientation corpus (`proofs/fixtures/orientation-contract-refinement-v0.json`) reconstruct `d` from rendered angles within a stated tolerance — evidence, not a theorem |
| I8 | `FM1.IRBCAM.ARC.MIDPOINT` | `‖midpoint − c‖ = ρ` and `midpoint` is at half the signed sweep; three points distinct when `Δ ∉ {0, ±2π}` | `exact` | ℝ | `ArcMidpoint.midpoint_on_circle_and_sweep` | `pending` (`FM1.F64.IRBCAM.ARC.MIDPOINT.TRIG`) | Rust test: `|m − c| − ρ` and sweep reconstruction over the arc vector `arc_g2_g3` and goldens; also pins `rapid.rs:58`/`krl.rs:655` retroactively |
| I9 | `FM1.IRBCAM.LOWER.NATIVE.CORPUS` | selected Lean seeds lower in Rust to the Lean-predicted target list on `O_irbcam` (17-decimal render) | `observational` | ℚ observed via f64 | `Tests.IrbcamLowerFixtures.irbcamLowerFixtureChecks` (`native_decide`) | `not-applicable` | `scope = implementation`; `proofs/fixtures/irbcam-lower-refinement-v0.json` + schema |

Normative clauses (`proofs/spec-claim-links.toml`, `source = "docs/28-apt-irbcam-dialects.md"`): `DRY.APT.LIFT_V0` (§5.1–5.7, §6: A1–A6, A8), `DRY.APT.ROUNDTRIP_V0` (§7.3: A7), `DRY.IRBCAM.EXPORT_V0` (§5.2, 5.7–5.10, §6.5: I1–I5, I9), `DRY.GEOMETRY.ZYZ_EULER_V0` (§5.3: I6, I7), `DRY.IRBCAM.ARC_MIDPOINT_V0` (§5.4: I8). One link per claim.

### 8.4 Numeric boundary inventories (Layer B)

Two new inventories, registered in `tools/validate_numeric_boundaries.py` `MODELS` (`:240`) with reciprocal `claim_ids` ↔ `numeric_boundaries` links (schema `proofs/numeric-boundaries.schema.json`, classification enum `:165-170`):

- `proofs/apt-numeric-boundaries-v0.toml` (`model = "apt-numeric-boundaries-v0"`, profile `FM1.NUMERIC.PROFILE.APT.INGRESS.V0`, sources pinned by sha256: `crates/core/src/apt/lift.rs`, `crates/core/src/apt.rs`): `FM1.F64.APT.WORD.PARSE` (`rejected`), `FM1.F64.APT.UNITS.INCH_SCALE` (`interval-bounded`: one correctly-rounded multiply), `FM1.F64.APT.TLAXIS.NORMALISE` (`interval-bound-pending`), `FM1.F64.APT.ARC.ENDPOINT_RADIUS` (`rejected` with tolerance policy), `FM1.F64.APT.ARC.SWEEP_CHECK` (`rejected`, libm `atan2`), `FM1.F64.APT.ARC.LENGTH` (`interval-bound-pending`, libm `hypot`), `FM1.F64.APT.CYCLE.AXIS_OFFSET` (`interval-bounded`).
- `proofs/irbcam-numeric-boundaries-v0.toml` (`model = "irbcam-numeric-boundaries-v0"`, profile `FM1.NUMERIC.PROFILE.IRBCAM.EXPORT.V0`, source `crates/core/src/emit/irbcam.rs`): `FM1.F64.IRBCAM.EULER.ACOS`, `FM1.F64.IRBCAM.EULER.ATAN2` (`interval-bound-pending`, libm 0.2.16 contract as `proofs/libm-0.2.16-trig-contract.md`), `FM1.F64.IRBCAM.EULER.DEGREES` (`interval-bounded`), `FM1.F64.IRBCAM.ARC.MIDPOINT.TRIG` (`interval-bound-pending`), `FM1.F64.IRBCAM.VELOCITY.DIV60` (`interval-bounded`), `FM1.F64.IRBCAM.RENDER.DECIMALS` (`interval-bounded`, ε = 5·10⁻⁽ᵖ⁺¹⁾), `FM1.F64.IRBCAM.SPINDLE_RATE` (`interval-bound-pending`, only under `SpindleRate`).

Exit condition for promoting I7/I8 to `numeric = bounded`: a `Dry.Numeric.*` composition through `RoundModel.Approx` (`formal/Dry/Numeric/RoundModel.lean:20`) for `acos`/`atan2`/`deg` — not in this slice; until then the claims read "proved over ℝ, binary64 pending", which ADR 0001 permits with that wording (`docs/adr/0001-formal-assurance-constitution.md:69-70`).

### 8.5 Mutation gate

`tools/check_apt_irbcam_mutations.py` + `proofs/apt-irbcam-mutations-v0.toml` (pattern `check_feature_mutations.py`, `formal/README.md` mutation paragraph): pinned sha256 of `apt/lift.rs` and `emit/irbcam.rs`; each mutant must be killed by a named fixture. Seed mutants — several are the Python converter's real defects, so the gate encodes what went wrong before:

1. bare `MULTAX` treated as `OFF`; 2. `IPM` not scaled; 3. `RAPID` applied to *all* following `GOTO`s instead of one; 4. `clockwise := nz > 0`; 5. cycle plunge `P + depth·a`; 6. `SPINDL/OFF` ignored; 7. `enc` writes `0` instead of `-1`; 8. sparse index off by one; 9. `rz1`/`ry` swapped; 10. `σ` flipped; 11. arc midpoint at `φ_s − Δ/2`; 12. `decimals` trimming re-enabled; 13. `Some→None` channel accepted; 14. dwell dropped under `Refuse`.

---

## 9. Necessity and sufficiency

### 9.1 Necessity — what breaks without each requirement

| Req | Requirement | What breaks without it |
|---|---|---|
| R-1 | Lift to L2, not L1 (§3.1) | `travel`, `speed`-per-segment, `power`, `tool`, `dwell_s` are L2 fields; via L1 every `GOTO` is a travel at `travel_speed` (`resolve.rs:47-56`) — the converter's defect |
| R-2 | Closed MAJOR-word partition with default-refuse (§6.2) | an unrecognised `TRANS`/`MATRIX` would leave every following point in the wrong frame while the file "imports cleanly" |
| R-3 | `CUTCOM`, `INSERT`, non-DRILL cycles refused (§6.2–6.4) | points offset by an unknown radius; opaque post text lost; fabricated tapping motion |
| R-4 | `try_mm` on every computed quantity (R-NUM-2) | κ·1e307 or a squared delta becomes `inf` in the IR; `Length::mm` only debug-asserts (`units.rs:122-125`) |
| R-5 | Non-finite refused on every token (R-NUM-1) | `1e400` in a `PPRINT` is harmless, in a `GOTO` it is `Xinf` in IRBCAM |
| R-6 | Units statement required or explicitly assumed (§5.1) | a silent 25.4× error, the converter's actual behaviour on inch files |
| R-7 | `RAPID` one-shot (§5.2) | feed cuts emitted as rapids after the first `RAPID` |
| R-8 | `-1` ⇔ `travel`; feed with `speed = 0` refused; explicit travel with `speed = 0` refused (§5.2) | a cut at "unstated" speed would become a robot rapid; an explicit travel at speed 0 would emit an unstated or zero velocity (`[irbcam-travel-speed-unstated]`) |
| R-9 | Direction ↔ (rz1, ry) with spin constant and spin-independence proved (§5.3) | any `rz2` choice would be unjustifiable; with the theorem it is provably irrelevant to where the tool points |
| R-10 | `σ` a single frame constant (⚠F-4) | the tool points into or away from the part depending on a sign nobody checked |
| R-11 | Only ±Z `MOVARC` lifted; helical refused at IRBCAM; full turn refused (§5.4) | chording without a tolerance claim; a three-point circle cannot climb (`krl.rs:673-678`); a full turn has no determinate three points |
| R-12 | Midpoint on the requested sweep (I8) | IRBCAM would fit the *other* arc through the same endpoints |
| R-13 | Cycle expansion as a definition with trace-exact theorem (§5.5) | hole depth or approach direction could differ per implementation with no way to say which is right |
| R-14 | Sparse vectors round-trip, well-formed, and collision-free (I2, I3, §5.7) | a stale tool number or spindle speed applies from the wrong index; Flow B policies overwrite upstream tooling/power |
| R-15 | Dwell/extrusion refuse-by-default with counted policies (§5.8–5.9) | silent drops — the class ADR 0002 §4 forbids |
| R-16 | Fixed ≥6 decimals (§5.10) | IRBCAM rejects or mis-parses (⚠F-9); trimming would give `10` where `10.000000` is required |
| R-17 | `Profile::emit_params` routed through `named()` (§3.3) | `firmware.flavor: "irbcam"` silently emits Marlin |
| R-18 | Ledger rows, TS union, CLI arms, `named()` names (§3.3–3.5) | parity CI fails; or worse, a surface answers a different flavor |
| R-19 | Frozen real Fusion `.apt` and Cura `.gcode` inputs (§10) | every fixture would be Dry-authored and could not detect a misunderstanding of ⚠F-12..17 |
| R-20 | Honest tier statement + Layer-D record (§10.4) | "IRBCAM accepts our files" would be claimed with no oracle, the RAPID situation `docs/14-known-limitations.md:28-33` already warns about |
| R-21 | Source maps many-to-one (§11) | a refusal or finding inside a cycle expansion could not be pointed at a line |
| R-22 | CCLW spindle refused on dialects lacking reverse support (§4.1, §6.5) | cutting tool spun backwards on unidirectional spindles, causing tool destruction |
| R-23 | Cycle entry clearance plane verified (§5.5, §6.4) | rapid traversal collision into stock or fixture before hole plunge |
| R-24 | Parameterized angle units in frame (§3.2, §5.3) | radians rendered as degrees or vice versa causing severe rotational error |

### 9.2 Sufficiency

The goal (§1.1) has three conjuncts. (1) *Faithful carriage on `O`* follows from A7/I5 (abstract), A8/I9 (implementation corpus) and the boundary inventories for the ℝ steps: every observation in `O_apt`/`O_irbcam` is either a ℚ-exact map (units, rapid, sparse channels, structure) with a refinement fixture, or an ℝ map (direction, midpoint) with a proved abstract theorem and an explicitly pending binary64 bound — nothing in `O` is carried by an unstated mechanism. (2) *No silent loss* follows from the partition in §6.2 being exhaustive (every APT statement is modeled, inert-with-finding, or refused; theorem A4 makes the refused set the *exact* complement of the accepted set) together with §7.4 listing every L2 field outside `O_irbcam` and the emitter refusing it by default (I4). (3) *Honest placement* follows from §8's table: each claim names its relation, its domain, and the layer at which it stops; §10.4 keeps IRBCAM itself at Layer D. Together these are sufficient for the goal as stated; they are **not** sufficient for "the robot machines the part correctly", which §1.2 excludes.

### 9.3 Nice-to-have, out of scope for this slice

Arc reconstruction from `CIRCLE` chords (OD-9); helical/non-XY linearisation (OD-3); IRBCAM JSON *import* into Rust; profile plumbing; Python/wasm `import-apt`; tapping/boring cycles; cutter compensation modelling; a bounded binary64 composition for `acos`/`atan2` (promotes I7/I8 to `bounded`); a `verify` rule for "position preceding a cycle is above the RAPTO plane".

---

## 10. Conformance and testing

### 10.1 Tiers (per `conformance/README.md:37`, `docs/22-krl-emit.md:11-32`)

| Evidence | Tier | What it establishes |
|---|---|---|
| IRBCAM import of a pinned emitted file (§10.4) | external oracle, **manual, pinned** — Layer D-adjacent interpretation evidence, not CI | that IRBCAM *version V* accepted *file sha S* on *date D* with *N* targets |
| `tools/validate_irbcam_export.py` against `spec/dry-irbcam-v0.schema.json` (§10.3) | Tier 3+ : dry-core-free but Dry-authored from vendor docs | structural well-formedness (I3) of every golden; **not** IRBCAM acceptance |
| Goldens under `conformance/reports/robot/` and `conformance/apt/` | Tier 2 drift gate | Dry's output did not move |
| Lean fixtures + refinement tests + mutation gate | Layer C | Rust agrees with the model on the corpus |
| Structural/refusal tests | Tier 3 | named behaviours |

Wording rule inherited from `krl_program_structure.rs:1-12`: no test or doc may say "valid IRBCAM" or "imports into IRBCAM" unless it cites a §10.4 record.

### 10.2 Frozen inputs, goldens, vectors

- `conformance/apt/fusion360-apt-cps/<part>.apt` — a **real** Fusion 360 export through Autodesk's stock `apt.cps` (2.5D pocket + drill cycle + one arc, with `MULTAX/ON` for a tilted variant); `conformance/apt/README.md` provenance ledger (Fusion build, `apt.cps` revision/date, exporter, sha256); `MANIFEST.json`. This is the reference input R-19 requires. Goldens beside it: `<part>.ir.json` (lifted L2), `<part>.irbcam.json`, `<part>.irbcam.csv`, `<part>.roundtrip.apt`, regenerated with `UPDATE_GOLDEN=1 cargo test -p dry-core --test apt_import_goldens`.
- `conformance/slicer-corpus/cube__cura-<version>-pla.gcode` — a real Cura export (provenance in that corpus's README; "descriptive evidence, not an oracle") with golden `conformance/reports/robot/cube-cura.irbcam.json` emitted under `--irbcam-extrusion motion-only --irbcam-dwell drop` (until OD-1/OD-8 decide otherwise; the golden records the policy in a header comment field if the format allows, else in its README).
- `conformance/reports/robot/reference-irbcam.json`, `reference-irbcam.csv`, `reference-apt.apt` — hand-seeded structural goldens from the same `reference_program_segments()` KRL/RAPID use (`krl_program_structure.rs:153`, `cnc_industrial_flavors.rs:252`), drift-gated by `crates/core/tests/irbcam_program_structure.rs`.
- IR vector `conformance/vectors/apt_fusion_lift/` (`Spec { design: None, emit: None, frozen: false, feature_tags: ["apt","five-axis","arc","power","tool","dwell","no-oracle"] }`, `spec_vectors.rs:33-46`): the lifted L2 of the Fusion input, with a description disclosing that nothing outside Dry backs it (the `five_axis_drape` wording). It exercises the IR codec on a channel-rich, oriented toolpath. `expected.gcode` is *not* produced (`emit: None`) — the vector generator only writes g-code, and an IRBCAM document under that name would mislead.

### 10.3 Independent structural checker

`spec/dry-irbcam-v0.schema.json` (fields per ⚠F-1..9, authored from vendor documentation with the source cited in `$comment`) and `tools/validate_irbcam_export.py` (stdlib + `jsonschema`, no `dry-core`; pattern `tools/validate_vectors.py`) checking: schema; sparse indices strictly increasing and `< len(targets)`; `type ∈ {0,1}`; every `type 1` followed by a `type 0`; `velocity == -1 or velocity > 0`; every numeric field has ≥ `decimals` fractional digits as text; CSV/JSON logical equality when both goldens exist. Runs in CI (`spec-vectors`-style job) over every `*.irbcam.json`/`.csv` golden. It is Dry checking Dry's *structure* against Dry's *reading* of IRBCAM's docs — Tier 3+, stated as such.

### 10.4 External oracle strategy — IRBCAM itself

IRBCAM is proprietary desktop software; it cannot run in CI. The evidence is a **qualification record** `conformance/apt/irbcam-import-qualification.md`: IRBCAM version, OS, date, operator, sha256 of the emitted golden, the import steps, and the observed result (target count; a screenshot of the first/last target values; whether arcs rendered as circular moves; which fields IRBCAM rejected). Each row is one (file, version) pair. With a record, the claim "file S was imported by IRBCAM V producing N targets matching the golden on `O_irbcam`" is permitted; without one, only §10.1's lower tiers may be cited. A second record kind — an IRBCAM-generated APT/G-code export of the *same* part compared against the Dry golden on `O_irbcam` — is the closest thing to a differential oracle available and is recommended for the first qualification. The plugin API being read-only (⚠F-11) means a future QML plugin can *read back* the loaded targets and diff them against Dry's `O_irbcam` projection automatically (§13), turning the record into an instrument; it cannot make IRBCAM emit for us.

### 10.5 Test files

`crates/core/tests/apt_import.rs` (units, MULTAX, RAPID one-shot, MOVARC CW/CCW, cycle expansion, source maps), `apt_import_refusals.rs` (every §6 code), `apt_roundtrip.rs` (A7 over vectors + goldens), `apt_lift_refinement.rs`, `irbcam_program_structure.rs` (structure, goldens, refusals, I6 spin column test, I8 numeric check), `irbcam_lower_refinement.rs`; `crates/cli/tests/cli.rs`: `import_apt_writes_dry_ir_json`, `review_apt_reports_unmodeled_statements`, `emit_irbcam_from_fusion_apt_matches_golden`, `emit_irbcam_from_cura_gcode_matches_golden`, `emit_irbcam_leaves_no_file_behind_when_refused`, `emit_irbcam_strict_fails_on_dropped_dwell`; existing `emit_rejects_unrepresentable.rs`/`emit_refuses_non_finite.rs` gain the three flavors. `python tools/validate_vectors.py conformance/vectors` continues to pass with the new vector.

---

## 11. Error handling and diagnostics

### 11.1 Source maps

`ImportedApt` (§3.2): `source_lines` (every input line, verbatim, no trailing newline); `segment_source_lines[i]` = 1-based first line of the statement that produced segment `i` (many-to-one: a cycle `GOTO` produces 3–4 segments); `statement_segments[k]` = the contiguous `Range<usize>` of segments statement `k` produced, or `None`. A continued statement (`$`, ⚠F-17) is attributed to its first line; `ParsedAptStatement::line_count` carries the span. Invariants (tested): `segment_source_lines.len() == toolpath.segments.len()`; the ranges in `statement_segments` are disjoint, ordered, and cover `0..segments.len()`.

### 11.2 Findings

New `RuleId`s (`crates/core/src/verify.rs:272-330` registry; kebab-case in `as_str` `:373`; `spec/dry-reports-v1.schema.json`; `docs/11-profiles-and-reports.md`; a report golden under `conformance/reports/unmodeled/`):

| Rule | Severity | Source | Message shape |
|---|---|---|---|
| `unmodeled-apt` | Warning | class-I statements | `"{MAJOR} is preserved but not semantically verified: {raw}"` (mirror of `report.rs:122-138`) |
| `apt-circle-annotation-ignored` | Warning | `CIRCLE` records | `"CIRCLE about ({cx}, {cy}, {cz}) r={r} is advisory: the following GOTO points lift as lines exactly as written"` |
| `apt-spindle-direction-dropped` | Info | `SPINDL` with `CLW`/`CCLW` | `"spindle direction {dir} has no L2 channel; power {n} carried, direction dropped"` |
| `apt-loadtl-minor-words-dropped` | Info | `LOADTL` with extra words | names the words |
| `irbcam-declared-loss` | Warning (CLI, from `IrbcamEmitStats`) | emit | `"{n} dwell(s) dropped under --irbcam-dwell drop"` etc.; `--irbcam-strict` ⇒ exit 1 |

`ReviewReport::add_unmodeled_apt(&ImportedApt)` mirrors `add_unmodeled_gcode`.

### 11.3 Error taxonomy

Fatal, first-error, located (`AptImportError { source_line, code, message }`, `Display` = `"line {n}: [{code}] {message}"`; `source_line 0` = parameter/whole-file, as `gcode/lift.rs:564-598`):

| Class | Codes |
|---|---|
| syntax | `syntax`, `continuation-unterminated`, `non-finite`, `goto-arity`, `limit-exceeded` |
| units/feed | `unit-missing`, `unit-unknown`, `unit-late`, `feed-negative`, `feed-unit-unsupported` |
| orientation | `multax-inconsistent`, `tlaxis-degenerate` |
| arc | `arc-normal-degenerate`, `arc-normal-unsupported`, `arc-full-turn`, `arc-off-circle`, `arc-sweep-mismatch`, `arc-radius-nonpositive`, `arc-endpoint-missing`, `rapid-arc` |
| cycle | `cycle-unsupported`, `cycle-minor-word`, `cycle-domain`, `cycle-off-inactive`, `cycle-statement-inside`, `cycle-clearance-violation` |
| channels | `loadtl-domain`, `spindle-domain`, `spindle-on-without-rpm`, `spindle-direction-unsupported`, `irbcam-channel-collision` |
| policy / emission | `cutcom-on`, `insert-opaque`, `major-word-hazardous`, `major-word-unknown`, `irbcam-feed-unstated`, `irbcam-travel-speed-unstated` |

Emit refusals are `CodecError::Other(String)` (`crates/core/src/codec/error.rs:29`, the convention for every emitter) whose text begins with the bracketed code from §6.5 and names the value and the segment index — e.g. `"[irbcam-arc-helical] segment 12 rises from Z 5 to Z 8: an IRBCAM arc is the circle through three points and cannot climb"`, `"[irbcam-travel-speed-unstated] segment 4: rapid encoding explicit requires travel speed > 0"`, or `"[spindle-direction-unsupported] segment 2 commands CCLW rotation but IRBCAM target list only supports unidirectional spindle speeds"`. Messages are full sentences naming value and reason (N1 in the kernel map; precedents `rapid.rs:91-95`, `krl.rs:673-678`).

---

## 12. Open decisions for the owner

| # | Decision | Options | Recommendation |
|---|---|---|---|
| OD-0 | Replace every `⚠F-n` in §2 with the verbatim external fact; re-derive the constants/signs in §5 that depend on it. | — | Completed for APT-CL against official ISO 4343:2000(E) standard: F-12, F-13 (Cl. 5.42 `RAPID` one-shot), F-14 (Cl. 5.32 `MULTAX`), F-16 (Cl. 10.8 `CYCLE`), F-17 (Cl. 4.2 syntax), F-19 (Cl. 5.13 `DELAY/DWELL`), F-20 (Cl. 5.51 `SPINDL`). IRBCAM facts F-1..F-11 remain based on vendor technical docs and require mechanical Layer-D qualification per §10.4. In particular verify F-3/F-4 (Euler convention, tool-Z sign) on live robot controller. |
| OD-1 | How Cura extrusion reaches IRBCAM | (a) `MotionOnly` — extrusion ignored, counted; (b) `ToolToggle { extrude_tool, travel_tool }` — `toolNumber` flips on extrude/travel, lets the robot cell switch an extruder output (refusing `[irbcam-channel-collision]` if `tool` is already commanded); (c) `SpindleRate` — `spindleSpeed` carries mm³/s (`volume·speed/(60·length)`) as a proportional signal (refusing `[irbcam-channel-collision]` if `power` is already commanded); (d) keep `Refuse` and make Flow B a non-goal | (b) as the default *when the user opts in*, with `Refuse` remaining the default policy. (b) is exact and needs no numeric boundary; (c) repurposes RPM semantics and needs `FM1.F64.IRBCAM.SPINDLE_RATE`; (a) loses the on/off information the cell most needs. |
| OD-2 | `rz2` default spin | 0°; 180°; user-supplied per run | 0°, exposed as `--irbcam-spin-deg`; theorem I6 makes the value irrelevant to tool direction, so the only reason to change it is cable routing, which is a cell property. |
| OD-3 | Arcs with normal ∉ {±Z} and helical arcs | (a) refuse (v0 text); (b) linearise at import with `--arc-linearise-tolerance ε` as `approximate(ε)` under a new boundary + theorem; (c) linearise only at IRBCAM emit | (a) for v0; (b) as the follow-up because it keeps L2 the single truth and lets `verify` see the chords. Never (c): it would make the IRBCAM file disagree with the reviewed IR. |
| OD-4 | Cura travels: `-1` (rapid) or explicit velocity | `MinusOne` (travel speed lost, travel flag kept); `Explicit` (travel flag lost, speed kept; refuses unstated speed `[irbcam-travel-speed-unstated]`) | `MinusOne` for Flow A (APT `RAPID` has no speed anyway); for Flow B ask the cell owner — a robot rapid near a printed part may be faster than Cura's travel speed. Expose `--irbcam-rapid`, default `minus-one`, and record the choice in the golden README. |
| OD-5 | CSV export parity | ship CSV in v0 with logical-equality check against JSON; defer CSV | Ship it: the checker in §10.3 makes parity cheap, and ⚠F-9 (decimals) suggests CSV is a first-class import path. |
| OD-6 | Tool-Z sign `σ` | `+1` (IRBCAM tool Z = Dry direction) or `−1` (flip, as KRL does with roll 180, `krl.rs` header) | Fact-dependent (⚠F-4). Whatever it is, hard-code it in `IrbcamFrame` and pin it with mutant 10; do not expose a flag. |
| OD-7 | Unknown MAJOR word default | `Refuse` (v0 text); `Preserve` with finding | `Refuse`. Fusion's `apt.cps` word set is small and closed (⚠F-12); a real file hitting this refusal is a signal to extend §6.2, not to relax it. |
| OD-8 | Dwell in IRBCAM | `Refuse` default; `Drop` default | `Refuse` default, `Drop` opt-in with `--irbcam-strict` available. |
| OD-9 | CATIA/NX `CIRCLE` + chords | advisory only (v0 text); reconstruct an `Arc` when all chord points are on-circle within τ and the normal is ±Z | Advisory in v0. Reconstruction is an inference over the source; when added it needs its own `approximate`/`rejection` claims. |
| OD-10 | Ship `--format apt` as a user target or keep it test-only | user-facing (Autodesk-flavoured subset); test-only helper for A7 | Ship it: IRBCAM imports APT-CL, so it is a second, independent route into IRBCAM (useful for the differential record in §10.4), and it costs one `EmitOutputFormat` arm. |
| OD-11 | Default `decimals` | 6 (⚠F-9 minimum); 9; 17 (bit-exact) | 6 for shipped output (matches the requirement, smallest files); 17 in the refinement fixtures. |
| OD-12 | Profile plumbing (`machine.irbcam`) in v0 | yes; defer like KRL | Defer; flags are the v0 surface, and the profile schema change deserves its own slice. |
| OD-13 | Whether `dry review-apt` ships in v0 or findings are printed by `import-apt` to stderr | separate command (v0 text); flag on `import-apt` | Separate command, mirroring `review-gcode`; the report contract already exists. |

---

## 13. Out of scope / future

- **IRBCAM QML read-back plugin** (⚠F-11): reads the loaded targets and diffs them against Dry's `O_irbcam` projection; would automate §10.4 and give I5 a Layer-D instrument. Cannot write targets.
- **IRBCAM JSON/CSV lift into Rust** (`dry import-irbcam`): straightforward, but no flow needs it; the Lean `liftIrbcam` is its specification if it is ever built.
- **APT emit for non-IRBCAM consumers** (CATIA/NX `CIRCLE` style, Mastercam NCI): only the Autodesk-flavoured subset is defined here (OD-10).
- **Helical / non-XY linearisation** (OD-3b), **`CIRCLE` arc reconstruction** (OD-9), **cycles beyond DRILL**, **cutter compensation**, **transformations** (`TRANS`/`MATRIX`): each needs new claims before it can be accepted.
- **Binary64 bounds for `acos`/`atan2`/midpoint trig**: promotes I7/I8 from `numeric = pending` to `bounded`.
- **Python/wasm/TS `import-apt`** and the imported-L2 emit path through the bindings (ledger `absent` rows in §3.5).
- **Fixing `dry_fusion_postprocessor.cps`**: the RS-274 post is superseded by Flow A; its README claims are corrected (§3.6) and the file itself is left to a separate decision.
- **Robot kinematics, reachability, collision, physical qualification**: Layer D; recorded, never claimed.
