#!/usr/bin/env python3
"""Generate the slicer-corpus STL model set (stdlib-only, no dependencies).

Writes 6 small parametric binary STL files into an output directory (default:
`tools/slicer_corpus/models/`), matching the model table in
`docs/superpowers/specs/2026-08-03-slicer-corpus-and-profiles-design.md` §2:

    cube               20 mm cube                                  baseline/control
    cylinder           48-segment cylinder, dia 15 x 15mm           curved-wall arc-fitting
    overhang_wedge     ~29x20x15mm ramped wedge (45/60/70 deg)      overhang without supports
    bridge             30x10x10mm two piers + unsupported span      bridging
    thin_wall_tower    dia 10 x 30mm hollow tube, single perimeter  thin-wall
    vase_cone          dia 20->10 x 25mm tapered frustum            continuous-Z spiral shell

All meshes are built from a small ear-clipping polygon triangulator plus a
generic "extrude a 2D profile along an axis" helper -- no external geometry
library, matching the prior probe's binary-STL-writer approach (see
`tools/slicer_corpus/slice_matrix.sh` for how these feed OrcaSlicer/CuraEngine).

Usage:
    python tools/slicer_corpus/gen_models.py [outdir]
"""

from __future__ import annotations

import math
import struct
import sys
from pathlib import Path

Point2 = tuple[float, float]
Point3 = tuple[float, float, float]
Triangle = tuple[Point3, Point3, Point3]


# ---------------------------------------------------------------------------
# Binary STL writer
# ---------------------------------------------------------------------------


def write_binary_stl(path: Path, triangles: list[Triangle], name: bytes = b"dry-slicer-corpus") -> None:
    header = name[:80].ljust(80, b"\0")
    with path.open("wb") as f:
        f.write(header)
        f.write(struct.pack("<I", len(triangles)))
        for (v1, v2, v3) in triangles:
            ux, uy, uz = (v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2])
            wx, wy, wz = (v3[0] - v1[0], v3[1] - v1[1], v3[2] - v1[2])
            nx, ny, nz = (uy * wz - uz * wy, uz * wx - ux * wz, ux * wy - uy * wx)
            length = math.sqrt(nx * nx + ny * ny + nz * nz)
            if length > 0:
                nx, ny, nz = (nx / length, ny / length, nz / length)
            f.write(struct.pack("<fff", nx, ny, nz))
            for v in (v1, v2, v3):
                f.write(struct.pack("<fff", *v))
            f.write(struct.pack("<H", 0))


# ---------------------------------------------------------------------------
# Ear-clipping triangulation for simple (non-self-intersecting) 2D polygons
# ---------------------------------------------------------------------------


def _signed_area(poly: list[Point2]) -> float:
    area = 0.0
    n = len(poly)
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        area += x1 * y2 - x2 * y1
    return area / 2.0


def _point_in_triangle(p: Point2, a: Point2, b: Point2, c: Point2) -> bool:
    def sign(p1: Point2, p2: Point2, p3: Point2) -> float:
        return (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1])

    d1 = sign(p, a, b)
    d2 = sign(p, b, c)
    d3 = sign(p, c, a)
    has_neg = (d1 < 0) or (d2 < 0) or (d3 < 0)
    has_pos = (d1 > 0) or (d2 > 0) or (d3 > 0)
    return not (has_neg and has_pos)


def triangulate(poly: list[Point2]) -> list[tuple[int, int, int]]:
    """Ear-clipping triangulation. Returns index triples into `poly`.

    Assumes `poly` is a simple polygon (no self-intersections, no holes) with
    no repeated closing vertex. Works for convex and non-convex (e.g. the
    bridge's notched "staple" outline) input, which is all this generator
    needs -- not a general-purpose CDT.
    """
    n = len(poly)
    if n < 3:
        return []
    indices = list(range(n))
    if _signed_area(poly) < 0:
        indices.reverse()

    triangles: list[tuple[int, int, int]] = []
    remaining = indices[:]
    guard = 0
    while len(remaining) > 3 and guard < 10_000:
        guard += 1
        ear_found = False
        m = len(remaining)
        for k in range(m):
            i0, i1, i2 = remaining[(k - 1) % m], remaining[k], remaining[(k + 1) % m]
            a, b, c = poly[i0], poly[i1], poly[i2]
            # Convex vertex? (cross product sign matches CCW orientation)
            cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
            if cross <= 0:
                continue
            if any(
                remaining[j] not in (i0, i1, i2) and _point_in_triangle(poly[remaining[j]], a, b, c)
                for j in range(m)
            ):
                continue
            triangles.append((i0, i1, i2))
            remaining.pop(k)
            ear_found = True
            break
        if not ear_found:
            # Degenerate/near-collinear input: fall back to a fan so the
            # generator never hard-fails on a hand-authored profile.
            break
    if len(remaining) >= 3:
        for k in range(1, len(remaining) - 1):
            triangles.append((remaining[0], remaining[k], remaining[k + 1]))
    return triangles


