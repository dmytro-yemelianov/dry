#!/usr/bin/env python3
"""LLM Integration Harness & Execution Helper for Dry (Toolpath Compiler Infrastructure).

Allows LLMs (OpenAI, Claude, Gemini, Llama, DeepSeek, etc.) to write, verify,
lower, and audit Dry IR toolpaths, Python/TS SDK code, and Rust engine passes
safely with structured JSON output and clear diagnostics.
"""

import json
import sys
import tempfile
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

DRY_LLM_SYSTEM_PROMPT = """\
You are an expert toolpath compiler engineer writing and auditing Dry IR and Dry SDK code.

### Dry Rules & Architecture:
1. Multi-level Intermediate Representation (Dry IR v0):
   - L0 (design intent): parametric features, geometric expressions.
   - L1 (path intent): ordered waypoints, extruder state, feature boundaries.
   - L2 (motion intent): machine-agnostic toolframe motion (XYZ + orientation) & process values.
   - L3 (target output): target-specific code (FFF G-code, CNC, laser, robot).

2. Core Invariants:
   - Toolframe (position + orientation) enables non-planar and 5-axis motion as native operations.
   - Units are strongly typed (Length, Speed, Volume, Flow, Temperature).
   - Pure functional core: `design(params) -> IR`.
   - Python SDK (`import dry`, `dry.Design()`), TS SDK (`@dry/sdk`), Rust Engine (`dry-core`).

3. Dry IR v0 JSON Structure:
   - Must conform to `spec/dry-ir-v0.schema.json`.
   - Canonical CLI invocation: `dry emit <input.dry.json>` or `dry review-gcode <input.gcode>`.
"""

def get_system_prompt() -> str:
    return DRY_LLM_SYSTEM_PROMPT

def validate_dry_ir(ir_path_or_content: str) -> dict:
    """Validate and lower a Dry IR JSON file or string using the dry CLI."""
    is_path = Path(ir_path_or_content).exists()
    if is_path:
        target_path = Path(ir_path_or_content)
        tmp_file = None
    else:
        tmp_file = tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False)
        tmp_file.write(ir_path_or_content)
        tmp_file.close()
        target_path = Path(tmp_file.name)

    try:
        cmd = ["cargo", "run", "-p", "dry-cli", "--bin", "dry", "--", "emit", str(target_path)]
        res = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True, timeout=60)
        if res.returncode == 0:
            return {"status": "success", "output": res.stdout.strip()}
        else:
            return {"status": "error", "error": res.stderr.strip() or res.stdout.strip()}
    except Exception as e:
        return {"status": "error", "error": str(e)}
    finally:
        if tmp_file and Path(tmp_file.name).exists():
            Path(tmp_file.name).unlink()

def run_dry_check() -> dict:
    """Run cargo test across the Dry workspace to verify correctness."""
    try:
        cmd = ["cargo", "test", "--workspace"]
        res = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True, timeout=180)
        if res.returncode == 0:
            return {"status": "success", "output": "Cargo test passed cleanly."}
        else:
            return {"status": "error", "error": res.stderr.strip() or res.stdout.strip()}
    except Exception as e:
        return {"status": "error", "error": str(e)}

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 dry_llm_runner.py [prompt|validate|check] [args...]")
        sys.exit(1)

    cmd = sys.argv[1]
    if cmd == "prompt":
        print(get_system_prompt())
    elif cmd == "validate":
        if len(sys.argv) < 3:
            print("Usage: python3 dry_llm_runner.py validate <file.json_or_content>")
            sys.exit(1)
        res = validate_dry_ir(sys.argv[2])
        print(json.dumps(res, indent=2))
    elif cmd == "check":
        res = run_dry_check()
        print(json.dumps(res, indent=2))
    else:
        print(f"Unknown command: {cmd}")
        sys.exit(1)

if __name__ == "__main__":
    main()
