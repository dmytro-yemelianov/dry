import Dry.Language.WellFormed

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

end Dry.Tests.WellFormedFixtures

def main : IO Unit :=
  IO.println Dry.Tests.WellFormedFixtures.render
