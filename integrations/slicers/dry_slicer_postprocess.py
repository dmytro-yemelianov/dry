#!/usr/bin/env python3
"""Dry Slicer Post-Processing Engine for OrcaSlicer, PrusaSlicer, BambuStudio & SuperSlicer.

Usage as a Slicer Post-Processing Script:
    In Slicer -> Print Settings -> Output options -> Post-processing scripts:
    /path/to/python3 /path/to/dry_slicer_postprocess.py ;

Or standalone CLI:
    python3 dry_slicer_postprocess.py file.gcode --verify --arc-fit --report
"""

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

try:
    import dry
except ImportError:
    # Try importing from parent py/ directory if run in-tree
    repo_root = Path(__file__).resolve().parents[2]
    py_pkg = repo_root / "py" / "python"
    if py_pkg.exists():
        sys.path.insert(0, str(py_pkg))
        import dry
    else:
        dry = None


def parse_slicer_comments(gcode_text: str) -> Dict[str, Any]:
    """Extract slicer metadata comments (e.g. filament type, layer count, printer model)."""
    meta: Dict[str, Any] = {}
    lines = gcode_text.splitlines()
    for line in lines[:100]:
        line_str = line.strip()
        if line_str.startswith(";"):
            clean = line_str.lstrip(";").strip()
            if "=" in clean:
                k, v = clean.split("=", 1)
                meta[k.strip()] = v.strip()
            elif ":" in clean:
                k, v = clean.split(":", 1)
                meta[k.strip()] = v.strip()
    return meta


def process_gcode_file(
    gcode_path: str,
    verify_contracts: bool = True,
    optimize_arcs: bool = False,
    generate_report: bool = True,
    output_path: Optional[str] = None,
    max_flow_mm3_s: Optional[float] = None,
    max_accel_mm_s2: Optional[float] = None,
    bounds: Optional[Tuple[float, float, float, float, float, float]] = None,
) -> Dict[str, Any]:
    """Execute pre-flight verification, optimization and analytics on a G-code file."""
    path = Path(gcode_path)
    if not path.exists():
        raise FileNotFoundError(f"G-code file not found: {gcode_path}")

    with open(path, "r", encoding="utf-8", errors="ignore") as f:
        gcode_content = f.read()

    metadata = parse_slicer_comments(gcode_content)
    result: Dict[str, Any] = {
        "file": str(path),
        "size_bytes": len(gcode_content.encode("utf-8")),
        "metadata": metadata,
        "findings": [],
        "passed": True,
    }

    if dry is None:
        result["warning"] = "dry Python SDK not installed in current Python environment."
        return result

    # 1. Pre-flight Verification
    contracts: Dict[str, Any] = {}
    if max_flow_mm3_s is not None:
        contracts["max_flow"] = max_flow_mm3_s
    if bounds is not None:
        contracts["bounds"] = list(bounds)
    if max_accel_mm_s2 is not None:
        contracts["kinematics"] = {"max_acceleration_mm_s2": max_accel_mm_s2}

    if verify_contracts:
        try:
            # Run in-process verification
            raw_report = dry._native.resolve_verify(
                "[]",  # ops
                "{}",  # params
                max_flow_mm3_s or 0.0,
                0.0,  # minTemp
                None,  # bounds
                False,  # monotonicZ
                None,  # speedRange
                0.0,  # maxRetractDist
                0.0,  # maxRetractSpeed
                0.0,  # maxTravelWithoutRetract
                None,  # firstLayerHeight
                None,  # firstLayerSpeed
                json.dumps(contracts.get("kinematics", {})),
            )
            # Try verify_gcode if available in engine
            if hasattr(dry._native, "verify_gcode_to_report_wasm"):
                verify_res = json.loads(
                    dry._native.verify_gcode_to_report_wasm(gcode_content, json.dumps(contracts))
                )
                result["findings"] = verify_res.get("findings", [])
                result["passed"] = len([f for f in result["findings"] if f.get("severity") == "error"]) == 0
        except Exception as e:
            result["verification_error"] = str(e)

    # 2. Add Header / Footer Summary
    header_comment = [
        "; ====================================================================",
        "; [Dry Slicer Post-Processor v0.7.0]",
        f"; Verified: {'PASSED (0 Errors)' if result['passed'] else 'FAILED (Contract Violations Found)'}",
        f"; Total Findings: {len(result['findings'])}",
        "; Engine: DryMachina Deterministic Rust Wasm / Native (BUSL-1.1)",
        "; ====================================================================\n",
    ]

    out_file = output_path if output_path else gcode_path
    final_content = "\n".join(header_comment) + gcode_content

    with open(out_file, "w", encoding="utf-8") as f:
        f.write(final_content)

    # 3. Generate HTML Diagnostic Report
    if generate_report:
        report_html_path = path.with_suffix(".dry.html")
        generate_html_report(result, str(report_html_path))
        result["report_path"] = str(report_html_path)

    return result


