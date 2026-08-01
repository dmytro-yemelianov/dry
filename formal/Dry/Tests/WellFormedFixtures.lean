import Dry.Language.WellFormed
import Lean.Data.Json

/-!
# Executable L2 well-formedness boundary fixtures

These fixtures are evaluated by Lean and snapshotted under `proofs/fixtures/`. They exercise both sides
of the abstract validator without importing or executing `dry-core`.
-/

namespace Dry.Tests.WellFormedFixtures

open Dry.Language
open Dry.Language.L2

def finite (value : ℚ) : Number :=
  .finite value

def origin : PartialVec3 :=
  ⟨some (finite 0), some (finite 0), some (finite 0)⟩

def unitX : Vec3 :=
  ⟨finite 1, finite 0, finite 0⟩

def baseSegment : Segment :=
  {
    start := origin
    finish := ⟨some (finite 1), some (finite 0), some (finite 0)⟩
    travel := false
    speed := finite 1200
    length := finite 1
    volume := finite 1
    filament := finite 1
    width := some (finite (2 / 5))
    height := some (finite (1 / 5))
  }

def oneSegment (segment : Segment) (version : Nat := 0) : Toolpath :=
  {
    version
    metadata := none
    segments := [segment]
  }

structure Fixture where
  id : String
  toolpath : Toolpath
  expected : Except Failure Unit

def fixtures : List Fixture :=
  [
    ⟨"valid-empty", ⟨0, none, []⟩, .ok ()⟩,
    ⟨"valid-line", oneSegment baseSegment, .ok ()⟩,
    ⟨"valid-oriented-line",
      oneSegment { baseSegment with orientation := some unitX },
      .ok ()⟩,
    ⟨"valid-arc",
      oneSegment
        { baseSegment with
          kind := .arc
          centre := some ⟨finite 0, finite 1⟩
          clockwise := true },
      .ok ()⟩,
    ⟨"valid-spline",
      oneSegment
        { baseSegment with
          kind := .spline
          controlPoints := some [⟨finite 1, finite 1, finite 0⟩] },
      .ok ()⟩,
    ⟨"valid-dwell",
      oneSegment
        { baseSegment with
          kind := .dwell
          dwellSeconds := some (finite 1) },
      .ok ()⟩,
    ⟨"valid-retract", oneSegment { baseSegment with kind := .retract }, .ok ()⟩,
    ⟨"valid-unretract", oneSegment { baseSegment with kind := .unretract }, .ok ()⟩,
    ⟨"valid-deposit", oneSegment { baseSegment with kind := .deposit }, .ok ()⟩,
    ⟨"valid-manual-gcode",
      oneSegment
        { baseSegment with
          kind := .manualGcode
          manualGcode := some "M400" },
      .ok ()⟩,
    ⟨"invalid-version", oneSegment baseSegment 1,
      .error ⟨none, "version", .unsupportedVersion⟩⟩,
    ⟨"invalid-speed-nonfinite",
      oneSegment { baseSegment with speed := .nonFinite },
      .error ⟨some 0, "speed", .nonFinite⟩⟩,
    ⟨"invalid-negative-length",
      oneSegment { baseSegment with length := finite (-1) },
      .error ⟨some 0, "length", .negative⟩⟩,
    ⟨"invalid-fan-range",
      oneSegment { baseSegment with fan := some (finite 2) },
      .error ⟨some 0, "fan", .outOfRange⟩⟩,
    ⟨"invalid-line-centre",
      oneSegment { baseSegment with centre := some ⟨finite 0, finite 0⟩ },
      .error ⟨some 0, "kind_fields", .inconsistentKindFields⟩⟩,
    ⟨"invalid-arc-centre",
      oneSegment { baseSegment with kind := .arc },
      .error ⟨some 0, "kind_fields", .inconsistentKindFields⟩⟩,
    ⟨"invalid-dwell-duration",
      oneSegment { baseSegment with kind := .dwell },
      .error ⟨some 0, "kind_fields", .inconsistentKindFields⟩⟩,
    ⟨"invalid-orientation",
      oneSegment
        { baseSegment with
          orientation := some ⟨finite 1, finite 1, finite 0⟩ },
      .error ⟨some 0, "orientation", .nonUnitOrientation⟩⟩,
    ⟨"invalid-control-point",
      oneSegment
        { baseSegment with
          kind := .spline
          controlPoints := some [⟨finite 0, .nonFinite, finite 0⟩] },
      .error ⟨some 0, "control_points", .nonFinite⟩⟩
  ]

def failureCodeLabel : FailureCode → String
  | .unsupportedVersion => "unsupported-version"
  | .nonFinite => "non-finite"
  | .nonPositive => "non-positive"
  | .negative => "negative"
  | .outOfRange => "out-of-range"
  | .nonUnitOrientation => "non-unit-orientation"
  | .inconsistentKindFields => "inconsistent-kind-fields"

def locationLabel (failure : Failure) : String :=
  match failure.segmentIndex with
  | none => failure.field
  | some index => s!"segments[{index}].{failure.field}"

