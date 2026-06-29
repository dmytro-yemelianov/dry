#!/usr/bin/env python3
"""Independent validator for the public Dry IR v0 conformance vectors.

This is a *second implementation* of the Dry IR v0 codec — it reads and writes the JSON wire form and
the DRY0 / DRY1 binary encodings using only the Python standard library (``zlib`` + ``struct`` + ``json``)
plus ``jsonschema`` for schema validation. It does NOT depend on ``dry-core``. Its purpose is to prove
the spec (``docs/10-dry-ir-v0-spec.md``) is self-sufficient: an external implementation can round-trip
every vector from the prose + schema alone.

Conformance is checked *semantically* (exact f64 bit-equality, structural equality), never by
cross-language byte-identity — DEFLATE output and float formatting are implementation-defined (spec §9).

Usage:
    python tools/validate_vectors.py [conformance/vectors]
"""

from __future__ import annotations

import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path

# ---------------------------------------------------------------------------
# Kind encodings (spec §3.4) — note the three-way asymmetry, all mapped to one canonical id.
# ---------------------------------------------------------------------------
CANON_KINDS = [
    "line",
    "arc",
    "spline",
    "dwell",
    "retract",
    "unretract",
    "deposit",
    "manualgcode",
]
JSON_KIND = {name: i for i, name in enumerate(CANON_KINDS)}
DRY0_DICT_STR = [
    "line",
    "arc",
    "spline",
    "dwell",
    "retract",
    "unretract",
    "deposit",
    "manual_gcode",
]
DRY0_DICT_TO_ID = {s: i for i, s in enumerate(DRY0_DICT_STR)}

# DRY1 segment-row flag bits (spec §6.3).
F_TRAVEL = 1 << 0
F_CLOCKWISE = 1 << 1
F_START = [1 << 2, 1 << 3, 1 << 4]
F_END = [1 << 5, 1 << 6, 1 << 7]
F_WIDTH = 1 << 8
F_HEIGHT = 1 << 9
F_CENTRE = 1 << 10
F_TEMPERATURE = 1 << 11
F_FAN = 1 << 12
F_FLOW = 1 << 13
F_DWELL = 1 << 14
F_TOOL = 1 << 15
F_ORIENTATION = 1 << 16
F_CONTROL_POINTS = 1 << 17
F_MANUAL_GCODE = 1 << 18
KNOWN_FLAGS = (1 << 19) - 1
LEGACY_KNOWN_FLAGS = (1 << 18) - 1

DRY0_MAGIC = b"DRY0"
DRY1_MAGIC = b"DRY1"


class DecodeError(Exception):
    """Any malformed / unsupported input — the documented rejection (spec §11)."""


# ---------------------------------------------------------------------------
# Low-level helpers
# ---------------------------------------------------------------------------
def raw_inflate(data: bytes) -> bytes:
    try:
        return zlib.decompress(data, -15)
    except zlib.error as exc:  # truncated / corrupt stream
        raise DecodeError(f"inflate failed: {exc}") from exc


def raw_deflate(data: bytes) -> bytes:
    c = zlib.compressobj(8, zlib.DEFLATED, -15)
    return c.compress(data) + c.flush()