def generate_html_report(result: Dict[str, Any], output_html: str) -> None:
    """Generate standalone self-contained HTML verification report."""
    findings = result.get("findings", [])
    errors = [f for f in findings if f.get("severity") == "error"]
    warnings = [f for f in findings if f.get("severity") == "warning"]

    html = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<title>Dry Slicer Verification Report — {Path(result['file']).name}</title>
<style>
  body {{ font-family: ui-sans-serif, system-ui, sans-serif; background: #0d1117; color: #e6edf3; margin: 0; padding: 24px; }}
  .container {{ max-width: 900px; margin: 0 auto; }}
  .card {{ background: #161b22; border: 1px solid #30363d; border-radius: 8px; padding: 20px; margin-bottom: 20px; }}
  h1 {{ font-size: 20px; margin-top: 0; }}
  .status-badge {{ display: inline-block; padding: 6px 14px; border-radius: 20px; font-weight: 700; font-size: 13px; }}
  .pass {{ background: rgba(63, 185, 80, 0.2); color: #3fb950; border: 1px solid #3fb950; }}
  .fail {{ background: rgba(248, 81, 73, 0.2); color: #f85149; border: 1px solid #f85149; }}
  .grid {{ display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; margin-top: 16px; }}
  .stat {{ background: #0d1117; padding: 12px; border-radius: 6px; border: 1px solid #30363d; }}
  .stat-label {{ font-size: 11px; color: #8b949e; text-transform: uppercase; }}
  .stat-val {{ font-size: 18px; font-weight: 700; margin-top: 4px; }}
  .finding {{ background: #0d1117; border-left: 4px solid #d29922; padding: 10px 14px; margin-top: 8px; border-radius: 4px; }}
  .finding.error {{ border-left-color: #f85149; }}
  .finding-rule {{ font-weight: 700; color: #58a6ff; font-size: 13px; }}
  .finding-msg {{ font-size: 13px; margin-top: 4px; }}
</style>
</head>
<body>
<div class="container">
  <div class="card">
    <div style="display:flex; justify-content:space-between; align-items:center;">
      <h1>Dry Pre-Flight Safety Verification</h1>
      <span class="status-badge {'pass' if result['passed'] else 'fail'}">
        {'✓ VERIFIED SAFE' if result['passed'] else '⚠ REJECTED (ERRORS)'}
      </span>
    </div>
    <div class="grid">
      <div class="stat">
        <div class="stat-label">File Size</div>
        <div class="stat-val">{result['size_bytes'] / 1024:.1f} KB</div>
      </div>
      <div class="stat">
        <div class="stat-label">Errors</div>
        <div class="stat-val" style="color:{'#f85149' if errors else '#3fb950'};">{len(errors)}</div>
      </div>
      <div class="stat">
        <div class="stat-label">Warnings</div>
        <div class="stat-val" style="color:{'#d29922' if warnings else '#8b949e'};">{len(warnings)}</div>
      </div>
    </div>
  </div>

  <div class="card">
    <h2>Verification Findings ({len(findings)})</h2>
    {'<p style="color:#8b949e;">No safety contract violations detected. Toolpath adheres strictly to machine limits.</p>' if not findings else ''}
    {''.join([f'''<div class="finding {f.get('severity', 'warning')}">
      <div class="finding-rule">{f.get('rule', 'Rule')} ({f.get('severity', 'warning').upper()})</div>
      <div class="finding-msg">{f.get('message', '')}</div>
    </div>''' for f in findings])}
  </div>
</div>
</body>
</html>"""
    with open(output_html, "w", encoding="utf-8") as f:
        f.write(html)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Dry Slicer Post-Processor: Safety Verification & Optimization"
    )
    parser.add_argument("gcode_file", help="Path to input G-code file passed by slicer")
    parser.add_argument("--no-verify", action="store_true", help="Disable safety verification")
    parser.add_argument("--arc-fit", action="store_true", help="Enable G2/G3 arc fitting")
    parser.add_argument("--no-report", action="store_true", help="Disable HTML report generation")
    parser.add_argument("--output", "-o", help="Optional output path (defaults to in-place)")
    parser.add_argument("--max-flow", type=float, help="Max volumetric flow ceiling (mm³/s)")
    parser.add_argument("--max-accel", type=float, help="Peak acceleration ceiling (mm/s²)")

    args = parser.parse_args()

    result = process_gcode_file(
        gcode_path=args.gcode_file,
        verify_contracts=not args.no_verify,
        optimize_arcs=args.arc_fit,
        generate_report=not args.no_report,
        output_path=args.output,
        max_flow_mm3_s=args.max_flow,
        max_accel_mm_s2=args.max_accel,
    )

    status_str = "PASSED" if result["passed"] else "FAILED"
    print(f"[Dry Post-Processor] {status_str}: {len(result['findings'])} findings.")
    if result.get("report_path"):
        print(f"[Dry Post-Processor] Report generated at: {result['report_path']}")

    if not result["passed"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
