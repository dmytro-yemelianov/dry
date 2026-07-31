# Verify strengthening — design (H1.3)

**Date:** 2026-07-31 · **Task:** H1.3 (`docs/superpowers/specs/2026-07-31-core-hardening-audit.md`)
**Issue:** [#185](https://github.com/dmytro-yemelianov/dry/issues/185)
**Governing decisions:** [ADR 0001](../../adr/0001-formal-assurance-constitution.md) (assurance layering),
[ADR 0002](../../adr/0002-numeric-ingress-and-emission-gates.md) (refuse rather than emit vacuously;
formal artifacts are authority)

## 1. Problem

`verify` is a strong **contract** checker and a weak **well-formedness** checker, and downstream
assurance arguments — including product claims — lean on it as the latter. ADR 0002 already records
this in its Consequences and defers it here.

Concretely, against the code as it stands:

- **Nothing compares `segments[i].end` to `segments[i+1].start`.** The only adjacency computation,
  `junction_contiguous` (`verify.rs:492`), *suppresses* a `junction-velocity` warning rather than
  raising one, and it does so at a 0.1 mm threshold. The emitter writes endpoints only
  (`emit/gcode.rs:330-343`: an axis word is emitted iff `s.end[i].is_some() && changed`), so a gap
  produces **no repositioning move** — the controller cuts a straight line from the previous end to
  this segment's end, along a path no rule inspected, while depositing this segment's `E`. For an
  `Arc`, `I`/`J` are computed from the segment's own `start` (`emit/gcode.rs:378-381`) but are
  interpreted by the controller relative to its *real* position, so the executed radius disagrees
  with the emitted one. `monotonic-z` is intra-segment only (`verify.rs:666`), so a vase-mode path
  that plunges between segments verifies clean.
- **No rule relates deposited material to geometry.** `resolve` establishes
  `volume = length·width·height·flow` and `filament = volume/area` by construction
  (`resolve.rs:573-576`); `verify` never re-checks either.
- **Sign and zero are unchecked.** Every material guard is `> 0.0`, so negative quantities fall into
  "not applicable" rather than into a finding.
- **`junction-velocity` measures a scalar feedrate delta** (`verify.rs:845`), not a direction change,
  while `optimize/adaptive_speed.rs` computes the correct cornering quantity under the same name.
- **A vacuous pass is byte-identical to a real one.** `Report` (`verify.rs:306`) carries only
  `findings`; `Contracts` is `Deserialize` but not `Serialize` (`verify.rs:21`).

## 2. What is actually true today

The audit is labelled hypotheses-to-verify and has been wrong twice. Everything below was re-checked
against the code; §11 lists where it is wrong again.

**Rule reachability under `Contracts::default()`.** Six of eighteen rules are evaluated with no
contract supplied: `unmodeled-gcode`, `finite`, `travel-extrudes`, `bead`, `orientation-not-unit`,
`arc-radius`. Only five of those are `Error` severity, so **five of eighteen rules can flip
`Report::ok()`**. The audit's "5 of 18" is right for `ok()` and one short for "produces any finding".

**Assurance-shaped `report.ok()` call sites.** Positive (`assert!(report.ok())`-shaped) uses:
`crates/core/tests/rewrite_balanced_max_gate.rs:134,169`, `crates/core/tests/rewrite_safe_gate.rs:104`,
`crates/core/tests/verify_contracts.rs:26`, `crates/core/tests/memory_scale.rs:113,234`,
`crates/core/tests/cnc_pocket_e2e.rs:69`, `crates/core/src/generate/pocket.rs:1086`,
`crates/core/src/verify.rs:1093`, and the CLI exit code at `crates/cli/src/main.rs:1881`. Of these,
`cnc_pocket_e2e.rs:69`, `pocket.rs:1086`, `memory_scale.rs:113,234` and `verify.rs:1093` run under
`Contracts::default()` — i.e. they assert five reachable rules and read as "sound". The `rewrite_*`
sites supply real `bounds` and are non-vacuous *for the bounds claim they make*.

**Every in-tree producer is exactly position-continuous.** `resolve` threads `pos` and writes
`start: pos` then `pos = end` on every emitting arm (`resolve.rs:541,577,585,642,726`), and
`Op::Move` emits no segment when `end == pos`. `gcode::lift` threads `state.pos = end`
(`lift.rs:660`). The codec round-trips bits. In `optimize/`: `travel_reorder` regenerates connecting
travels (`travel.rs:make_travel`), `z_hop` splits into lift/traverse/drop sharing endpoints,
`merge_collinear` sums into the surviving segment, `arc_fit` takes `start: first.start,
end: last.end`, `coasting` splits at a shared point, `adaptive_speed` touches only `speed`. So
continuity is not a tolerance question for any Dry-produced toolpath — it is exact. This is what
makes an always-on rule affordable (§3.1).

**Two `optimize` passes deliberately break `volume = length·w·h`.** `coasting` zeroes `volume` and
`filament` on the tail of an extrusion run while keeping `length`, `width` and `height`
(`coasting.rs:160-171`), and leaves `travel = false`. `arc_fit` sets `length = radius·swept` but
`volume = Σ` of the *chord* volumes (`arc.rs:140-143`); for a 90° arc recovered from three points the
chords are 2.6% shorter than the arc, so the identity is violated by that much. Both are legitimate
Dry outputs. This is decisive for §3.3.

**`filament` cannot be checked against a diameter from the IR alone.** `bead_area(p.dia)` lives in
`ResolveParams` (`resolve.rs:477`); `Toolpath`/`Meta` (`ir.rs:124-149`) carry no filament diameter.
The audit's `filament ≈ volume/(πd²/4)` is therefore not implementable as written without a new
input. It *is* implementable as a ratio-constancy check (§3.3).

**Arc length has one formula everywhere.** `resolve.rs:614` uses `hypot(radius·swept, dz)`;
`gcode/lift.rs:840-843` uses the identical `hypot(planar, dz)`; `optimize/arc.rs:140` uses
`radius·swept` for a planar run. So `length ≈ hypot(r·sweep, Δz)` is an always-on-safe predicate.

**The tolerance idiom already exists twice.** `verify.rs:522` and `gcode/lift.rs:819` both use
`1e-6 * max(a, b, 1.0)` — absolute below 1 mm, relative above.

## 3. Decisions

### 3.1 Which rules are always-on structural, and which are contract-gated

This is the central choice: an always-on rule changes what "clean" means for every existing caller.
The test applied to each candidate is **"can any Dry producer emit IR that violates this?"** If no,
it is a well-formedness property and belongs always-on. If yes, it encodes a *policy* and belongs
behind a contract.

| New rule | Severity | Gating | Why |
|---|---|---|---|
| `continuity` | error | **always-on** | Every producer is exactly continuous (§2). A violation makes the emitted program describe a different path than the IR — an artifact that is confidently wrong in ADR 0002 §4's sense. |
| `negative-quantity` | error | **always-on** | `length`, `volume`, `speed` < 0 and `width`/`height` ≤ 0 when `Some` are outside the IR's own type contract. No producer emits them; no contract can make them acceptable. (`filament` < 0 is a retraction and is excluded.) |
| `arc-length` | error | **always-on** | One formula across `resolve`, `lift` and `arc_fit` (§2). A disagreeing `length` misstates time, flow and travel accounting for a segment whose geometry the emitter takes from the endpoints. |
| `filament-consistency` | **warning**, then error | **always-on** | `volume/filament` must be one constant per `tool` across the program. Holds exactly for `resolve` (one `area`) and `lift` (one `filament_area`), and is preserved by every `optimize` pass. Staged (§8) because multi-diameter IR is *unusual*, not *ill-formed*. |
| `bead-volume` | error | **contract-gated** (`bead_volume_tolerance: Option<f64>`) | `coasting` and `arc_fit` violate `volume ≈ length·w·h·flow` by design, and imported IR takes `volume` from `E` while `width`/`height` come from a user-supplied constant. Always-on is not available. |

`junction-velocity` keeps its gate (`kinematics.max_junction_velocity_mm_s`); only its measure changes
(§3.4).

Rejected alternatives:

- **All five contract-gated.** Rejected: it leaves `Contracts::default()` at five reachable error
  rules, so the eight `report.ok()` sites keep meaning what they mean today and H1.3 delivers nothing
  to the callers that motivated it. Well-formedness is not a machine-specific policy and should not
  need a machine to be checked.
- **All five always-on.** Rejected on `bead-volume`, which our own optimizer violates; shipping it
  always-on would mean either exempting `SegmentKind::Arc` and zero-volume segments (a rule defined by
  its exceptions) or reporting errors on correct output.
- **A new `Severity::Structural` tier below `Warning`.** Rejected: it adds a wire enum value to
  `spec/dry-reports-v1.schema.json` for every consumer in order to avoid making one decision about
  severity, and ADR 0001 is explicit that assurance strength must be *named*, not graded into
  invisibility.

### 3.2 Continuity: predicate, tolerance, and undefined axes

Track the previous segment's `end` across **all** segment kinds (not the `prev_print_end` used by
`junction-velocity`, which resets on travel — a travel that lands somewhere other than where the next
print starts is exactly the hazard). For consecutive segments *a*, *b* and each axis *k* ∈ {X, Y, Z}:

```
fire iff  a.end[k] = Some(p) ∧ b.start[k] = Some(q) ∧ |p − q| > 1e-6 · max(|p|, |q|, 1.0)
```

- **`None` participates as "inherit", never as a violation.** `(Some, None)`, `(None, None)` and
  `(None, Some)` are all continuous. This is forced by the semantics on both sides: `resolve` writes
  `end = x.map(..).or(pos[k])` and the emitter carries `prog_pos` forward when `s.start[k]` is
  `None`, so an unstated axis *is* the previous value. `(None, Some)` can only occur at the first
  definition of an axis — definedness is monotone in both `resolve` and `lift` — which is the
  legitimate case `conformance/vectors/travel_and_line` was frozen to pin ("a travel move with
  undefined (null) start axes").
- **Per-axis, not Euclidean.** The emitter emits per-axis words; a per-axis predicate names the
  offending axis in the message, matching `bounds`'s existing style.
- **Hybrid absolute/relative tolerance, `CONTINUITY_TOLERANCE_MM = 1e-6`.** Absolute 1e-6 mm below
  1 mm, relative 1e-6 above. Chosen because (i) it is the *third* use of an idiom already in the tree
  at `verify.rs:522` and `lift.rs:819`, so `verify` keeps one tolerance policy rather than three;
  (ii) 1e-6 mm matches the emitter's own print resolution (`num()` is `{v:.6}`), so any gap below it
  is not representable in the output; (iii) a purely absolute rule is unsatisfiable at large
  coordinates, where `f64` spacing exceeds 1e-6 mm — and ADR 0002 §2 measured acceptance out to
  1e154 mm, so the rule must not become the magnitude policy that ADR deliberately avoided.
- **`SegmentKind::ManualGcode` resets the tracked position to `[None; 3]`** rather than being
  compared. Verbatim G-code may move the machine arbitrarily; `unmodeled-gcode` already says the
  segment is outside the model, and claiming a continuity result across it would be a stronger claim
  than we can support. `Dwell` participates normally (`start == end == pos` in both producers).
- **State is O(1)** — one `[Option<Length>; 3]` — so `verify_stream` stays streaming
  (`memory_scale.rs` covers 20k segments).

Rejected: **0.1 mm, reusing `junction_contiguous`'s threshold.** That number exists to *suppress* a
warning, where being generous is safe; as a violation threshold it would admit a 99 µm gap, which on
a 0.2 mm layer is half a layer height. Rejected: **Euclidean distance** — loses the axis in the
message and mixes Z (where a gap is a plunge) with XY (where it is a scar).

### 3.3 Material consistency: form and applicable segment kinds

Split into two rules, because the two halves have different achievable strength.

**`filament-consistency` (always-on).** Over segments with `!travel ∧ volume > 0 ∧ filament > 0`,
grouped by `s.tool`, the ratio `volume/filament` must agree with the group's first observed ratio
within **relative 1e-6**. Rationale: the ratio *is* the filament cross-section, `resolve` and `lift`
each use exactly one, `merge`/`arc_fit` sum numerator and denominator together, and `coasting` zeroes
both (excluded by the `> 0` gates). No new contract input, and it catches the audit's C5 class —
an under-extruded segment whose `filament` disagrees with its `volume` — without inventing a diameter
the IR does not carry. Relative form, because the quantity compared is itself a ratio.

Known limit, stated rather than hidden: a *uniformly* wrong diameter yields a constant ratio and
passes. That residue needs the diameter, and the diameter needs an IR or contract input — deferred
with §3.7's CNC work, not smuggled in.

**`bead-volume` (contract-gated on `bead_volume_tolerance: Option<f64>`, relative).** For
`SegmentKind::Line` and `Spline` only, with `width` and `height` both `Some` and `volume > 0`:

```
fire iff  |volume − length·width·height·flow| > tol · length·width·height·flow
```

`flow` is `s.flow.unwrap_or(1.0)` — omitted from the wire when exactly 1.0 (`resolve.rs:502-504`), so
it must be defaulted, not skipped. `Arc` is excluded because `arc_fit` produces the chord/arc
mismatch quantified in §2; `volume == 0` is excluded because that is `coasting`'s output.

Rejected: **an upper-bound-only always-on form** (`volume ≤ length·w·h·flow·(1+ε)`), which would
survive both `coasting` and `arc_fit` since each pushes `volume` *down*. Rejected because it is
one-sided by accident rather than by principle — under-extrusion, the audit's actual 8000× example,
is exactly what it cannot see — and because imported IR with a user-supplied `line_width` breaks it
in both directions on any real slicer output (variable width, first layer, bridges), which would put
`conformance/gcode` in conflict for no gain.

### 3.4 `junction-velocity`: fix the measure, keep the id

**Fix the measure.** At a contiguous printing junction, with unit tangents `t̂ₐ` (exit of *a*) and
`t̂_b` (entry of *b*):

```
fire iff  ‖ v_b·t̂_b − v_a·t̂ₐ ‖ > max_junction_velocity_mm_s
```

Tangents are computed the way `optimize/adaptive_speed.rs::get_tangents` already computes them —
arc-aware (tangent ⟂ radius, winding-signed) and Δz-aware — so `verify` checks the quantity
`adaptive_speed` shapes.

Reasoning:

- The contract field is `max_junction_velocity_mm_s`, documented in `docs/11` as the machine's
  square-corner velocity. **The name and the contract are right; the implementation is the bug.**
  Renaming the rule to match the bug would publish a new permanent id (`junction-speed-change`) whose
  only known behaviour is to miss corners and fire on collinear accelerations, and would leave the
  contract field pointing at it.
- ADR 0001's whole point is that a named relation must mean one thing. `optimize` and `verify`
  computing different quantities under one name is precisely the drift it exists to prevent.
- The measure **strictly generalises** the current one: when `t̂ₐ = t̂_b` it reduces to `|v_b − v_a|`.
  Nothing that fires today stops firing, so this cannot silently relax an existing gate.
- The rule is a **warning**, so no `Report::ok()` anywhere changes.

Concrete, pre-computed corpus impact: `conformance/reports/kinematics/verify.json` is generated from a
fixture whose seg 0 runs +X at 25 mm/s and seg 1 runs +Y at 50 mm/s — a true 90° corner
(`report_goldens.rs:265-328`). Today it reports `Δv 25.0`; under the vector measure it reports
`hypot(25, 50) = 55.9`. **The finding set (rule, severity, segment) is unchanged; only the message
number moves.** That is the entire blast radius of this decision.

Rejected: **rename + add a second rule.** Defensible — it preserves byte-identical output for the
existing id — but it doubles the catalog for the same coverage and enshrines a measure with no
physical meaning as a public contract. Rejected: **leave it and add `corner-velocity` beside it** —
same cost, plus two rules named after one machine limit.

### 3.5 What `Report` gains, and the schema consequence

`Report` gains three fields, all `#[serde(default)]`:

```rust
pub struct Report {
    pub findings: Vec<Finding>,
    pub segments_inspected: usize,   // was this pass over anything?
    pub rules_evaluated: Vec<String>, // which rule ids were in force (catalog order, wire ids)
    pub contracts: Contracts,         // with what limits
}
pub fn evaluated(&self, rule: RuleId) -> bool;
```

All three are needed and none is redundant: `segments_inspected` separates "clean" from "empty";
`rules_evaluated` separates "clean under 23 rules" from "clean under 10"; `contracts` separates
"`max_flow` was checked" from "`max_flow` was checked against 1e9". `Contracts` and
`KinematicContracts` gain `Serialize` (they are already `Deserialize`).

**Schema consequence — and it is not purely additive.** `spec/dry-reports-v1.schema.json`'s
`VerifyReport` is `"additionalProperties": false, "required": ["findings"]`. Therefore:

- Documents produced by *older* Dry (findings only) **remain valid** under the amended schema,
  provided the three new properties are added to `properties` but **not** to `required`. This is the
  compatibility direction we can preserve, and we do.
- Documents produced by *newer* Dry are **rejected by the unamended v1 schema text**. Any consumer
  pinning the published v1 file breaks. This is a wire-format break and must be recorded as BREAKING
  in `CHANGELOG.md` alongside ADR 0002's other acceptance narrowings — not described as additive.
- Decision: **amend v1 in place; do not mint v2.** A v2 forces every consumer to migrate for three
  optional keys and splits the report vocabulary; the amendment is a strict superset in the direction
  that matters (old documents keep validating), and `docs/11` already reserves "minor change" for
  additions to the report contract. The honest statement in the changelog is "additive to the schema,
  breaking for strict validators of the previous schema text."

`Contracts` also needs `#[serde(deny_unknown_fields)]`-free round-tripping and a stable field order;
the `contracts` echo must serialize `None` fields as `null` or skip them consistently, since
`conformance/reports/*/verify.json` are byte-compared goldens.

### 3.6 No `Contracts::strict()`; make vacuity visible instead

**Decision: do not add `Contracts::strict()` or any maximal preset.**

A "strict" preset would have to supply `bounds`, `max_flow`, `speed_range`, `min_temp` and the
kinematic limits — i.e. **fabricate a machine**. `report.ok()` under such a preset would mean "clean
against a fictional printer", which reads stronger and *is* weaker than today. That is exactly the
confidently-wrong artifact ADR 0002 §4 forbids, and under ADR 0001 it is an unnamed relation
masquerading as a named one.

What replaces it:

1. **`Contracts::default()` becomes a materially stronger baseline for free.** Reachable rules go
   from 6/18 (5 error) to **10/23** — 8 error on landing, 9 after `filament-consistency` is promoted
   per §8: `finite`, `travel-extrudes`, `bead`,
   `orientation-not-unit`, `arc-radius`, `continuity`, `negative-quantity`, `arc-length`,
   `filament-consistency`, plus `unmodeled-gcode`. Most of the eight sites wanted *well-formedness*,
   and that is now what they get.
2. **Assurance sites state their coverage.** Each becomes `assert!(report.ok())` **plus** an explicit
   `assert!(report.evaluated(RuleId::X))` for the rules its claim actually depends on:
   - `cnc_pocket_e2e.rs:69` and `generate/pocket.rs:1086` — assert `Continuity`, `ArcLength`,
     `NegativeQuantity`, `FilamentConsistency`, and `segments_inspected > 0`. The pocket path's
     acceptance claim is geometric; these are the rules that carry it.
   - `memory_scale.rs:113,234` — its subject is peak working set, not soundness. Keep `ok()`, add
     `assert_eq!(report.segments_inspected, N)` so a decoder that silently yields zero segments
     cannot pass the memory bar.
   - `verify.rs:1093` `empty_toolpath_is_ok` — **invert it**: assert `segments_inspected == 0`. It is
     the canonical vacuous pass and should be named as one.
   - `rewrite_safe_gate.rs:104`, `rewrite_balanced_max_gate.rs:134,169`, `verify_contracts.rs:26` —
     already non-vacuous for their claims; add `evaluated(RuleId::Bounds)` to pin that the contract
     they rely on is actually in force.
   - `cli/src/main.rs:1881` — unchanged; the CLI's contracts come from the profile, and the exit code
     is already the right semantics. The human-readable branch should print
     `N segments, M rules in force` so a vacuous run is visible without `--json`.
3. **A new pinning test**, `contracts_default_evaluates_only_structural_rules`, asserts the exact
   always-on id set, so the 10/23 number cannot drift silently the way 5/18 did.

Free consequence worth naming: `apply_gated`/`apply_safe_gated` compare error-rule sets before and
after a rewrite, so **the optimize gates now also reject any rewrite that introduces a discontinuity,
a sign inversion, or a material inconsistency** — without touching `optimize/`.

### 3.7 CNC rules: explicitly out of scope, deferred

**Decision: H1.3 adds no CNC-specific rules.**

- The rules a mill needs — stepover vs tool diameter, depth-of-cut vs flute length, plunge feed vs
  cutting feed, spindle-on-before-cut, climb vs conventional — are **contract** rules over quantities
  the IR does not carry. There is no tool diameter, no flute count, no stock and no material in
  `Segment` or `Meta`. Writing them today means reinterpreting `width` as tool diameter and `height`
  as depth-per-pass, which is what the pocket acceptance path already does implicitly; adding rules
  on top would freeze that reinterpretation into the **public rule catalog**, where it is far harder
  to undo than in a generator.
- The correct precondition is an IR/profile change (`spec/dry-ir-v0.schema.json`,
  `spec/dry-profile-v1.schema.json`), which is a spec slice, not a verify slice.
- H1.3 nonetheless improves CNC assurance more than a bespoke CNC rule would: `continuity` is *more*
  load-bearing on a mill than on a printer, because a gap in a milling path is a full-depth cut
  across the part at cutting feed, not a missing bead.
- Deferred to [#180](https://github.com/dmytro-yemelianov/dry/issues/180) (already scoped as "5-axis
  hardening — limits, verify rules"), with the IR precondition recorded there.

## 4. Rule catalog delta

Catalog goes 18 → 23. `RuleId::ALL`, `as_str`, `from_wire`, `default_severity`, `summary`,
`docs/11-profiles-and-reports.md` §2 and `conformance/reports/` seeds all derive from the one enum and
move together (`report_goldens.rs` already asserts every id is covered by a seed).

| Id | Severity | Enabled by |
|---|---|---|
| `continuity` | error | always |
| `negative-quantity` | error | always |
| `arc-length` | error | always |
| `filament-consistency` | warning → error (§8) | always |
| `bead-volume` | error | `process.bead_volume_tolerance` |

`docs/11` already states the rule set is open for addition ("a reader MAY treat an unknown rule id as
a forward-compatible addition"), so new ids are the compatible part of this change; the severities and
the `Report` shape are the incompatible parts.

## 5. Adjudicating a rule that fires on a frozen corpus

The frozen corpora under `conformance/vectors` and `conformance/gcode` are oracle-generated and
well-formed by construction (ADR 0002 §Context). If a new always-on rule fires on one, that is a
finding either way — the rule is wrong, or FullControl produces output Dry now calls invalid. **This
procedure is fixed here, before the rule is written, because the temptation at implementation time
will be to weaken the rule to keep CI green.**

1. **Probe before wiring.** Each new always-on rule is first run over every frozen corpus as a
   report-only probe (an `#[ignore]`d test or a `tools/` script), *before* it is added to
   `verify_stream`. Its output goes in the PR body.
2. **A hit stops the PR.** The finding is triaged into exactly one class, named in writing:
   - **(a) rule defect** — the predicate is wrong: bad tolerance, wrong `None` handling, or a
     legitimate producer the rule did not anticipate. Admissible **only** if the PR names the
     producer and the code path (as `coasting`/`arc_fit` were named in §2). The rule is fixed; the
     fixture is untouched.
   - **(b) oracle divergence** — the corpus really is ill-formed under a predicate we are prepared to
     defend publicly. Then the fixture is **not** regenerated, **not** deleted, and the rule is
     **not** weakened. The fixture moves to `conformance/vectors/_negative/` with a note, and the
     divergence is filed as its own issue, mirroring ADR 0002 §6's handling of a code/model
     divergence.
   - **(c) synthetic-fixture defect** — a hand-authored fixture under `conformance/reports/` that was
     never intended to describe a connected, materially consistent path. The fixture is repaired;
     this is not weakening, because the fixture is not oracle output. §6 lists the ones we already
     know about.
3. **"Loosening the tolerance made it pass" is a (b) in disguise** and is not an admissible (a).
4. Every tolerance constant introduced this way is registered per §7 so it cannot be retuned later
   without the same scrutiny.

**Prediction, from §2:** because `resolve`, `lift` and the codec all thread position exactly, the
continuity, arc-length and filament-consistency probes are expected to fire **nowhere** in
`conformance/vectors` or `conformance/gcode`. If any of them does, that alone is the headline finding
of the slice and the rule does not ship until it is triaged. The noise this slice will actually
generate comes from our own hand-built goldens — which is precisely where (c) is easiest to mistake
for (b).

## 6. Known corpus hits, pre-computed

These were derived by reading `crates/core/tests/report_goldens.rs` and are class (c) — synthetic
fixtures built from a shared `base()` segment, `(0,0,0.2) → (10,0,0.2)`, which was never meant to
chain. They are listed so the implementer does not discover them and reach for the rule.

- **`continuity` fires on 5 of 7 golden fixtures**: `structural` (2 gaps), `contracts` (4),
  `retraction` (2), `first_layer` (1), `kinematics` (1). Repair: insert connecting travel segments so
  each fixture describes a real path, then regenerate. The `kinematics` fixture's arc is
  *deliberately* non-contiguous, to prove `junction-velocity` does not false-positive across a
  non-junction — a travel between them preserves that property exactly, because travel already resets
  `prev_print_end`.
- **`filament-consistency` fires on `contracts` seg 1 and seg 2**, which override `volume` (8.0, 0.4)
  while inheriting `base()`'s `filament: 0.33`, and on `structural` seg 0 if travels were included
  (they are not — the rule gates on `!travel`). Repair: set `filament` consistently in the fixture.
- **`junction-velocity` message text changes** in `conformance/reports/kinematics/verify.json`
  (25.0 → 55.9), with no change to the finding set (§3.4).
- **`Report`'s three new fields change every `conformance/reports/*/verify.json`** and
  `conformance/reports/explain/explain.json` is unaffected (it embeds `ReviewReport`, which already
  carries `segments` and `error_count` — evidence that the fields chosen in §3.5 are the ones this
  schema already considers necessary for a non-vacuous report).

## 7. What this work owes the formal artifacts

**No file under `proofs/` or `formal/` is edited by this slice.** ADR 0002 §6 makes the artifacts
authority; H1.3 records obligations against them and files them.

- **`FM1.VERIFIER_SOUNDNESS.MODEL.SEMANTICS` (`claims.toml:1410`) is untouched in scope.** Its
  exclusions already limit it to "speed, flow, retraction speed/distance, and monotonic elevation".
  H1.3 changes none of those five predicates, so `abstract = "proved"` is unaffected and the claim's
  wording stays true.
- **The audit's argument that `refinement` should read `"pending"` rather than `"not-applicable"` is
  correct, and H1.3 makes it more urgent.** Under ADR 0001, `refinement = "not-applicable"` asserts
  that no implementation refinement is *owed*. That is untenable once `Report::ok()` is used at eight
  sites as an assurance claim, and H1.3 widens the gap between the Lean predicate set (5 rules) and
  `verify.rs` (23). **This slice does not change the status.** It files the change as its own task
  with the justification above, so the registry moves through review rather than as a side effect.
- **Three new tolerance constants are numeric-boundary material.** Under ADR 0001 a tolerance-bearing
  predicate is `approximate` (`≈ε`) and the ε must be named. H1.3 owes `proofs/` one boundary entry
  each for `CONTINUITY_TOLERANCE_MM` (1e-6, hybrid), the `arc-length` relative tolerance, and the
  `filament-consistency` relative tolerance — proposed here, written in the follow-up, validated by
  `tools/validate_numeric_boundaries.py`.
- **`emit` and the new rules remain outside the claims corpus**, as the audit records. Nothing in
  this slice may be described as formally verified; the strongest available statement is
  "checked by an always-on structural rule with a named tolerance".
- Out of scope and left filed: `FM1.UNIT.NORMALIZE_CONVERT` naming `units.rs` as its Rust source when
  `units.rs` contains no unit conversion (audit §Artifact obligations).

## 8. Staging and communication of the breaking change

Any always-on rule can turn a currently-clean report red. The staging is **split by whether a
violation is demonstrably a wrong artifact**, not applied uniformly:

- **`continuity`, `negative-quantity`, `arc-length` ship at `Error` immediately.** ADR 0002's test
  applies unchanged: *nothing that previously worked is refused.* IR violating any of these produces
  an emitted program that does not describe the IR's own geometry, so the only reports that turn red
  were already wrong. Recorded as BREAKING in `CHANGELOG.md` with the predicate and tolerance spelled
  out per rule.
- **`filament-consistency` ships at `Warning` for one minor release, then `Error`.** Its
  false-positive space is genuinely open: multi-diameter or multi-material IR is unusual but not
  ill-formed, and no in-tree producer makes it, so we have no evidence either way. One release of
  observation buys that evidence. The changelog entry states the promotion release explicitly; the
  promotion is itself a BREAKING entry.
- **`bead-volume` is opt-in from birth** (contract-gated), so it breaks nobody.

**No opt-out flag.** Rejected: a `--verify-legacy-rules` escape hatch makes the strengthened `ok()`
unclaimable again — a caller could never tell which meaning it got — which is the exact defect H1.3
exists to remove. Callers who must proceed past a finding already have the tool: read `findings` and
filter by rule id.

Communication surface: `CHANGELOG.md` (BREAKING), `docs/11` §2 catalog rows and the schema section,
`docs/02-roadmap.md` (the robustness-gap risk ADR 0002 registered), and the `Report` shape note in the
SDK docs.

## 9. Binding parity

`verify` behaviour is re-exposed by surfaces that build outside the workspace, so `cargo test` will
not catch them. Each must be re-verified:

- **`crates/wasm`** — `resolveVerify` / `Design.verify()` serialize `Report` to JSON; three new keys
  appear on every call.
- **`sdk/ts`** — `src/ops.ts:136` declares `interface Report { findings: Finding[] }`; it needs the
  three fields (optional, to keep older engines readable). `test/conformance.test.ts:185` asserts
  `deepEqual(report.findings, [])` — if its fixture is discontinuous it fails on **findings**, which a
  warning-severity staging does not protect; check it explicitly.
  `test/kinematics.test.ts` and `test/parity.test.ts` filter by rule id and are safe.
- **`containers/verify-runner`** — mirrors the CLI's `report.ok()` → exit-code semantics
  (`tests/handler.rs:270`); no CI, verify locally.
- **`crates/cloud`, `py/`** — re-serialize the same `Report`; no CI for `crates/cloud`.
- **`web/viewer.js`** — renders findings; ADR 0002 §3 records that this surface has already misread a
  degenerate result once.

## 10. Validation

1. **Unit, per rule, both directions.** For each new rule: a fixture that fires it, a near-miss
   fixture just inside tolerance that does not, and — for `continuity` — one fixture per `None`
   combination `(Some,None)`, `(None,None)`, `(None,Some)` asserting no finding.
2. **Producer-invariance property tests.** `resolve`, `import_gcode`, `decode`, and each `optimize`
   pass, over the existing fixture inputs, produce toolpaths on which `continuity`,
   `arc-length` and `filament-consistency` report nothing. This is the claim §2 rests on and it must
   be a test, not a reading.
3. **Corpus probe (§5) before wiring**, output recorded in the PR.
4. **`contracts_default_evaluates_only_structural_rules`** pins the always-on id set exactly.
5. **Vacuity tests.** `segments_inspected == 0` on an empty toolpath; `rules_evaluated.len()` differs
   between `Contracts::default()` and a fully populated `Contracts`; a report round-trips through
   `serde_json` with the new fields absent (old-document compatibility).
6. **`junction-velocity` regression**: a constant-speed 90° corner fires (it does not today); a
   collinear junction with identical tangents reproduces the old scalar value exactly.
7. **`cargo test -p dry-core`, `cargo test -p dry-cli`,
   `python tools/validate_reports.py`, `python tools/validate_vectors.py conformance/vectors`,
   `tools/validate_numeric_boundaries.py`** — all green, with regenerated goldens reviewed
   line-by-line rather than blanket-accepted via `UPDATE_REPORTS=1`.
8. **Binding builds and tests** per §9, run from their own directories.

## 11. Corrections to the audit

The audit is explicitly hypotheses-to-verify. Re-checked against the code, it is wrong or imprecise in
four places:

1. **"A zero-length extruding move disables three rules at once (bead, speed, flow)."** Two, not
   always three. `bead` (`verify.rs:590`) and `speed` (`:653`) are gated on `length > 0` and are
   disabled. `flow` is *not*: `segment_motion_time` (`engine.rs:56-64`) falls back to timing against
   `|filament|` when `length` is zero, so `max-flow` still applies whenever `filament ≠ 0`. Only when
   `length == 0 ∧ filament == 0` is `flow` disabled too.
2. **"A point deposit of arbitrary volume is inspected by nothing."** Under
   `Contracts::default()`, true. But `cold-extrusion` (`:679`), `first-layer-height` and
   `first-layer-speed` (`:752`) all gate on `!travel ∧ volume > 0` with no length condition, so a
   zero-length deposit *is* inspected by three rules whenever those contracts are supplied.
3. **"Negative `width`/`height` fall through `> 0.0` guards into 'not applicable'."** False for the
   main case. `bead` tests `w <= 0.0 || h <= 0.0` (`:593`), so a negative width or height on a
   positive-length extruding move already fires `bead` today. The gap is narrower than stated: only on
   travel moves and zero-length moves are they unchecked. (Negative `length`, `volume` and `speed` are
   unchecked as described — the finding stands, with a smaller footprint.)
4. **"`Contracts::default()` leaves only 5 of 18 rules able to fail."** Correct for `Error` severity
   and therefore for `Report::ok()`; six rules can produce a *finding*, since `unmodeled-gcode` is
   always evaluated at `Warning`.

Additionally, C5's `filament ≈ volume/(πd²/4)` is **not implementable as written** — the filament
diameter is in `ResolveParams`, not in the IR (§2). §3.3 substitutes the strongest predicate the IR
actually supports and states what that leaves uncovered.

## 12. ADR

**No new ADR is warranted, and `docs/adr/README.md` is unchanged by this slice.** Every cross-cutting
decision here is an application of an ADR that already exists: §3.1's always-on/gated split and §3.6's
refusal to fabricate a `strict()` preset are ADR 0002 §4 (refuse rather than emit vacuously); §7 is
ADR 0002 §6 (artifacts are authority) and ADR 0001's layering; §3.2's tolerance reasoning is ADR 0002
§2 (no magnitude policy in disguise). What H1.3 produces that outlives it — the corpus-adjudication
procedure of §5 — is a *process* for this class of change and belongs in this spec and in the H1.3
PR template, not in a new architecture decision.

Should a future slice decide to put a filament diameter, a tool model or a material into the IR so
that the deferred half of §3.3 and all of §3.7 become checkable, **that** is an IR-scope decision and
warrants its own ADR.
