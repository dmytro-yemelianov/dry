"""Oriented (5-axis / non-planar) conformance for the PyO3 binding.

Every other conformance suite here drives `conformance/gallery/`, which is FullControl-oracle output
and therefore entirely planar — so before this file the binding's toolframe-orientation channel had no
Python-side coverage at all. `conformance/vectors/five_axis_drape` is the corpus's one oriented design;
it publishes its L1 op list and `ResolveParams` as `design.json` precisely so a binding that builds
outside the engine's Cargo workspace can drive the same design and diff its own output.

NOTE: like the rest of `py/tests`, these require the compiled `_native` extension built by
`maturin develop`; they are gated by the `python-sdk` CI job.

The fixture is NOT oracle-backed (see its `vector.json`): the committed g-code is this engine's own
`emit`. What these tests establish is native/Python *parity* on the orientation path, not that the
rotary convention is correct.
"""

import json
import os

import pytest

import dry

VECTOR_DIR = os.path.join(
    os.path.dirname(__file__), "..", "..", "conformance", "vectors", "five_axis_drape"
)


def _read(name):
    with open(os.path.join(VECTOR_DIR, name)) as f:
        return json.load(f)


def _lines(name):
    with open(os.path.join(VECTOR_DIR, name)) as f:
        return f.read().rstrip("\n").split("\n")


@pytest.fixture(scope="module")
def drape():
    design = _read("design.json")
    vector = _read("vector.json")
    emit = vector["emit_params"]
    # `kinematics` is the engine's Debug rendering, e.g. `Ab { pivot_offset: [...] }`; the binding
    # takes the ab/ac/bc selector. Read it from the vector instead of restating it, so a
    # regeneration under different settings fails loudly here rather than diverging in silence.
    rotary = str(emit["kinematics"]).split(" ")[0].lower()
    assert rotary in ("ab", "ac", "bc"), emit
    assert emit["flavor"] == "Marlin" and emit["five_axis"], emit
    return design, emit, rotary


def test_five_axis_drape_gcode_matches_the_committed_vector(drape):
    design, emit, rotary = drape
    got = dry._native.resolve_gcode(
        json.dumps(design["ops"]),
        json.dumps(design["resolve_params"]),
        emit["relative_e"],
        emit["travel_g1_e0"],
        True,
        rotary,
    )
    assert got == _lines("expected.gcode")

    # A byte match against a file that carried no rotary words would prove nothing.
    letters = rotary.upper()
    rotary_words = [
        w
        for line in got
        for w in line.split(" ")
        if w[:1] in letters and w[1:].lstrip("-").replace(".", "", 1).isdigit()
    ]
    assert len(rotary_words) >= 4, got


def test_five_axis_drape_three_axis_emit_drops_the_orientation(drape):
    """Orientation is not representable on three axes; the documented behaviour is to drop it."""
    design, emit, rotary = drape
    planar = dry._native.resolve_gcode(
        json.dumps(design["ops"]),
        json.dumps(design["resolve_params"]),
        emit["relative_e"],
        emit["travel_g1_e0"],
        False,
        rotary,
    )
    letters = rotary.upper()
    for line in planar:
        for word in line.split(" "):
            assert word[:1] not in letters, f"3-axis emit carries a rotary word: {line}"


def test_five_axis_drape_metrics_match_the_committed_vector(drape):
    design, _emit, _rotary = drape
    got = json.loads(
        dry._native.resolve_metrics(
            json.dumps(design["ops"]), json.dumps(design["resolve_params"])
        )
    )
    want = _read("metrics.json")
    for key, expected in want.items():
        # A missing key must fail, not skip: `got.get(key)` returning None and comparing NaN-ishly
        # is exactly how a drift gate goes vacuous.
        assert key in got, f"metrics.{key} missing from resolve_metrics output"
        if isinstance(expected, float):
            assert abs(got[key] - expected) <= 1e-9, f"metrics.{key} {got[key]} != {expected}"
        else:
            assert got[key] == expected, f"metrics.{key} {got[key]} != {expected}"


def test_five_axis_drape_ir_carries_the_orientations(drape):
    """The resolved IR reaching Python carries the dome normals, not just the g-code."""
    design, _emit, _rotary = drape
    ir = json.loads(
        dry._native.resolve_ir(
            json.dumps(design["ops"]), json.dumps(design["resolve_params"])
        )
    )
    want = _read("input.json")
    assert len(ir["segments"]) == len(want["segments"])
    for got_seg, want_seg in zip(ir["segments"], want["segments"]):
        assert got_seg.get("orientation") == want_seg.get("orientation")
