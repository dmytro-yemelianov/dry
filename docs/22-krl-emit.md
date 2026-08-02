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
2. **The syntax is accepted by a KRL grammar nobody here wrote** — `tools/krl_check.sh` parses the
   golden with `kuka/krl.g4` from [`antlr/grammars-v4`][g4], pinned by commit and SHA-256.

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
`/usr/bin/java` is a stub with no JRE behind it, so `brew install openjdk` or set `JAVA_BIN`. Like
[`tools/linuxcnc_check.sh`](../tools/linuxcnc_check.sh) — the RS-274 equivalent, which runs the
LinuxCNC `rs274` interpreter — it is a **local/manual** oracle with a heavier environment than the
default CI gates, and it treats *parsed cleanly but contains no motion instruction* as a rejection.

The grammar is fetched at check time from a pinned commit with its SHA-256 verified, rather than
vendored: it is LGPL and this repository is proprietary, and an in-tree copy would be a copy we could
edit. An oracle we can edit is not an oracle.

## The subset Dry emits

```text
program        ::= "DEF" name "(" ")" NL comment* frame_setup statement* "END" NL
frame_setup    ::= tool_line base_line apo_line?
tool_line      ::= "$TOOL" "=" frame NL
base_line      ::= "$BASE" "=" frame NL
apo_line       ::= "$APO.CDIS" "=" real NL
statement      ::= vel_line | motion | dwell | passthrough
vel_line       ::= "$VEL.CP" "=" real NL
motion         ::= ptp | lin | circ
ptp            ::= "PTP" pose NL
lin            ::= "LIN" pose approx? NL
circ           ::= "CIRC" pose "," pose approx? NL
approx         ::= "C_DIS"
pose           ::= "{" "E6POS" ":" component ("," component)* "}"
component      ::= ("X"|"Y"|"Z"|"A"|"B"|"C") real
frame          ::= "{" component ("," component)* "}"
real           ::= "-"? digit+ "." digit*
comment        ::= ";" <any text to end of line> NL
name           ::= (letter|"_") (letter|digit|"_"){0,23}
```

`passthrough` is a `manualgcode` segment, copied verbatim. Dry cannot tell whether its text is KRL;
`tools/krl_check.sh` is what surfaces a g-code line smuggled into a robot program.

A worked example — the frozen golden, one instance of every construct:

```krl
DEF dry ( )
;  Emitted by dry. Structure checked against an external KRL grammar
;  (tools/krl_check.sh); never run on a KUKA controller or simulator.
;  PTP speed is $VEL_AXIS[] (percent of maximum), which dry does not set.
  $TOOL = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}
  $BASE = {X 0.0, Y 0.0, Z 0.0, A 0.0, B 0.0, C 0.0}
  PTP {E6POS: X 10.0, Y 0.0, Z 5.0, A 0.0, B 0.0, C 180.0}
  $VEL.CP = 0.02
  LIN {E6POS: X 20.0}
  CIRC {E6POS: X 27.071068, Y 2.928932, Z 5.0}, {E6POS: X 30.0, Y 10.0, B 36.869898}
  WAIT SEC 1.5
  $VEL.CP = 0.01
  LIN {E6POS: Y 20.0, A -90.0, B 90.0}
END
```

A component absent from an aggregate keeps its current value on the controller, which is the same
modality g-code gives an omitted axis word — so `LIN {E6POS: X 20.0}` moves in X and holds everything
else, and a 3-axis emit (no `--five-axis`) states no orientation at all and leaves the robot's alone.

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
controller fault, and `emit` is the last gate before a machine (ADR 0002 §4).

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
appear only when the caller supplies `KrlFrame::approx_mm`. Absent it, every motion is exact
positioning and no `$APO` line is written: an approximation distance no instruction references would
be a vacuous emission (ADR 0002 §4). `PTP` blending is a different pair (`C_PTP` with `$APO.CPTP` in
percent) and is not emitted.

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

Arc *radius consistency* (`|end − centre|` vs `|start − centre|`) is not re-checked here: it is a
tolerance-bearing question and `verify` already owns it as the `arc-radius` rule with its published
epsilon. An arc that fails that rule emits an auxiliary point on the *start* radius, and the circle
KUKA fits through the three points is then not the circle the IR described.

## What this still does not do

- **No execution evidence of any kind.** See the top of this page.
- **No `PTP` velocity**, per above.
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
