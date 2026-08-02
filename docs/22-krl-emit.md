# KRL (KUKA robot) emission

`dry emit --format krl` writes a KUKA Robot Language module. This page records **what that claim
covers**, the grammar the output is checked against, the conventions it follows and where they come
from, and what it still does not do.

Implementation: [`crates/core/src/emit/krl.rs`](../crates/core/src/emit/krl.rs). Structural tests:
[`crates/core/tests/krl_program_structure.rs`](../crates/core/tests/krl_program_structure.rs).
External check: [`tools/krl_check.sh`](../tools/krl_check.sh).

## The honest boundary

**Dry's KRL output has never been executed on a KUKA controller or on a KUKA simulator.** Nothing in
this repository establishes that a KRC will load or run it. Two weaker things *are* established:

1. **The structure is what the BNF below says it is** — asserted by the tests and frozen as a golden
   at `conformance/reports/robot/reference-five-axis.src`.
2. **The syntax of the golden is accepted by a KRL grammar nobody here wrote** — `tools/krl_check.sh`
   parses `conformance/reports/robot/*.src` with `kuka/krl.g4` from [`antlr/grammars-v4`][g4], pinned
   by commit and SHA-256, in CI (the `krl` job).

Note the scope of (2): **the golden**, not every program Dry emits. The emitter runs no grammar, and
`krl_check.sh` is a separate command over a fixed corpus. A program you emit today has not been
parsed by anything, which is why the banner Dry writes into every file says *checkable with*
`tools/krl_check.sh` rather than *checked against* it. Point it at your own `.src` to change that.

The distinction matters because the previous claim was neither. Until this change, "valid KRL" meant
"Dry's own g-code parser round-tripped it", which is circular: the emitter and the parser were built
together, so their agreement measures nothing about KUKA. That round-trip was only ever possible
because the output was not KRL — it was g-code words with `G0`/`G1`/`G2` swapped for `PTP`/`LIN`/`CIRC`
and `I`/`J` swapped for a Dry-invented `C`/`D`.

### What was searched for, and what exists

The issue asked for an external grammar check, on the assumption none would be obtainable. The search
found more than expected, and the results are recorded here so nobody repeats it:

