#!/usr/bin/env python3
"""Assert every cross-target capability is reachable exactly where the manifest says it is.

The parity gap in this repo opened silently and in *both* directions — kernel capabilities that
never reached a binding, and binding capabilities that never got a CLI surface — because nothing
asserted anything. `docs/14-known-limitations.md` carried a hand-written table that was a snapshot
of whatever happened to be true when someone last looked, and it was wrong in two cells the first
time it was written.

`conformance/capability-parity.toml` is that table made executable. Each declared cell is checked
against the source both ways:

    status = "reachable"  ->  the symbol MUST appear in the file
    status = "absent"     ->  the symbol MUST NOT appear in the file

So adding a binding without recording it fails, and recording one without adding it fails. An
`absent` row is a reviewed gap carrying a `note`, not an oversight.

Those checks verify the cells that exist. Nothing verified the cells that do not, and that is the
hole the 2026-09-05 audit found: the wasm binding exported 34 functions against 12 recorded
capabilities and this gate was green, so machine compatibility could be reimplemented in two SDKs —
each missing two of the engine's seven rules — with no row to check it against. The manifest's
`[uncovered]` table now lists every binding export awaiting a capability row, and
`check_binding_completeness` fails on an export that is in neither place, and on a stale entry.

Exit 0 when the manifest and the tree agree; 1 otherwise, listing every divergence.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10 and earlier
    import tomli as tomllib

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "conformance" / "capability-parity.toml"
SURFACES = ("core", "cli", "python", "wasm", "ts")


def fail(errors: list[str], msg: str) -> None:
    errors.append(msg)


# Where each binding's exported surface lives, and how to recognise an export in it.
BINDING_EXPORTS = {
    "wasm": ("crates/wasm/src/lib.rs", r"#\[wasm_bindgen\]\s*\npub fn (\w+)"),
    "python": ("py/src/lib.rs", r"#\[pyfunction\]\s*\nfn (\w+)"),
}


def _covers(recorded: str, export: str) -> bool:
    """Does a manifest symbol account for a binding export?

    Recorded symbols are written per surface and do not always match the export name exactly — the
    tool-holder capability records `check_tool_holder_collision` against an export actually named
    `check_tool_holder_collision_wasm`. Substring containment either way is therefore the rule.
    It is deliberately loose: the manifest is reviewed data and this check exists to catch exports
    nobody recorded at all, not to police naming.
    """
    return recorded in export or export in recorded


def check_binding_completeness(capabilities, uncovered, errors: list[str]) -> int:
    """Every binding export is either covered by a capability row or listed in `[uncovered]`.

    The manifest's per-cell checks verify the cells that exist. Nothing verified the cells that do
    not: the wasm binding exported 34 functions against 12 recorded capabilities and the gate was
    green, which is how machine compatibility came to be reimplemented in two SDKs with two of the
    engine's seven rules missing. `[uncovered]` turns that invisible gap into a reviewed backlog,
    and this check keeps it honest in both directions — an unrecorded export fails, and so does a
    stale entry whose export vanished or which has since gained a capability row.
    """
    accounted = 0
    for surface, (rel_path, pattern) in BINDING_EXPORTS.items():
        source = ROOT / rel_path
        if not source.is_file():
            fail(errors, f"[uncovered]: no such binding source: {rel_path}")
            continue
        exports = set(re.findall(pattern, source.read_text(encoding="utf-8")))
        recorded = {
            cap[surface]["symbol"]
            for cap in capabilities
            if surface in cap and isinstance(cap[surface], dict) and "symbol" in cap[surface]
        }
        listed = set(uncovered.get(surface, []))

        for export in sorted(exports):
            has_row = any(_covers(sym, export) for sym in recorded)
            if has_row:
                accounted += 1
                if export in listed:
                    fail(
                        errors,
                        f"[uncovered].{surface}: '{export}' is listed as uncovered but a capability "
                        f"row now covers it — remove the entry",
                    )
            elif export in listed:
                accounted += 1
            else:
                fail(
                    errors,
                    f"{rel_path} exports '{export}' but no capability row covers it and it is not "
                    f"listed in [uncovered].{surface} — record it either way",
                )

        for stale in sorted(listed - exports):
            fail(
                errors,
                f"[uncovered].{surface}: '{stale}' is listed but {rel_path} no longer exports it "
                f"— remove the entry",
            )
    return accounted


def main() -> int:
    errors: list[str] = []

    if not MANIFEST.is_file():
        print(f"error: missing manifest {MANIFEST.relative_to(ROOT)}", file=sys.stderr)
        return 1

    doc = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    if doc.get("schema_version") != 1:
        print("error: capability-parity manifest must declare schema_version = 1", file=sys.stderr)
        return 1

    capabilities = doc.get("capability")
    if not isinstance(capabilities, list) or not capabilities:
        print("error: manifest must contain a non-empty [[capability]] array", file=sys.stderr)
        return 1

    seen: set[str] = set()
    reachable = absent = 0

    for index, cap in enumerate(capabilities):
        cid = cap.get("id")
        if not isinstance(cid, str) or not cid:
            fail(errors, f"capability[{index}]: missing id")
            continue
        if cid in seen:
            fail(errors, f"{cid}: duplicate capability id")
        seen.add(cid)
        if not cap.get("title"):
            fail(errors, f"{cid}: missing title")

        declared = [s for s in SURFACES if s in cap]
        if not declared:
            fail(errors, f"{cid}: declares no surfaces")
        for surface in SURFACES:
            if surface not in cap:
                fail(errors, f"{cid}: surface '{surface}' is not declared (record it, even as absent)")

        for surface in declared:
            cell = cap[surface]
            if not isinstance(cell, dict):
                fail(errors, f"{cid}.{surface}: must be a table")
                continue
            path, symbol, status = cell.get("path"), cell.get("symbol"), cell.get("status")
            if not path or not symbol or status not in ("reachable", "absent"):
                fail(
                    errors,
                    f"{cid}.{surface}: needs path, symbol and status in (reachable, absent)",
                )
                continue
            if status == "absent" and not cell.get("note"):
                fail(errors, f"{cid}.{surface}: an absent cell must carry a note saying why")

            target = ROOT / path
            if not target.is_file():
                fail(errors, f"{cid}.{surface}: no such file: {path}")
                continue

            present = symbol in target.read_text(encoding="utf-8", errors="replace")
            if status == "reachable":
                reachable += 1
                if not present:
                    fail(
                        errors,
                        f"{cid}.{surface}: recorded reachable, but '{symbol}' is not in {path} "
                        f"(the capability was removed or renamed — update the manifest)",
                    )
            else:
                absent += 1
                if present:
                    fail(
                        errors,
                        f"{cid}.{surface}: recorded absent, but '{symbol}' IS in {path} "
                        f"(the surface gained the capability — record it as reachable)",
                    )

    covered = check_binding_completeness(capabilities, doc.get("uncovered", {}), errors)

    if errors:
        print("capability parity: FAILED", file=sys.stderr)
        for e in errors:
            print(f"  error: {e}", file=sys.stderr)
        return 1

    print(
        f"capability parity: ok ({len(capabilities)} capabilities, "
        f"{reachable} reachable and {absent} recorded-absent cells verified against the source; "
        f"{covered} binding exports accounted for)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
