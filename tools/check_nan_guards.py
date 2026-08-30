#!/usr/bin/env python3
"""Every public `f64` parameter compared by ordering must be accounted for.

An ordering comparison is **false** when either side is `NaN`. A guard written as one therefore lets
`NaN` through silently, and if the guard's `false` branch means "no problem here", the check fails
*open*. This review found the same shape seven times, three of them in safety checks:

    check_dual_robot_clearance      reported `safe = true` on a NaN joint angle
    detect_anomalies                reported a healthy printer on a NaN thermistor reading
    calculate_clearance_velocity_scale  returned NaN instead of slowing down
    optimize_constant_mrr           wrote NaN and negative feedrates past `depth_of_cut <= 0.0`
    BrepSolid::slice_to_l1_ops      accepted a NaN Z bound past `z_end < z_start`
    DexelGrid::new_stock            accepted a NaN bound past `max_x <= min_x`
    generate_trochoidal_slot        emitted a non-finite coordinate past `slot_width <= tool_diameter`

`generate/tpms.rs` and `clothoid.rs` already carried source notes about exactly this — so the class
was known, fixed locally twice, and never generalised. This is the generalisation.

The detector is deliberately approximate: it enumerates candidates, and
`conformance/nan-guards.toml` carries the judgement. Every candidate must be recorded as either

    status = "guarded"  -- non-finite input is refused or declined; `how` says where
    status = "safe"     -- the ordering result is fail-safe or NaN cannot reach; `why` says how

A candidate that is not in the manifest fails, so new code carrying this shape cannot land unreviewed.
Sites in the manifest that the detector no longer finds also fail, so the manifest cannot rot.
"""

from __future__ import annotations

import pathlib
import re
import sys

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib

ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "conformance" / "nan-guards.toml"
ROOTS = ("crates/core/src", "crates/moonraker/src", "crates/llm/src", "crates/license/src")

FN = re.compile(
    r"(?P<vis>pub(?:\([a-z]+\))?\s+)?fn\s+(?P<name>[a-z_0-9]+)\s*\((?P<args>[^)]*)\)", re.S
)


def f64_params(args: str) -> list[str]:
    out = []
    for part in re.split(r",(?![^<]*>)", args):
        m = re.match(r"(?:mut\s+)?([a-z_][a-z0-9_]*)\s*:\s*(.+)$", part.strip(), re.S)
        if m and m.group(2).replace("\n", " ").strip() == "f64":
            out.append(m.group(1))
    return out


def body_of(src: str, start: int) -> str:
    i = src.find("{", start)
    if i < 0:
        return ""
    depth, j = 0, i
    while j < len(src):
        if src[j] == "{":
            depth += 1
        elif src[j] == "}":
            depth -= 1
            if depth == 0:
                return src[i : j + 1]
        j += 1
    return src[i:]


def candidates() -> set[tuple[str, str]]:
    found: set[tuple[str, str]] = set()
    for root in ROOTS:
        for path in sorted((ROOT / root).rglob("*.rs")):
            src = path.read_text(encoding="utf-8")
            for m in FN.finditer(src):
                if not m.group("vis"):
                    continue
                params = f64_params(m.group("args"))
                if not params:
                    continue
                body = body_of(src, m.end())
                for p in params:
                    cmp_re = re.compile(
                        rf"\b{re.escape(p)}\s*(<=|>=|<|>)\s*[-\w.]+"
                        rf"|[-\w.]+\s*(<=|>=|<|>)\s*\b{re.escape(p)}\b"
                    )
                    if not cmp_re.search(body):
                        continue
                    direct = re.search(
                        rf"\b{re.escape(p)}\s*\.\s*is_(finite|nan)\s*\(", body
                    )
                    if direct:
                        continue
                    found.add((f"{m.group('name')}", p))
    return found


def main() -> int:
    if not MANIFEST.is_file():
        print(f"error: missing {MANIFEST.relative_to(ROOT)}", file=sys.stderr)
        return 1
    doc = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    if doc.get("schema_version") != 1:
        print("error: nan-guards manifest must declare schema_version = 1", file=sys.stderr)
        return 1

    recorded: dict[tuple[str, str], dict] = {}
    for entry in doc.get("site", []):
        key = (entry.get("function", ""), entry.get("parameter", ""))
        recorded[key] = entry

    errors: list[str] = []
    for key, entry in recorded.items():
        status = entry.get("status")
        if status not in ("guarded", "safe"):
            errors.append(f"{key[0]}({key[1]}): status must be 'guarded' or 'safe'")
        if status == "guarded" and not entry.get("how"):
            errors.append(f"{key[0]}({key[1]}): a guarded site must say how, in `how`")
        if status == "safe" and not entry.get("why"):
            errors.append(f"{key[0]}({key[1]}): a safe site must say why, in `why`")

    found = candidates()
    for key in sorted(found - set(recorded)):
        errors.append(
            f"{key[0]}({key[1]}: f64): compared by ordering with no finiteness check and not "
            f"recorded in conformance/nan-guards.toml. An ordering comparison is false for NaN — "
            f"decide whether that is fail-safe here, then record it."
        )
    for key in sorted(set(recorded) - found):
        errors.append(
            f"{key[0]}({key[1]}): recorded in the manifest but no longer a candidate — the guard "
            f"was fixed or the function changed; drop the entry."
        )

    if errors:
        print("nan guards: FAILED", file=sys.stderr)
        for e in errors:
            print(f"  error: {e}", file=sys.stderr)
        return 1

    guarded = sum(1 for e in recorded.values() if e.get("status") == "guarded")
    print(
        f"nan guards: ok ({len(recorded)} sites accounted for — "
        f"{guarded} guarded, {len(recorded) - guarded} reviewed fail-safe)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