| Candidate | What it is | Usable as an oracle? |
|---|---|---|
| [`antlr/grammars-v4` `kuka/krl.g4`][g4] | ANTLR4 grammar, LGPL-3.0-or-later. Written by Jan Schlößin (2010–2011) for the reverse-engineering study [arXiv:1009.5004][paper]; ported to ANTLR4 by Tom Everett (2016). | **Yes — this is what is wired in.** Covers `DEF`/`END`, `DEFDAT`, `DEFFCT`, `PTP`/`LIN`/`CIRC` (and the `_REL` forms), struct aggregates, `$`-prefixed system variables, `WAIT SEC`, `;` comments and `&` header lines. |
| [`krllint`](https://pypi.org/project/krllint/) (PyPI, MIT) | Style checker and autofixer for `.src` files. | No. It lints conventions (line length, naming, whitespace); it is not a grammar and does not decide well-formedness. |
| [`afeldman/kuka-krl-compiler`](https://github.com/afeldman/kuka-krl-compiler) | BNFC grammar plus a Haskell front end and LLVM back end. | Not wired in — a second grammar behind a Haskell/BNFC/LLVM toolchain, for no additional discrimination over the ANTLR one. Worth revisiting only as a cross-check. |
| [`OpenKuka/KRL`](https://github.com/OpenKuka/KRL), [`JavaKUKA`](https://github.com/whitegreen/JavaKUKA) | C# / Java libraries that *generate* KRL. | No. Generators, not validators. |
| KUKA OfficeLite / WorkVisual | The real thing. | No. Proprietary, licensed, Windows-only. Nothing free executes KRL. |

So: a syntax oracle exists and is now used; an **execution** oracle does not exist and no claim
depends on one.

[g4]: https://github.com/antlr/grammars-v4/blob/master/kuka/krl.g4
[paper]: https://arxiv.org/abs/1009.5004

### Running the check

```sh
tools/krl_check.sh                     # the goldens under conformance/reports/robot/
tools/krl_check.sh path/to/part.src    # anything else
```

It needs a working Java runtime (for the ANTLR tool) and `antlr4-python3-runtime`. On macOS
`/usr/bin/java` is a stub with no JRE behind it, so `brew install openjdk` or set `JAVA_BIN`. It runs
in CI as the `krl` job, alongside [`tools/linuxcnc_check.sh`](../tools/linuxcnc_check.sh) — the
RS-274 equivalent, which runs the LinuxCNC `rs274` interpreter as the `linuxcnc` job. Both are
oracles with heavier environments than the rest of the gates (a JRE here, a Debian container there),
and both treat *parsed cleanly but contains no motion instruction* as a rejection.

That last rule is about the oracle's usefulness, not about KRL: a module whose only statement is a
`WAIT SEC` is perfectly good KRL, and the grammar accepts it. So a dwell-only or empty IR emits a
module `krl_check.sh` will report as REJECTED — correctly for a golden, which is meant to exercise
motion, and misleadingly for anything else. Both goldens under `conformance/reports/robot/` contain
motion.

Both the grammar and the ANTLR tool jar are fetched at check time from pinned URLs with their
SHA-256 verified, rather than vendored: the grammar is LGPL and this repository is proprietary, and
an in-tree copy would be a copy we could edit. An oracle we can edit is not an oracle — and an oracle
whose *parser generator* arrives unverified is not one either, which is why the jar is pinned too
(cross-checked against Maven Central's published `.sha1`) and cached under `$XDG_CACHE_HOME/dry`
rather than written into the Maven repository layout.

## The subset Dry emits

```text
program        ::= "DEF" name "(" ")" NL comment* frame_setup statement* "END" NL
frame_setup    ::= tool_line base_line
tool_line      ::= "$TOOL" "=" frame NL
base_line      ::= "$BASE" "=" frame NL
statement      ::= apo_line | vel_line | motion | dwell
apo_line       ::= "$APO.CDIS" "=" real NL
vel_line       ::= "$VEL.CP" "=" real NL
motion         ::= ptp | lin | circ
ptp            ::= "PTP" pose NL
lin            ::= "LIN" pose approx? NL
circ           ::= "CIRC" pose "," pose approx? NL
dwell          ::= "WAIT" "SEC" real NL
approx         ::= "C_DIS"
pose           ::= "{" "E6POS" ":" component ("," component)* "}"
component      ::= ("X"|"Y"|"Z"|"A"|"B"|"C") real
frame          ::= "{" component ("," component)* "}"
real           ::= "-"? digit+ "." digit*
comment        ::= ";" <any text to end of line> NL
name           ::= (letter|"_") (letter|digit|"_"){0,23}
```

Two placement rules the grammar does not express: `apo_line` appears at most once per program and
only immediately before the first `lin`/`circ`, and `vel_line` only immediately before a `lin`/`circ`
whose velocity differs from the last one written. Both keep the program free of state that no
instruction goes on to reference (ADR 0002 §4).

There is **no passthrough production**. A `manualgcode` segment carries verbatim *g-code* by
definition (`spec/dry-ir-v0.schema.json`, `docs/10-dry-ir-v0-spec.md` § `manual_gcode`), so it is
provably not a KRL statement and is refused rather than copied — copying it produced a `DEF`/`END`
module with `M117 hello` in the body, which the external grammar rejects at `line 7:5`.

A worked example — the frozen golden, one instance of every construct:

```krl
DEF dry ( )
;  Emitted by dry: never run on a KUKA controller or simulator.
;  The structure of THIS program has not been checked either -- dry emits KRL,
;  it does not parse it. tools/krl_check.sh checks a file against an external
;  KRL grammar that nobody here wrote.
;  PTP speed is $VEL_AXIS[] (percent of maximum), which dry does not set.
  $TOOL = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}
  $BASE = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}
  PTP {E6POS: X 10.0, Y 0.0, Z 5.0, A 0.0, B 0.0, C 180.0}
  $VEL.CP = 0.02
  LIN {E6POS: X 20.0, Y 0.0, Z 5.0}
  CIRC {E6POS: X 27.071068, Y 2.928932, Z 5.0}, {E6POS: X 30.0, Y 10.0, Z 5.0, B 36.869898}
  WAIT SEC 1.5
  $VEL.CP = 0.01
  LIN {E6POS: X 30.0, Y 20.0, Z 5.0, A -90.0, B 90.0}
END
```

A component absent from an aggregate keeps its current value on the controller, which is the same
modality g-code gives an omitted axis word — so `LIN {E6POS: Y 20.0}` moves in Y and holds everything
else, and a 3-axis emit (no `--five-axis`) states no orientation at all and leaves the robot's alone.

**Which components are stated is decided by exactly the rule in
[`emit/gcode.rs`](../crates/core/src/emit/gcode.rs)**, so a word the g-code renderer writes is a
component this one writes: under `--five-axis`, an axis that *changed* or that the segment *named*;
in 3-axis mode, an axis the segment named that also changed (plus X and Y on a `CIRC`). The
five-axis branch is why the golden above restates `Y 0.0, Z 5.0` on a move that only changed X: it is
also what makes a segment like `start: [50, 0, 0], end: [null, 10, null]` emit its inherited `X 50.0`
instead of walking the arm 40 mm to the wrong place.

## Conventions, and their sources

### Orientation: `A`/`B`/`C` are ZYX Euler angles

KUKA writes an orientation as Euler angles applied to the **moving** frame: `A` about Z, then `B`
about the new Y, then `C` about the resulting X — `R = Rz(A)·Ry(B)·Rx(C)`. Sources: the KUKA System
Software programming manual's "Euler angles ZYX" description, restated in
[RoboDK's *Robot Euler Angles*](https://robodk.com/blog/robot-euler-angles/) and
[Mecademic's orientation tutorial](https://mecademic.com/insights/academic-tutorials/space-orientation-euler-angles/).

`Segment.orientation` is a unit **tool-axis** vector, with `[0, 0, 1]` the untilted tool pointing
*away* from the work. A KUKA flange's Z axis points the other way — out of the flange, along the tool,
toward the work — so the emitted pose must satisfy `R·[0,0,1] = −d`. Pinning the roll at `C = 180°`
solves that exactly:

```text
A = atan2(j, i)      B = acos(k)      C = 180
```

which is `Kinematics::Bc` with zero offsets — already implemented in
[`crates/core/src/emit/kinematics.rs`](../crates/core/src/emit/kinematics.rs). The KRL renderer
resolves through it rather than re-deriving the trig, and so inherits its **singular-cone hold**: at
`d = ±Z` the `A` angle is undetermined and is held at its previous value instead of being swung to
`atan2(0, 0) = 0`. The untilted tool comes out as `A 0, B 0, C 180`, the canonical KUKA
tool-pointing-down pose — which is the sanity check that the sign convention is the right way round.

Two deliberate choices:

- **`C` is a choice, not a measurement.** Dry's IR carries no roll about the tool axis. `180°` is
  picked because it makes the untilted case canonical; any roll would describe the same tool
  direction.
- **`B` is not folded into `[−90°, 90°]`.** KUKA's standardised readback interval for `B` is
  `[−90°, 90°]`, and `B = acos(k)` reaches `180°` when the tool axis points below horizontal. The
  fold `(A, B, C) ≡ (A±180, 180−B, C±180)` is exact, but it is discontinuous at `B = 90°`: applying
  it would make `A` jump half a turn because the tool crossed horizontal, commanding a reorientation
  the geometry never asked for. A controller normalises on readback; a program does not have to.

### Velocity: `$VEL.CP` is m/s, and does not govern `PTP`

`$VEL.CP` is the Cartesian path velocity in **metres per second**; `Feedrate` is mm/min, so the
conversion is `f / 60000` (1200 mm/min → `0.02`). It is written before `LIN` and `CIRC` and, modally,
only when it changes.

**`PTP` velocity comes from `$VEL_AXIS[]`, a percentage of each joint's maximum, and Dry does not set
it.** A Cartesian feedrate cannot be converted to joint percentages without the robot's joint-rate
data, which the IR does not carry, so a `PTP` here runs at whatever the controller was last told. The
emitted banner says so in the program itself, because the file is what reaches an operator.

A CP move whose feedrate is zero or negative is **refused**, not emitted: `$VEL.CP = 0` is a
controller fault, and `emit` is the last gate before a machine (ADR 0002 §4). So is one that *prints*
as zero — `$VEL.CP` is written to 12 decimal places, so any feedrate below half an ulp of that
(`3e-8 mm/min`, i.e. `5e-13 m/s`) would render as the literal `0.0` however positive the input was.
The guard reads the formatted literal, not only the number it came from; the comparison is exact and
carries no epsilon.

A non-finite feedrate is refused on **`PTP` as well**, even though a `PTP` states no speed at all in
the program. Every g-code flavor refuses a `NaN` feedrate on a rapid because it writes an `F` word
there; KRL writing nothing must not therefore be a wider gate than the rest.

### `$TOOL` / `$BASE` are pinned, and the default is the flange

Both are always written, defaulting to identity. Leaving them unstated would run the program against
whatever tool and base the operator last selected — the same hazard `CncFrame` closes by always
writing a `G54`.

**The identity `$TOOL` puts the TCP at the flange**, so an emitted `X`/`Y`/`Z` is a flange point and
the tool's own length is not accounted for anywhere. That is the truthful default (Dry has no robot
tool-geometry model) and it is what makes the `A`/`B`/`C` interpretation above well-defined, but a
real deployment must supply `KrlFrame::tool` with the flange→TCP transform.

### `$APO.CDIS` is emitted only when there is one to state

Approximation (corner blending) is `$APO.CDIS` plus a `C_DIS` suffix on the instruction, and both
appear only when the caller supplies `KrlFrame::approx_mm` **and** the program goes on to contain a
`LIN` or `CIRC` to carry the `C_DIS`. Absent either, every motion is exact positioning and no `$APO`
line is written: an approximation distance no instruction references would be a vacuous emission
(ADR 0002 §4). That is why the line is written lazily, immediately before the first CP instruction,
rather than in the frame prologue — a PTP-only or dwell-only program would otherwise have set a
blending distance that nothing in it could use. `PTP` blending is a different pair (`C_PTP` with
`$APO.CPTP` in percent) and is not emitted.

### `CIRC` takes an auxiliary point, not a centre offset

Real KRL is `CIRC aux, end`: an auxiliary point on the arc and the end point. Dry places the
auxiliary point at the **midpoint of the swept angle**, which fixes the plane and the direction of
travel — so clockwise and counter-clockwise arcs no longer emit the same instruction, closing the
audit finding that `CIRC` lost arc direction.

`CA` (circular angle) is deliberately not emitted. Three points already determine the sweep, and `CA`
would *override* the programmed end point rather than confirm it.

Three arc shapes are **refused** rather than approximated, because a three-point `CIRC` cannot
express them:

- a **helix** (`start.Z ≠ end.Z`) — a `CIRC` through three points is planar;
- a **full turn** (`end` = `start`) — the three points would be two, and the circle underdetermined;
- a **zero radius**.

Arc *radius consistency* (`|end − centre|` vs `|start − centre|`) is **not** re-checked here, and
what that does and does not buy is worth stating exactly, because an earlier draft of this page
overstated it.

The epsilon is `ARC_RADIUS_TOLERANCE_MM`. It is one constant, defined in
[`crates/core/src/verify.rs`](../crates/core/src/verify.rs) and imported by `resolve.rs` rather than
restated there, applied by `verify`'s `arc-radius` rule and by the L1 arc gate, and published as the
boundary `FM1.F64.VERIFY.ARC_RADIUS` in
[`proofs/verify-numeric-boundaries-v0.toml`](../proofs/verify-numeric-boundaries-v0.toml) with the
budget `…BUDGET.ARC_RADIUS_RELATIVE_ERROR` pinned against it by
`tools/validate_numeric_boundaries.py`. Re-applying it in the emitter would be a third application on
a third input domain, which under ADR 0001 needs a boundary entry of its own; this renderer defers
instead.

**The deferral is not a guarantee on the emit path.** `dry emit` never runs the verifier (ADR 0002
§1), and a design authored as IR JSON never passes the L1 gate either — so an arc whose endpoint is
off the radius reaches a KRL program unchecked unless you also ran `dry verify`. Dry emits the
auxiliary point on the *start* radius, and the circle KUKA fits through the three points is then not
the circle the IR described. This is the target where that is least recoverable: RS-274 keeps the
IR's own centre in `I`/`J`, so a downstream interpreter can still see the contradiction, whereas a
refitted three-point circle carries no trace of it. Listed below as a limitation, not as a solved
problem.

## What this still does not do

- **No execution evidence of any kind.** See the top of this page.
- **No `PTP` velocity**, per above.
- **Arc-radius consistency is not checked at emit.** Per the section above: `verify` owns the rule
  and the published epsilon, `dry emit` does not run `verify`, and a KRL `CIRC` loses the evidence.
  Run `dry verify` on IR you did not author.
- **No motion-parameter initialisation.** A hand-written or vendor-generated KRL module normally
  opens with `INI` (which expands to `BAS(#INITMOV, 0)` and sets `$ACC.CP`, `$APO.CPTP`, `$ORI_TYPE`
  and the rest to defaults). Dry writes none of it, so every motion parameter it does not set
  explicitly — acceleration above all — is whatever the previously executed program left behind. The
  external grammar accepts the module without `INI`; a controller running it at someone else's
  acceleration is a different question, and one nothing here answers.
- **A `manualgcode` segment is refused, not passed through.** Every g-code flavor copies that field
  verbatim; KRL cannot, because the field is defined as g-code. There is no way to inject raw KRL
  text into a program.
- **A segment that commands no pose change is refused.** A retract, an unretract or a stationary
  deposit is filament motion, and a KRL program has no axis for filament — so once the pose is
  known, such a segment has nothing left to state. It used to restate the pose the robot was already
  at, which fabricated a zero-distance move (carrying `C_DIS`, if blending was on). In practice this
  means FDM IR containing retracts cannot be emitted as KRL at all, which is the honest outcome:
  extrusion is dropped silently on every *other* segment too (see the next bullet), and refusing is
  the loud half of that.
- **Extrusion is not carried.** No `E` axis, no analogue of one: `volume`, `filament` and `flow` are
  read only to decide whether a move is a rapid. A KRL program from extruding IR describes the
  motion and nothing else.
- **An empty or motion-free IR still emits a module.** `DEF`/banner/`$TOOL`/`$BASE`/`END` with no
  instruction between them, where g-code emits an empty file. `tools/krl_check.sh` reports such a
  file as REJECTED (see "Running the check") even though the grammar accepts it.
- **No `.dat` companion, no `&ACCESS`/`&REL` header**, no `S`/`T` status/turn components on the
  poses, no `FOLD` structure, no interrupt/tool-change/IO logic. A `PTP` with no status and turn
  keeps the arm configuration the robot is currently in.
- **No spindle, laser or process power channel** — Dry has no power channel on any target yet.
- **The `DEF` name is not derived from the output file name.** KUKA requires them to match; Dry
  defaults to `dry` and accepts an explicit `KrlFrame::program_name` (validated as a KRL identifier,
  refused rather than sanitised). Nothing wires that from the CLI or profile yet, so a program
  written to `bracket.src` will still say `DEF dry`.
- **No profile plumbing.** `machine` has no KRL block, so `$TOOL`/`$BASE`/`$APO`/name are reachable
  only through the core `EmitParams` API, not from a profile JSON or a CLI flag.
- **Not reachable from the Python or TypeScript SDKs**, which is a P5.4 gap for GRBL too.
- **`rewrite-gcode` refuses a KRL flavor.** The renderer emits a whole `DEF`/`END` module, which
  cannot be spliced into a g-code file span by span; the source-preserving rewrite path says so
  instead of producing a hybrid.
- **Near-full-turn arcs are not refused**, only exactly-full ones. An arc sweeping `2π − ε` puts its
  end point arbitrarily close to its start, and the circle KUKA fits through three near-coincident
  points is numerically poor. Refusing that needs a tolerance, which needs a `proofs/` entry.
