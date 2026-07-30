import Mathlib.Data.Rat.Defs
import Mathlib.Tactic

/-!
# Abstract Field-Preserving Codec Model (FM1.7)

This module formalizes a serializer-neutral, field-preserving encoding model:
- `encodeSegment` / `decodeSegment`;
- `encodeToolpath` / `decodeToolpath`;
- Proves exact round-trip inverse theorem: $\text{decode}(\text{encode}(s)) = \text{some } s$.

It intentionally does not model JSON syntax, DEFLATE, DRY0 columns, DRY1 chunks,
malformed bytes, version migration, or resource bounds.
-/

namespace Dry.Semantics.CodecInverse

structure Segment where
  travel : Bool
  length : ℚ
  speed : ℚ
deriving DecidableEq, Repr

structure EncodedSegment where
  travel : Bool
  length : ℚ
  speed : ℚ
deriving DecidableEq, Repr

def encodeSegment (s : Segment) : EncodedSegment :=
  { travel := s.travel, length := s.length, speed := s.speed }

def decodeSegment (e : EncodedSegment) : Option Segment :=
  some { travel := e.travel, length := e.length, speed := e.speed }

theorem decode_encode_segment_inverse (s : Segment) :
    decodeSegment (encodeSegment s) = some s := by
  rfl

def encodeToolpath (segs : List Segment) : List EncodedSegment :=
  segs.map encodeSegment

def decodeToolpath (encoded : List EncodedSegment) : Option (List Segment) :=
  encoded.mapM decodeSegment

theorem decode_encode_toolpath_inverse (segs : List Segment) :
    decodeToolpath (encodeToolpath segs) = some segs := by
  induction segs with
  | nil => rfl
  | cons head tail ih =>
      rw [encodeToolpath, List.map_cons]
      rw [decodeToolpath, List.mapM_cons]
      rw [decode_encode_segment_inverse]
      dsimp
      rw [← decodeToolpath, ← encodeToolpath, ih]
      rfl

end Dry.Semantics.CodecInverse