class Reader:
    def __init__(self, buf: bytes):
        self.buf = buf
        self.at = 0

    def take(self, n: int) -> bytes:
        if self.at + n > len(self.buf):
            raise DecodeError("unexpected end of input (truncated)")
        out = self.buf[self.at : self.at + n]
        self.at += n
        return out

    def u8(self) -> int:
        return self.take(1)[0]

    def u32(self) -> int:
        return struct.unpack("<I", self.take(4))[0]

    def f64(self) -> float:
        return struct.unpack("<d", self.take(8))[0]

    def bits(self, n: int) -> list[bool]:
        nbytes = (n + 7) // 8
        raw = self.take(nbytes)
        return [bool((raw[i // 8] >> (i % 8)) & 1) for i in range(n)]


def fkey(x):
    """Canonicalize a float (or None) to an exact, hashable key (its 8-byte IEEE-754 image)."""
    if x is None:
        return None
    return struct.pack("<d", float(x))


# ---------------------------------------------------------------------------
# IR representation: a plain dict; floats compared by exact bits via canon().
# ---------------------------------------------------------------------------
def empty_segment() -> dict:
    return {
        "start": [None, None, None],
        "end": [None, None, None],
        "travel": False,
        "speed": 0.0,
        "length": 0.0,
        "volume": 0.0,
        "filament": 0.0,
        "width": None,
        "height": None,
        "kind": 0,
        "centre": None,
        "clockwise": False,
        "temperature": None,
        "fan": None,
        "flow": None,
        "tool": None,
        "dwell_s": None,
        "manual_gcode": None,
        "orientation": None,
        "control_points": None,
    }


def canon_meta(meta):
    if meta is None:
        return None
    return (
        meta.get("generator"),
        meta.get("units"),
        meta.get("source_hash"),
        tuple(meta.get("invariants") or []),
    )


def canon_seg(s: dict):
    return (
        tuple(fkey(v) for v in s["start"]),
        tuple(fkey(v) for v in s["end"]),
        bool(s["travel"]),
        fkey(s["speed"]),
        fkey(s["length"]),
        fkey(s["volume"]),
        fkey(s["filament"]),
        fkey(s["width"]),
        fkey(s["height"]),
        int(s["kind"]),
        None if s["centre"] is None else tuple(fkey(v) for v in s["centre"]),
        bool(s["clockwise"]),
        fkey(s["temperature"]),
        fkey(s["fan"]),
        fkey(s["flow"]),
        None if s["tool"] is None else int(s["tool"]),
        fkey(s["dwell_s"]),
        s["manual_gcode"],
        None if s["orientation"] is None else tuple(fkey(v) for v in s["orientation"]),
        None
        if s["control_points"] is None
        else tuple(tuple(fkey(v) for v in p) for p in s["control_points"]),
    )


def canon(ir: dict):
    return (
        int(ir["version"]),
        canon_meta(ir["meta"]),
        tuple(canon_seg(s) for s in ir["segments"]),
    )


# ---------------------------------------------------------------------------
# JSON wire form
# ---------------------------------------------------------------------------
def parse_meta(obj):
    if obj is None:
        return None
    return {
        "generator": obj.get("generator"),
        "units": obj.get("units"),
        "source_hash": obj.get("source_hash"),
        "invariants": list(obj.get("invariants") or []),
    }


def parse_json_ir(obj: dict) -> dict:
    """Parse the JSON wire form. Unknown object keys are ignored (forward-compat, spec §8); an unknown
    kind string is rejected."""

    def num(v):
        return None if v is None else float(v)

    segs = []
    for s in obj["segments"]:
        kind_str = s.get("kind", "line")
        if kind_str not in JSON_KIND:
            raise DecodeError(f"unknown SegmentKind {kind_str!r}")
        seg = empty_segment()
        seg["start"] = [num(v) for v in s["start"]]
        seg["end"] = [num(v) for v in s["end"]]
        seg["travel"] = bool(s["travel"])
        seg["speed"] = float(s["speed"])
        seg["length"] = float(s["length"])
        seg["volume"] = float(s["volume"])
        seg["filament"] = float(s["filament"])
        seg["width"] = num(s["width"])
        seg["height"] = num(s["height"])
        seg["kind"] = JSON_KIND[kind_str]
        seg["centre"] = None if s.get("centre") is None else [float(v) for v in s["centre"]]
        seg["clockwise"] = bool(s.get("clockwise", False))
        seg["temperature"] = num(s.get("temperature"))
        seg["fan"] = num(s.get("fan"))
        seg["flow"] = num(s.get("flow"))
        seg["tool"] = None if s.get("tool") is None else int(s["tool"])
        seg["dwell_s"] = num(s.get("dwell_s"))
        seg["manual_gcode"] = s.get("manual_gcode")
        seg["orientation"] = (
            None if s.get("orientation") is None else [float(v) for v in s["orientation"]]
        )
        seg["control_points"] = (
            None
            if s.get("control_points") is None
            else [[float(v) for v in p] for p in s["control_points"]]
        )
        segs.append(seg)
    return {"version": int(obj["version"]), "meta": parse_meta(obj.get("meta")), "segments": segs}


# ---------------------------------------------------------------------------
# DRY0 — columnar (spec §5)
# ---------------------------------------------------------------------------
def _opt_col(r: Reader, n: int):
    valid = r.bits(n)
    vals = [r.f64() for _ in range(n)]
    return [vals[i] if valid[i] else None for i in range(n)]


def _col(r: Reader, n: int):
    return [r.f64() for _ in range(n)]


def decode_dry0(buf: bytes) -> dict:
    h = Reader(buf)
    if h.take(4) != DRY0_MAGIC:
        raise DecodeError("bad magic (not DRY0)")
    enc = h.u8()
    if enc not in (0, 1):
        raise DecodeError(f"unsupported DRY0 enc_ver {enc}")
    version = h.u32()
    n = h.u32()
    body_len = h.u32()
    body = raw_inflate(buf[h.at :])
    if len(body) > body_len:
        raise DecodeError("inflated body exceeds declared body_len")
    r = Reader(body)

    travel = r.bits(n)
    clockwise = r.bits(n)
    sx, sy, sz = _opt_col(r, n), _opt_col(r, n), _opt_col(r, n)
    ex, ey, ez = _opt_col(r, n), _opt_col(r, n), _opt_col(r, n)
    width, height = _opt_col(r, n), _opt_col(r, n)
    cx, cy = _opt_col(r, n), _opt_col(r, n)
    speed, length, volume, filament = _col(r, n), _col(r, n), _col(r, n), _col(r, n)
    temperature, fan = _opt_col(r, n), _opt_col(r, n)
    flow, dwell_s = _opt_col(r, n), _opt_col(r, n)

    # opt-u32 and opt-vec3 columns store a value for EVERY segment (0 / [0,0,0] placeholder for None),
    # masked by the validity bitmap — same shape as the nullable f64 columns above.
    tool_valid = r.bits(n)
    tool_vals = [r.u32() for _ in range(n)]
    tool = [tool_vals[i] if tool_valid[i] else None for i in range(n)]
    ori_valid = r.bits(n)
    ori_vals = [[r.f64(), r.f64(), r.f64()] for _ in range(n)]
    orientation = [ori_vals[i] if ori_valid[i] else None for i in range(n)]

    cp_valid = r.bits(n)
    control_points = []
    for i in range(n):
        if cp_valid[i]:
            count = r.u32()
            control_points.append([[r.f64(), r.f64(), r.f64()] for _ in range(count)])
        else:
            control_points.append(None)

    if enc == 0:
        manual = [None] * n
    else:
        mg_valid = r.bits(n)
        manual = []
        for i in range(n):
            if mg_valid[i]:
                ln = r.u32()
                manual.append(r.take(ln).decode("utf-8"))
            else:
                manual.append(None)

    dict_len = r.u32()
    kind_dict = []
    for _ in range(dict_len):
        ln = r.u32()
        s = r.take(ln).decode("utf-8")
        if s not in DRY0_DICT_TO_ID:
            raise DecodeError(f"unknown SegmentKind dictionary entry {s!r}")
        kind_dict.append(DRY0_DICT_TO_ID[s])
    kinds = [kind_dict[r.u32()] for _ in range(n)]

    present = r.u8()
    meta = None
    if present:
        ln = r.u32()
        meta = parse_meta(json.loads(r.take(ln).decode("utf-8")))

    segs = []
    for i in range(n):
        s = empty_segment()
        s["start"] = [sx[i], sy[i], sz[i]]
        s["end"] = [ex[i], ey[i], ez[i]]
        s["travel"] = travel[i]
        s["speed"] = speed[i]
        s["length"] = length[i]
        s["volume"] = volume[i]
        s["filament"] = filament[i]
        s["width"] = width[i]
        s["height"] = height[i]
        s["kind"] = kinds[i]
        s["centre"] = [cx[i], cy[i]] if (cx[i] is not None and cy[i] is not None) else None
        s["clockwise"] = clockwise[i]
        s["temperature"] = temperature[i]
        s["fan"] = fan[i]
        s["flow"] = flow[i]
        s["tool"] = tool[i]
        s["dwell_s"] = dwell_s[i]
        s["manual_gcode"] = manual[i]
        s["orientation"] = orientation[i]
        s["control_points"] = control_points[i]
        segs.append(s)
    return {"version": version, "meta": meta, "segments": segs}


def _push_bits(out: bytearray, flags: list[bool]):
    n = len(flags)
    for byte_i in range((n + 7) // 8):
        b = 0
        for bit in range(8):
            idx = byte_i * 8 + bit
            if idx < n and flags[idx]:
                b |= 1 << bit
        out.append(b)


def _push_opt_col(out: bytearray, vals):
    _push_bits(out, [v is not None for v in vals])
    for v in vals:
        out += struct.pack("<d", v if v is not None else 0.0)


def encode_dry0(ir: dict, kind_dict_override=None) -> bytes:
    segs = ir["segments"]
    n = len(segs)
    body = bytearray()
    _push_bits(body, [s["travel"] for s in segs])
    _push_bits(body, [s["clockwise"] for s in segs])
    for axis in range(3):
        _push_opt_col(body, [s["start"][axis] for s in segs])
    for axis in range(3):
        _push_opt_col(body, [s["end"][axis] for s in segs])
    _push_opt_col(body, [s["width"] for s in segs])
    _push_opt_col(body, [s["height"] for s in segs])
    _push_opt_col(body, [None if s["centre"] is None else s["centre"][0] for s in segs])
    _push_opt_col(body, [None if s["centre"] is None else s["centre"][1] for s in segs])
    for field in ("speed", "length", "volume", "filament"):
        for s in segs:
            body += struct.pack("<d", s[field])
    for field in ("temperature", "fan", "flow", "dwell_s"):
        _push_opt_col(body, [s[field] for s in segs])
    _push_bits(body, [s["tool"] is not None for s in segs])
    for s in segs:
        body += struct.pack("<I", s["tool"] if s["tool"] is not None else 0)
    _push_bits(body, [s["orientation"] is not None for s in segs])
    for s in segs:
        o = s["orientation"] if s["orientation"] is not None else [0.0, 0.0, 0.0]
        body += struct.pack("<ddd", *o)
    _push_bits(body, [s["control_points"] is not None for s in segs])
    for s in segs:
        if s["control_points"] is not None:
            body += struct.pack("<I", len(s["control_points"]))
            for p in s["control_points"]:
                body += struct.pack("<ddd", *p)
    _push_bits(body, [s["manual_gcode"] is not None for s in segs])
    for s in segs:
        if s["manual_gcode"] is not None:
            mg = s["manual_gcode"].encode("utf-8")
            body += struct.pack("<I", len(mg)) + mg

    # kind dictionary (first-appearance order)
    order = []
    index = {}
    for s in segs:
        k = s["kind"]
        if k not in index:
            index[k] = len(order)
            order.append(k)
    dict_strs = kind_dict_override if kind_dict_override is not None else [DRY0_DICT_STR[k] for k in order]
    body += struct.pack("<I", len(dict_strs))
    for s in dict_strs:
        sb = s.encode("utf-8")
        body += struct.pack("<I", len(sb)) + sb
    for s in segs:
        body += struct.pack("<I", index[s["kind"]])

    meta = ir["meta"]
    if meta is None:
        body.append(0)
    else:
        body.append(1)
        mj = json.dumps(_meta_json(meta), separators=(",", ":")).encode("utf-8")
        body += struct.pack("<I", len(mj)) + mj

    out = bytearray()
    out += DRY0_MAGIC
    out.append(1)
    out += struct.pack("<I", ir["version"])
    out += struct.pack("<I", n)
    out += struct.pack("<I", len(body))
    out += raw_deflate(bytes(body))
    return bytes(out)


def _meta_json(meta: dict) -> dict:
    """Serialize Meta the way serde does: omit None / empty fields."""
    out = {}
    if meta.get("generator") is not None:
        out["generator"] = meta["generator"]
    if meta.get("units") is not None:
        out["units"] = meta["units"]
    if meta.get("source_hash") is not None:
        out["source_hash"] = meta["source_hash"]
    if meta.get("invariants"):
        out["invariants"] = meta["invariants"]
    return out


# ---------------------------------------------------------------------------
# DRY1 — chunked streaming (spec §6)
# ---------------------------------------------------------------------------
def decode_dry1(buf: bytes) -> dict:
    h = Reader(buf)
    if h.take(4) != DRY1_MAGIC:
        raise DecodeError("bad magic (not DRY1)")
    enc = h.u8()
    if enc not in (1, 2):
        raise DecodeError(f"unsupported DRY1 enc_ver {enc}")
    version = h.u32()
    n = h.u32()
    block_size = h.u32()
    if block_size == 0:
        raise DecodeError("DRY1 block_size == 0")
    meta = None
    if h.u8():
        ln = h.u32()
        meta = parse_meta(json.loads(h.take(ln).decode("utf-8")))

    known = LEGACY_KNOWN_FLAGS if enc == 1 else KNOWN_FLAGS
    segs = []
    remaining = n
    while remaining > 0:
        block_n = h.u32()
        if block_n == 0 or block_n > remaining:
            raise DecodeError("bad DRY1 block_n")
        body_len = h.u32()
        deflate_len = h.u32()
        body = raw_inflate(h.take(deflate_len))
        if len(body) != body_len:
            raise DecodeError("DRY1 block body length mismatch")
        r = Reader(body)
        for _ in range(block_n):
            segs.append(_decode_row(r, enc, known))
        if r.at != len(body):
            raise DecodeError("trailing bytes in DRY1 block")
        remaining -= block_n
    return {"version": version, "meta": meta, "segments": segs}


def _decode_row(r: Reader, enc: int, known: int) -> dict:
    flags = r.u32()
    if flags & ~known:
        raise DecodeError(f"unsupported segment flags 0x{flags & ~known:08x}")
    tag = r.u8()
    if tag > 7:
        raise DecodeError(f"unknown SegmentKind tag {tag}")
    s = empty_segment()
    s["kind"] = tag
    s["start"] = [r.f64() if flags & F_START[i] else None for i in range(3)]
    s["end"] = [r.f64() if flags & F_END[i] else None for i in range(3)]
    s["width"] = r.f64() if flags & F_WIDTH else None
    s["height"] = r.f64() if flags & F_HEIGHT else None
    s["centre"] = [r.f64(), r.f64()] if flags & F_CENTRE else None
    s["speed"] = r.f64()
    s["length"] = r.f64()
    s["volume"] = r.f64()
    s["filament"] = r.f64()
    s["temperature"] = r.f64() if flags & F_TEMPERATURE else None
    s["fan"] = r.f64() if flags & F_FAN else None
    s["flow"] = r.f64() if flags & F_FLOW else None
    s["dwell_s"] = r.f64() if flags & F_DWELL else None
    if enc != 1 and flags & F_MANUAL_GCODE:
        ln = r.u32()
        s["manual_gcode"] = r.take(ln).decode("utf-8")
    s["tool"] = r.u32() if flags & F_TOOL else None
    s["orientation"] = [r.f64(), r.f64(), r.f64()] if flags & F_ORIENTATION else None
    if flags & F_CONTROL_POINTS:
        count = r.u32()
        s["control_points"] = [[r.f64(), r.f64(), r.f64()] for _ in range(count)]
    s["travel"] = bool(flags & F_TRAVEL)
    s["clockwise"] = bool(flags & F_CLOCKWISE)
    return s


def _encode_row(s: dict, extra_flags: int = 0) -> bytes:
    flags = extra_flags
    if s["travel"]:
        flags |= F_TRAVEL
    if s["clockwise"]:
        flags |= F_CLOCKWISE
    for i in range(3):
        if s["start"][i] is not None:
            flags |= F_START[i]
        if s["end"][i] is not None:
            flags |= F_END[i]
    if s["width"] is not None:
        flags |= F_WIDTH
    if s["height"] is not None:
        flags |= F_HEIGHT
    if s["centre"] is not None:
        flags |= F_CENTRE
    if s["temperature"] is not None:
        flags |= F_TEMPERATURE
    if s["fan"] is not None:
        flags |= F_FAN
    if s["flow"] is not None:
        flags |= F_FLOW
    if s["dwell_s"] is not None:
        flags |= F_DWELL
    if s["tool"] is not None:
        flags |= F_TOOL
    if s["orientation"] is not None:
        flags |= F_ORIENTATION
    if s["control_points"] is not None:
        flags |= F_CONTROL_POINTS
    if s["manual_gcode"] is not None:
        flags |= F_MANUAL_GCODE

    out = bytearray()
    out += struct.pack("<I", flags)
    out.append(s["kind"])
    for i in range(3):
        if s["start"][i] is not None:
            out += struct.pack("<d", s["start"][i])
    for i in range(3):
        if s["end"][i] is not None:
            out += struct.pack("<d", s["end"][i])
    if s["width"] is not None:
        out += struct.pack("<d", s["width"])
    if s["height"] is not None:
        out += struct.pack("<d", s["height"])
    if s["centre"] is not None:
        out += struct.pack("<dd", *s["centre"])
    out += struct.pack("<dddd", s["speed"], s["length"], s["volume"], s["filament"])
    for field in ("temperature", "fan", "flow", "dwell_s"):
        if s[field] is not None:
            out += struct.pack("<d", s[field])
    if s["manual_gcode"] is not None:
        mg = s["manual_gcode"].encode("utf-8")
        out += struct.pack("<I", len(mg)) + mg
    if s["tool"] is not None:
        out += struct.pack("<I", s["tool"])
    if s["orientation"] is not None:
        out += struct.pack("<ddd", *s["orientation"])
    if s["control_points"] is not None:
        out += struct.pack("<I", len(s["control_points"]))
        for p in s["control_points"]:
            out += struct.pack("<ddd", *p)
    return bytes(out)


def encode_dry1(ir: dict, block_size: int = 512) -> bytes:
    segs = ir["segments"]
    out = bytearray()
    out += DRY1_MAGIC
    out.append(2)
    out += struct.pack("<I", ir["version"])
    out += struct.pack("<I", len(segs))
    out += struct.pack("<I", block_size)
    meta = ir["meta"]
    if meta is None:
        out.append(0)
    else:
        out.append(1)
        mj = json.dumps(_meta_json(meta), separators=(",", ":")).encode("utf-8")
        out += struct.pack("<I", len(mj)) + mj
    for start in range(0, len(segs), block_size):
        chunk = segs[start : start + block_size]
        body = b"".join(_encode_row(s) for s in chunk)
        comp = raw_deflate(body)
        out += struct.pack("<I", len(chunk))
        out += struct.pack("<I", len(body))
        out += struct.pack("<I", len(comp))
        out += comp
    return bytes(out)


# ---------------------------------------------------------------------------
# Validation driver
# ---------------------------------------------------------------------------
def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_vector(vdir: Path, entry: dict, validator) -> list[str]:
    errs = []
    name = entry["name"]
    input_obj = json.loads((vdir / "input.json").read_text())

    # 1. JSON Schema
    schema_errs = sorted(validator.iter_errors(input_obj), key=lambda e: e.path)
    for e in schema_errs:
        errs.append(f"[{name}] schema: {e.message}")

    # 2. parse JSON -> reference IR
    ref = parse_json_ir(input_obj)
    ref_c = canon(ref)

    # 3. JSON re-serialize round-trip (semantic)
    if canon(parse_json_ir(json.loads(json.dumps(input_obj)))) != ref_c:
        errs.append(f"[{name}] JSON re-serialize round-trip differs")

    # 4. decode reference DRY0 / DRY1 -> must equal ref
    # 5. independent encode -> decode self round-trip (lossless)
    # A malformed committed artifact is reported as an error line, never an uncaught traceback.
    checks = [
        ("DRY0 decode != JSON", lambda: decode_dry0((vdir / "expected.dry0").read_bytes())),
        ("DRY1 decode != JSON", lambda: decode_dry1((vdir / "expected.dry1").read_bytes())),
        ("DRY0 self round-trip lost data", lambda: decode_dry0(encode_dry0(ref))),
        ("DRY1 self round-trip lost data", lambda: decode_dry1(encode_dry1(ref))),
    ]
    for label, thunk in checks:
        try:
            if canon(thunk()) != ref_c:
                errs.append(f"[{name}] {label}")
        except (DecodeError, ValueError, KeyError, struct.error) as exc:
            errs.append(f"[{name}] {label} (decode error: {exc})")

    # 6. sha256 of every artifact vs MANIFEST
    for fname, want in entry["artifacts"].items():
        got = sha256_file(vdir / fname)
        if got != want:
            errs.append(f"[{name}] sha256 mismatch for {fname}")
    return errs


def validate_negatives(ndir: Path) -> list[str]:
    errs = []
    index = json.loads((ndir / "INDEX.json").read_text())
    for case in index:
        f, fmt, expect = case["file"], case["format"], case["expect"]
        data = (ndir / f).read_bytes()
        if fmt == "binary":
            accepted = _try(lambda: (decode_dry0 if data[:4] == DRY0_MAGIC else decode_dry1)(data))
        elif fmt == "json":
            accepted = _try(lambda: parse_json_ir(json.loads(data)))
        else:
            errs.append(f"[neg {f}] unknown format {fmt}")
            continue
        if expect == "reject" and accepted:
            errs.append(f"[neg {f}] should have been rejected")
        if expect == "accept" and not accepted:
            errs.append(f"[neg {f}] should have been accepted")

    # Synthesized binary faults the committed files can't express (compressed-body edits):
    # an unknown DRY0 dictionary kind, and an unknown DRY1 flag bit.
    base = {"version": 0, "meta": None, "segments": [empty_segment()]}
    if _try(lambda: decode_dry0(encode_dry0(base, kind_dict_override=["frobnicate"]))):
        errs.append("[neg synth] DRY0 unknown dictionary kind should be rejected")
    if _try(lambda: decode_dry1(_synth_dry1_bad_flag(base))):
        errs.append("[neg synth] DRY1 unknown flag bit should be rejected")
    return errs


def _synth_dry1_bad_flag(ir: dict) -> bytes:
    """A DRY1 stream whose single row sets a flag bit outside the known mask (bit 20)."""
    seg = ir["segments"][0]
    body = _encode_row(seg, extra_flags=1 << 20)
    comp = raw_deflate(body)
    out = bytearray()
    out += DRY1_MAGIC
    out.append(2)
    out += struct.pack("<I", ir["version"])
    out += struct.pack("<I", 1)
    out += struct.pack("<I", 512)
    out.append(0)  # no meta
    out += struct.pack("<I", 1)  # block_n
    out += struct.pack("<I", len(body))
    out += struct.pack("<I", len(comp))
    out += comp
    return bytes(out)


def _try(thunk) -> bool:
    """Return True if thunk succeeds, False if it raises a DecodeError / parse error (a rejection)."""
    try:
        thunk()
        return True
    except (DecodeError, KeyError, ValueError, TypeError, json.JSONDecodeError):
        return False


def main(argv: list[str]) -> int:
    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        print("error: jsonschema is required — pip install -r tools/requirements.txt", file=sys.stderr)
        return 2

    vectors_dir = Path(argv[1]) if len(argv) > 1 else Path("conformance/vectors")
    repo_root = vectors_dir.resolve().parent.parent
    schema = json.loads((repo_root / "spec" / "dry-ir-v0.schema.json").read_text())
    validator = Draft202012Validator(schema)

    manifest = json.loads((vectors_dir / "MANIFEST.json").read_text())
    errors = []
    for entry in manifest["vectors"]:
        errors += validate_vector(vectors_dir / entry["name"], entry, validator)
    errors += validate_negatives(vectors_dir / "_negative")

    if errors:
        print(f"FAIL — {len(errors)} problem(s):", file=sys.stderr)
        for e in errors:
            print("  " + e, file=sys.stderr)
        return 1
    print(
        f"OK — {len(manifest['vectors'])} vectors validated independently "
        f"(JSON + DRY0 + DRY1 decode/encode/round-trip, schema, sha256) with no dry-core."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
