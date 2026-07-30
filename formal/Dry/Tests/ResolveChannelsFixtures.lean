import Dry.Semantics.ResolveChannels
import Lean.Data.Json

namespace Dry.Tests.ResolveChannelsFixtures

open Dry.Language
open Dry.Language.L2
open Dry.Semantics.ResolveChannels
open Lean

structure FixtureCase where
  id : String
  ops : List Op
deriving DecidableEq, Repr

def cases : List FixtureCase := [
  { id := "default-channels",
    ops := [.move ⟨some (.finite 10), some (.finite 0), some (.finite 0)⟩ (.finite 30)] },
  { id := "temp-fan-tool-propagation",
    ops := [
      .temperature (.finite 210),
      .fan (.finite 1),
      .tool 0,
      .move ⟨some (.finite 10), some (.finite 0), some (.finite 0)⟩ (.finite 30),
      .move ⟨some (.finite 20), some (.finite 0), some (.finite 0)⟩ (.finite 30)
    ] },
  { id := "non-default-flow-propagation",
    ops := [
      .flow (.finite (5/4)),
      .move ⟨some (.finite 10), some (.finite 0), some (.finite 0)⟩ (.finite 30)
    ] },
  { id := "default-flow-omitted",
    ops := [
      .flow (.finite 1),
      .move ⟨some (.finite 10), some (.finite 0), some (.finite 0)⟩ (.finite 30)
    ] },
  { id := "dwell-inherits-channels",
    ops := [
      .temperature (.finite 220),
      .dwell (.finite 5)
    ] },
  { id := "channel-overwrite",
    ops := [
      .temperature (.finite 200),
      .move ⟨some (.finite 10), some (.finite 0), some (.finite 0)⟩ (.finite 30),
      .temperature (.finite 210),
      .move ⟨some (.finite 20), some (.finite 0), some (.finite 0)⟩ (.finite 30)
    ] }
]

def numberToJson (n : Number) : Json :=
  match n with
  | .nonFinite => Json.null
  | .finite x =>
      let rat : Rat := x
      if rat.den = 1 then
        Json.num (JsonNumber.fromInt rat.num)
      else
        Json.mkObj [
          ("numerator", Json.num (JsonNumber.fromInt rat.num)),
          ("denominator", Json.num (JsonNumber.fromNat rat.den))
        ]

def optionToJson {α : Type} (f : α → Json) (opt : Option α) : Json :=
  match opt with
  | none => Json.null
  | some val => f val

def pointToJson (p : PartialVec3) : Json :=
  Json.mkObj [
    ("x", optionToJson numberToJson p.x),
    ("y", optionToJson numberToJson p.y),
    ("z", optionToJson numberToJson p.z)
  ]

def opToJson (op : Op) : Json :=
  match op with
  | .move p speed =>
      Json.mkObj [
        ("type", Json.str "move"),
        ("finish", pointToJson p),
        ("speed", numberToJson speed)
      ]
  | .dwell s =>
      Json.mkObj [
        ("type", Json.str "dwell"),
        ("seconds", numberToJson s)
      ]
  | .temperature t =>
      Json.mkObj [
        ("type", Json.str "temperature"),
        ("nozzle", numberToJson t)
      ]
  | .fan f =>
      Json.mkObj [
        ("type", Json.str "fan"),
        ("speed", numberToJson f)
      ]
  | .flow r =>
      Json.mkObj [
        ("type", Json.str "flow"),
        ("ratio", numberToJson r)
      ]
  | .tool idx =>
      Json.mkObj [
        ("type", Json.str "tool"),
        ("index", Json.num (JsonNumber.fromNat idx))
      ]

def kindToString (k : SegmentKind) : String :=
  match k with
  | .line => "line"
  | .arc => "arc"
  | .spline => "spline"
  | .dwell => "dwell"
  | .retract => "retract"
  | .unretract => "unretract"
  | .deposit => "deposit"
  | .manualGcode => "manualGcode"

def segmentToJson (seg : Segment) : Json :=
  Json.mkObj [
    ("kind", Json.str (kindToString seg.kind)),
    ("temperature", optionToJson numberToJson seg.temperature),
    ("fan", optionToJson numberToJson seg.fan),
    ("flow", optionToJson numberToJson seg.flow),
    ("tool", optionToJson (fun idx => Json.num (JsonNumber.fromNat idx)) seg.tool),
    ("dwell_seconds", optionToJson numberToJson seg.dwellSeconds)
  ]

def evaluateCase (c : FixtureCase) : Json :=
  let (_, segs) := resolve (initialState ⟨some (.finite 0), some (.finite 0), some (.finite 0)⟩) c.ops
  Json.mkObj [
    ("id", Json.str c.id),
    ("ops", Json.arr (c.ops.map opToJson).toArray),
    ("expected", Json.mkObj [
      ("emitted_count", Json.num (JsonNumber.fromNat segs.length)),
      ("segments", Json.arr (segs.map segmentToJson).toArray)
    ])
  ]

def document : Json :=
  Json.mkObj [
    ("schema_version", Json.num 1),
    ("model", Json.str "resolve-channels-refinement-v0"),
    ("model_checks", Json.bool true),
    ("cases", Json.arr (cases.map evaluateCase).toArray)
  ]

def resolveChannelsFixtureChecks : Bool :=
  decide (cases.length = 6)

theorem resolveChannelsFixtureChecks_theorem : resolveChannelsFixtureChecks = true := by
  rfl

def main : IO Unit := do
  if ¬resolveChannelsFixtureChecks then
    throw (IO.userError "resolveChannelsFixtureChecks failed")
  IO.println document.compress

end Dry.Tests.ResolveChannelsFixtures

def main : IO Unit := Dry.Tests.ResolveChannelsFixtures.main