# ---------------------------------------------------------------------------
# Extrusion: sweep a closed 2D profile along one of the three axes
# ---------------------------------------------------------------------------


def _lift(u: float, v: float, w: float, axis: int) -> Point3:
    if axis == 2:  # profile in XY, swept along Z
        return (u, v, w)
    if axis == 1:  # profile in XZ, swept along Y
        return (u, w, v)
    return (w, u, v)  # axis == 0: profile in YZ, swept along X


def extrude(profile: list[Point2], lo: float, hi: float, axis: int = 2) -> list[Triangle]:
    """Sweep a closed simple polygon `profile` from `lo` to `hi` along `axis`.

    Produces a closed, manifold triangle mesh: two triangulated end caps plus
    a quad (as two triangles) per boundary edge. Triangle winding is not
    guaranteed outward-consistent for every input (irrelevant to slicing,
    which uses ray/plane intersection rather than shading normals), but every
    edge is shared by exactly two triangles.
    """
    tris_idx = triangulate(profile)
    n = len(profile)
    bottom = [_lift(u, v, lo, axis) for u, v in profile]
    top = [_lift(u, v, hi, axis) for u, v in profile]

    triangles: list[Triangle] = []
    for (i, j, k) in tris_idx:
        triangles.append((bottom[i], bottom[k], bottom[j]))
    for (i, j, k) in tris_idx:
        triangles.append((top[i], top[j], top[k]))
    for e in range(n):
        i, j = e, (e + 1) % n
        b0, b1 = bottom[i], bottom[j]
        t0, t1 = top[i], top[j]
        triangles.append((b0, b1, t1))
        triangles.append((b0, t1, t0))
    return triangles


def regular_polygon(radius: float, segments: int, cx: float = 0.0, cy: float = 0.0) -> list[Point2]:
    return [
        (cx + radius * math.cos(2 * math.pi * i / segments), cy + radius * math.sin(2 * math.pi * i / segments))
        for i in range(segments)
    ]


# ---------------------------------------------------------------------------
# Models
# ---------------------------------------------------------------------------


def model_cube() -> list[Triangle]:
    profile = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]
    return extrude(profile, 0.0, 20.0, axis=2)


def model_cylinder(segments: int = 48) -> list[Triangle]:
    profile = regular_polygon(7.5, segments)
    return extrude(profile, 0.0, 15.0, axis=2)


def model_overhang_wedge() -> list[Triangle]:
    # Three ramp segments climbing 5 mm each at overhang angles (from
    # vertical) of 45/60/70 deg, horizontal run scaled to fit a 20 mm
    # footprint; see the design doc §2 and this file's module docstring.
    #
    # The footprint must *widen* with Z (narrow at the build plate, wide at
    # the top) for this to actually be an overhang -- each layer has to
    # extend past the layer below it, unsupported. A ramp that narrows with
    # Z (wide base, tapering to a point) is a self-supporting incline
    # instead: every layer sits entirely within the footprint of the one
    # below it, so no slicer ever needs to bridge or flag it. (Previously
    # this profile ran top-to-bottom `(0,15)->(x1,10)->(x2,5)->(x3,0)`,
    # i.e. narrowing with height -- measured against the frozen slice this
    # produced zero `; FEATURE: Overhang wall` blocks. See
    # `docs/25-slicer-corpus-baseline.md` for the measurement.)
    #
    # A pure widen-to-a-point-at-the-bed profile (the first attempt at this
    # fix) is *also* wrong the other way: the Z=0 cross-section collapses to
    # a single line with zero contact area, which OrcaSlicer refuses to
    # slice at all ("found slicing or export error", no first layer to
    # print onto). `foot` gives the base a small flat, full-Y-length
    # contact patch at Z=0 so the model is both printable and a genuine
    # overhang -- the ramp still widens monotonically with Z above it.
    # Runs are *not* scaled down to fit a 20 mm budget: scaling shrinks the
    # horizontal run without shrinking `dz`, which flattens the effective
    # angle-from-vertical of every segment (e.g. the previous scale factor
    # turned the intended 45/60/70 deg into an actual ~36/52/64 deg -- below
    # OrcaSlicer's own overhang-detection threshold for the first two
    # segments). Unscaled, the footprint is a few mm wider than 20 mm, but
    # the angles are the ones the docstring claims, which is what actually
    # matters for exercising overhang detection.
    foot = 2.0
    dz = 5.0
    runs = [dz * math.tan(math.radians(a)) for a in (45.0, 60.0, 70.0)]
    x0 = foot
    x1 = x0 + runs[0]
    x2 = x1 + runs[1]
    x3 = x2 + runs[2]
    profile = [
        (0.0, 0.0),
        (x0, 0.0),
        (x1, 5.0),
        (x2, 10.0),
        (x3, 15.0),
        (0.0, 15.0),
    ]
    return extrude(profile, 0.0, 20.0, axis=1)