def renderFixture (fixture : Fixture) : String :=
  let actual := Validation.validate fixture.toolpath
  if actual != fixture.expected then
    match actual with
    | .ok () => s!"{fixture.id}\tL2\tfixture-error\tactual-valid\t-"
    | .error failure =>
        s!"{fixture.id}\tL2\tfixture-error\t{failureCodeLabel failure.code}\t{locationLabel failure}"
  else
    match actual with
    | .ok () => s!"{fixture.id}\tL2\tvalid\t-\t-"
    | .error failure =>
        s!"{fixture.id}\tL2\tinvalid\t{failureCodeLabel failure.code}\t{locationLabel failure}"

def render : String :=
  String.intercalate "\n"
    ("id\tdialect\toutcome\tcode\tlocation" :: fixtures.map renderFixture)

open Lean

def optionalJson (encode : α → Json) : Option α → Json
  | none => .null
  | some value => encode value

def listJson (encode : α → Json) (values : List α) : Json :=
  .arr (values.toArray.map encode)

def numberJson : Number → Json
  | .finite value =>
      Json.mkObj [
        ("kind", .str "finite"),
        ("numerator", .str value.num.repr),
        ("denominator", .str value.den.repr)
      ]
  | .nonFinite =>
      Json.mkObj [("kind", .str "non-finite")]

def partialVec3Json (vector : PartialVec3) : Json :=
  .arr #[
    optionalJson numberJson vector.x,
    optionalJson numberJson vector.y,
    optionalJson numberJson vector.z
  ]

def vec2Json (vector : Vec2) : Json :=
  .arr #[numberJson vector.x, numberJson vector.y]

def vec3Json (vector : Vec3) : Json :=
  .arr #[numberJson vector.x, numberJson vector.y, numberJson vector.z]

def segmentKindLabel : SegmentKind → String
  | .line => "line"
  | .arc => "arc"
  | .spline => "spline"
  | .dwell => "dwell"
  | .retract => "retract"
  | .unretract => "unretract"
  | .deposit => "deposit"
  | .manualGcode => "manualgcode"

def metadataJson (metadata : Meta) : Json :=
  Json.mkObj [
    ("generator", optionalJson Json.str metadata.generator),
    ("units", optionalJson Json.str metadata.units),
    ("source_hash", optionalJson Json.str metadata.sourceHash),
    ("invariants", listJson Json.str metadata.invariants)
  ]

def segmentJson (segment : Segment) : Json :=
  Json.mkObj [
    ("start", partialVec3Json segment.start),
    ("end", partialVec3Json segment.finish),
    ("travel", .bool segment.travel),
    ("speed", numberJson segment.speed),
    ("length", numberJson segment.length),
    ("volume", numberJson segment.volume),
    ("filament", numberJson segment.filament),
    ("width", optionalJson numberJson segment.width),
    ("height", optionalJson numberJson segment.height),
    ("kind", .str (segmentKindLabel segment.kind)),
    ("centre", optionalJson vec2Json segment.centre),
    ("clockwise", .bool segment.clockwise),
    ("temperature", optionalJson numberJson segment.temperature),
    ("fan", optionalJson numberJson segment.fan),
    ("flow", optionalJson numberJson segment.flow),
    ("tool", optionalJson (fun value => .num value) segment.tool),
    ("dwell_s", optionalJson numberJson segment.dwellSeconds),
    ("manual_gcode", optionalJson Json.str segment.manualGcode),
    ("orientation", optionalJson vec3Json segment.orientation),
    ("control_points", optionalJson (listJson vec3Json) segment.controlPoints)
  ]

def toolpathJson (toolpath : Toolpath) : Json :=
  Json.mkObj [
    ("version", .num toolpath.version),
    ("meta", optionalJson metadataJson toolpath.metadata),
    ("segments", listJson segmentJson toolpath.segments)
  ]

def expectedJson : Except Failure Unit → Json
  | .ok () =>
      Json.mkObj [("outcome", .str "valid")]
  | .error failure =>
      Json.mkObj [
        ("outcome", .str "invalid"),
        ("code", .str (failureCodeLabel failure.code)),
        ("segment_index", optionalJson (fun value => .num value) failure.segmentIndex),
        ("field", .str failure.field)
      ]

def fixtureJson (fixture : Fixture) : Json :=
  Json.mkObj [
    ("id", .str fixture.id),
    ("dialect", .str "L2"),
    ("toolpath", toolpathJson fixture.toolpath),
    ("expected", expectedJson fixture.expected)
  ]

def fixtureDocument : Json :=
  Json.mkObj [
    ("schema_version", .num 1),
    ("model", .str "dry-ir-v0-l2-logical"),
    ("cases", listJson fixtureJson fixtures)
  ]

def renderJson : String :=
  Json.pretty fixtureDocument 100

end Dry.Tests.WellFormedFixtures

def main (arguments : List String) : IO Unit := do
  if arguments.contains "--json" then
    IO.println Dry.Tests.WellFormedFixtures.renderJson
  else
    IO.println Dry.Tests.WellFormedFixtures.render
