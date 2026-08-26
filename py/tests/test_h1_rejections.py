"""Binding-level coverage for the H1 hardening rejections (#192).

Four slices narrowed what the engine accepts — H1.1 (emit gate), H1.2 (ingress validation),
H1.4 (TPMS vacuity) and H1.3 (structural verify rules). Each was judged "a coverage gap rather
than a live risk" on the grounds that nothing which previously *worked* is now refused, but that
reasoning had never been checked from a binding surface, which is where it matters: a refusal that
crosses the boundary as a value rather than an exception is indistinguishable from an empty
successful program. ADR 0002 §3 records exactly that happening on the browser surface.

These assertions mirror `sdk/ts/test/h1-rejections.test.ts`, so a divergence between the two
published bindings shows up as a test failure in one of them rather than as a support ticket.
"""

import pytest

import dry


def oriented_design(i, j, k):
    """A one-segment five-axis design with an explicit toolframe orientation."""
    return (
        dry.Design()
        .geometry(0.6, 0.2)
        .extruder(True)
        .point(0, 0, 0.2)
        .orient(i, j, k)
        .point(10, 0, 0.2)
    )


def test_refused_tpms_option_set_raises_rather_than_returning_an_empty_program():
    # H1.4: an isoLevel outside the field's range traces no contour on any layer. Before the fix
    # this resolved, verified with zero findings and simulated to zero volume — a call that
    # "succeeds" and deposits nothing is the confidently-wrong artifact ADR 0002 §4 forbids.
    with pytest.raises(ValueError, match="isoLevel"):
        dry.tpms_gcode({"isoLevel": 2.0, "cellsX": 2, "cellsY": 2, "cellsZ": 2})

    # The control: the same call without the bad isoLevel produces a real program.
    assert len(dry.tpms_gcode({"cellsX": 2, "cellsY": 2, "cellsZ": 2})) > 0


def test_zero_magnitude_orientation_is_refused_at_ingress():
    # H1.2: there is no tool direction to recover from a zero vector, so it cannot be normalised
    # and must not be silently treated as +Z.
    with pytest.raises(ValueError, match="non-zero magnitude"):
        oriented_design(0, 0, 0).gcode(five_axis=True, rotary_axes="ab")


@pytest.mark.parametrize("rotary_axes", ["ab", "ac", "bc"])
def test_non_unit_orientation_is_normalised_so_every_rotary_model_agrees(rotary_axes):
    # Regression test for audit finding C2. `ab` recovers tilt with `atan2` and is scale-invariant,
    # while `ac`/`bc` use `acos(k)` and assume ||v|| == 1 — so the same orientation used to produce
    # different angles under different models, and [0,0,0.5] put the *linear* axes at the wrong
    # point entirely (`Z-8.660254 B60`).
    #
    # H1.1 fixed it by normalising rather than refusing, which is the stronger choice: a non-unit
    # direction vector is unambiguous, so there is nothing to refuse. What must hold is that
    # scaling the vector changes nothing about the emitted program.
    scaled = oriented_design(0, 0, 0.5).gcode(five_axis=True, rotary_axes=rotary_axes)
    unit = oriented_design(0, 0, 1).gcode(five_axis=True, rotary_axes=rotary_axes)

    assert scaled == unit, f"rotary model {rotary_axes} is sensitive to orientation magnitude"
    assert len(scaled) > 0


def test_verify_still_reports_the_non_unit_orientation_that_emit_tolerates():
    # The two surfaces have different jobs and both must do theirs: `emit` is robust so it never
    # produces a wrong-point program, while `verify` says the IR is malformed. If only one acted, a
    # caller would either get a bad program or no warning about a bad design.
    report = oriented_design(0, 0, 0.5).verify()
    rules = [f["rule"] for f in report["findings"]]
    assert "orientation-not-unit" in rules, rules


def test_verify_report_states_its_own_coverage():
    # H1.3: `findings == []` is equally true of a clean program and of one that was never
    # inspected, and this SDK previously had no way to tell the two apart.
    report = (
        dry.Design()
        .geometry(0.6, 0.2)
        .extruder(True)
        .point(0, 0, 0.2)
        .point(10, 0, 0.2)
        .verify()
    )

    assert report["findings"] == []
    assert report["segments_inspected"] > 0, report
    for rule in ("continuity", "segment-length", "arc-length", "negative-quantity"):
        assert rule in report["rules_evaluated"], report["rules_evaluated"]
    assert isinstance(report["contracts"], dict)


def test_refusals_arrive_as_exceptions_never_as_blank_success():
    # The historic failure mode this file exists for: a refusal that reaches the caller as an empty
    # sequence reads as a successful program with no moves.
    refusals = {
        "tpms vacuity": lambda: dry.tpms_gcode(
            {"isoLevel": 2.0, "cellsX": 2, "cellsY": 2, "cellsZ": 2}
        ),
        "zero orientation": lambda: oriented_design(0, 0, 0).gcode(
            five_axis=True, rotary_axes="ab"
        ),
    }

    for name, call in refusals.items():
        with pytest.raises(ValueError) as excinfo:
            result = call()
            pytest.fail(
                f"[{name}] returned {result!r} instead of raising — a refusal that arrives as a "
                "value is indistinguishable from an empty successful program"
            )
        assert str(excinfo.value), f"[{name}] raised with no message"


# The SDK parses the same documented CSV contract formats the Rust CLI does. `float()` already
# refuses "abc" and an empty field — more than JavaScript's `Number` manages — but it still accepts
# "nan", "inf" and "1e400" (the last as infinity). A non-finite contract cannot decide anything:
# every ordering comparison against NaN is false, so the engine treats one as not in force and the
# report simply omits the rule. Refusing here says why instead of silently checking less.
@pytest.mark.parametrize(
    "bounds",
    ["abc,1,2,3,4,5", "0,100,0,,0,100", "0,nan,0,100,0,100", "0,1e400,0,100,0,100"],
)
def test_bounds_csv_refuses_unusable_values(bounds):
    design = (
        dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(50, 0, 0.2)
    )
    with pytest.raises(ValueError, match="not a number|must all be finite"):
        design.verify(bounds=bounds)


def test_bounds_csv_still_accepts_a_well_formed_volume():
    design = (
        dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(50, 0, 0.2)
    )
    report = design.verify(bounds="0,100,0,100,0,100")
    assert any("bounds" in rule for rule in report["rules_evaluated"])


@pytest.mark.parametrize("rng", ["60,nan", "60,1e400", "60,"])
def test_range_csv_refuses_unusable_values(rng):
    design = (
        dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(50, 0, 0.2)
    )
    with pytest.raises(ValueError, match="not a number|must all be finite"):
        design.verify(speed_range=rng)


def test_range_csv_length_is_checked():
    """`_range_to_list` never checked its length, so '60,70,80' reached the binding as three."""
    design = (
        dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(50, 0, 0.2)
    )
    with pytest.raises(ValueError, match="must be 'min,max'"):
        design.verify(speed_range="60,70,80")


def test_structured_contracts_must_be_finite_too():
    design = (
        dry.Design().geometry(0.6, 0.2).extruder(True).point(0, 0, 0.2).point(50, 0, 0.2)
    )
    with pytest.raises(ValueError, match="must all be finite"):
        design.verify(bounds=[[0, 100], [0, float("inf")], [0, 100]])
    with pytest.raises(ValueError, match="must all be finite"):
        design.verify(speed_range=[60, float("nan")])