def model_bridge() -> list[Triangle]:
    # 30x10x10 mm bounding box: two 5mm-wide, 5mm-tall piers at the ends, a
    # full-width slab on top -- a rectangle with a notch cut from the bottom
    # middle (the unsupported span between the piers).
    profile = [
        (0.0, 0.0),
        (0.0, 10.0),
        (30.0, 10.0),
        (30.0, 0.0),
        (25.0, 0.0),
        (25.0, 5.0),
        (5.0, 5.0),
        (5.0, 0.0),
    ]
    return extrude(profile, 0.0, 10.0, axis=1)


def model_thin_wall_tower(segments: int = 32, wall: float = 1.0) -> list[Triangle]:
    # A capped hollow tube: outer + inner cylinder walls, annulus caps top
    # and bottom. Single-perimeter, no-infill stress case per the design doc.
    outer_r, height = 5.0, 30.0
    inner_r = outer_r - wall
    outer = regular_polygon(outer_r, segments)
    inner = regular_polygon(inner_r, segments)

    triangles: list[Triangle] = []

    def lift(ring: list[Point2], z: float) -> list[Point3]:
        return [(x, y, z) for x, y in ring]

    outer_bot, outer_top = lift(outer, 0.0), lift(outer, height)
    inner_bot, inner_top = lift(inner, 0.0), lift(inner, height)

    # Outer wall (normal points outward-ish).
    for e in range(segments):
        i, j = e, (e + 1) % segments
        triangles.append((outer_bot[i], outer_bot[j], outer_top[j]))
        triangles.append((outer_bot[i], outer_top[j], outer_top[i]))
    # Inner wall (reversed winding relative to outer -- irrelevant to slicing).
    for e in range(segments):
        i, j = e, (e + 1) % segments
        triangles.append((inner_bot[j], inner_bot[i], inner_top[i]))
        triangles.append((inner_bot[j], inner_top[i], inner_top[j]))
    # Bottom annulus cap (quad strip outer->inner per segment).
    for e in range(segments):
        i, j = e, (e + 1) % segments
        triangles.append((outer_bot[i], inner_bot[i], inner_bot[j]))
        triangles.append((outer_bot[i], inner_bot[j], outer_bot[j]))
    # Top annulus cap.
    for e in range(segments):
        i, j = e, (e + 1) % segments
        triangles.append((outer_top[i], inner_top[j], inner_top[i]))
        triangles.append((outer_top[i], outer_top[j], inner_top[j]))
    return triangles


def model_vase_cone(segments: int = 48) -> list[Triangle]:
    # Solid tapered frustum, dia 20 -> 10 over 25 mm height. Modeled solid
    # (not hollow): a slicer's own spiral-vase mode is what turns a solid
    # profile like this into a single continuous-Z wall; the corpus does not
    # depend on that mode being enabled to still exercise a tapering shape.
    height = 25.0
    bottom_r, top_r = 10.0, 5.0
    bottom = regular_polygon(bottom_r, segments)
    top = regular_polygon(top_r, segments)

    triangles: list[Triangle] = []
    bottom3 = [(x, y, 0.0) for x, y in bottom]
    top3 = [(x, y, height) for x, y in top]
    tris_idx = triangulate(bottom)
    for (i, j, k) in tris_idx:
        triangles.append((bottom3[i], bottom3[k], bottom3[j]))
    for (i, j, k) in tris_idx:
        triangles.append((top3[i], top3[j], top3[k]))
    for e in range(segments):
        i, j = e, (e + 1) % segments
        triangles.append((bottom3[i], bottom3[j], top3[j]))
        triangles.append((bottom3[i], top3[j], top3[i]))
    return triangles


MODELS = {
    "cube": model_cube,
    "cylinder": model_cylinder,
    "overhang_wedge": model_overhang_wedge,
    "bridge": model_bridge,
    "thin_wall_tower": model_thin_wall_tower,
    "vase_cone": model_vase_cone,
}


def main(argv: list[str]) -> int:
    outdir = Path(argv[1]) if len(argv) > 1 else Path(__file__).resolve().parent / "models"
    outdir.mkdir(parents=True, exist_ok=True)
    for name, builder in MODELS.items():
        triangles = builder()
        path = outdir / f"{name}.stl"
        write_binary_stl(path, triangles, name=name.encode("ascii"))
        print(f"wrote {path} ({len(triangles)} triangles)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
