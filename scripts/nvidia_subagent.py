#!/usr/bin/env python3
"""NVIDIA Build API Subagent Orchestration & Hard-Work Engine for Dry.

This runner executes heavy subagent tasks (e.g. Dry IR generation, formal proof claim verification,
Lean 4 theorem synthesis, G-code lowering auditing, or Rust kernel numerical passes)
by offloading heavy computation to the NVIDIA Build API (DeepSeek-R1, DeepSeek V4, Llama 3.3 70B, Qwen 2.5 72B, Mixtral).

Gemini 3.6 Flash (High) acts as the high-level orchestrator, verifier, and scout, delegating
computationally intensive kernel/proof tasks to these NVIDIA API subagents.
"""

import argparse
import json
import os
import socket
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NVIDIA_API_KEY = os.environ.get("NVIDIA_API_KEY", "")
BASE_URL = "https://integrate.api.nvidia.com/v1/chat/completions"

AVAILABLE_MODELS = {
    "llama": "meta/llama-3.3-70b-instruct",
    "llama-70b": "meta/llama-3.3-70b-instruct",
    "deepseek-r1": "deepseek-ai/deepseek-r1",
    "deepseek": "deepseek-ai/deepseek-r1",
    "deepseek-v4": "deepseek-ai/deepseek-v4-pro",
    "mixtral": "mistralai/mixtral-8x22b-instruct-v0.1",
    "qwen": "qwen/qwen2.5-72b-instruct",
    "qwen-72b": "qwen/qwen2.5-72b-instruct",
}

TASK_PROFILES = {
    "proof": {
        "model": "deepseek-ai/deepseek-r1",
        "temperature": 0.1,
        "max_tokens": 8192,
        "system_prompt": (
            "You are a formal verification specialist and mathematical assurance engineer working on Dry. "
            "Your task is Lean 4 theorem proving, numeric boundary analysis, precision floating-point error budgeting, "
            "and proving structural invariants of the Dry IR multi-level toolpath compiler."
        ),
    },
    "kernel": {
        "model": "meta/llama-3.3-70b-instruct",
        "temperature": 0.15,
        "max_tokens": 8192,
        "system_prompt": (
            "You are a senior Rust kernel engineer and computational geometry specialist working on Dry's core engine (`crates/core`). "
            "You write robust, unit-typed, dependency-free Rust code for L0/L1/L2 IR lowering, toolframe orientation math, "
            "5-axis kinematics (AB/BC/AC), TPMS implicit surface slicing, clothoid curves, and G-code emitters."
        ),
    },
    "heavy-dev": {
        "model": "meta/llama-3.3-70b-instruct",
        "temperature": 0.2,
        "max_tokens": 8192,
        "system_prompt": (
            "You are a senior systems software developer working on Dry. "
            "You build multi-language bindings (Python PyO3, TypeScript/WASM), Moonraker/Cloud APIs, "
            "and comprehensive CLI tooling."
        ),
    },
    "audit": {
        "model": "deepseek-ai/deepseek-r1",
        "temperature": 0.1,
        "max_tokens": 4096,
        "system_prompt": (
            "You are a security, contract, and correctness auditor for toolpath compilers. "
            "Analyze code, specifications, and test suites for non-finite value leaks (NaN/inf), "
            "boundary overflows, unrepresented G-code states, and protocol non-conformance."
        ),
    },
}

DEFAULT_SYSTEM_PROMPT = (
    "You are an expert toolpath compiler engineer, computational geometry specialist, "
    "and formal verification developer working on the Dry project."
)


def call_nvidia(
    prompt: str,
    system_prompt: str = DEFAULT_SYSTEM_PROMPT,
    model_alias: str = "llama",
    max_tokens: int = 4096,
    temperature: float = 0.2,
) -> str:
    model_name = AVAILABLE_MODELS.get(model_alias, model_alias)
    headers = {
        "Authorization": f"Bearer {NVIDIA_API_KEY}",
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    payload = {
        "model": model_name,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": prompt},
        ],
        "max_tokens": max_tokens,
        "temperature": temperature,
    }
    for attempt in range(10):
        req = urllib.request.Request(
            BASE_URL,
            data=json.dumps(payload).encode("utf-8"),
            headers=headers,
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                res_data = json.loads(resp.read().decode("utf-8"))
                return res_data["choices"][0]["message"]["content"]
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, socket.timeout, Exception) as e:
            if attempt < 9:
                sleep_time = min(30, (2 ** attempt) + 2)
                sys.stderr.write(
                    f"NVIDIA API request issue ({e}), retrying in {sleep_time}s (attempt {attempt+1}/10)...\n"
                )
                time.sleep(sleep_time)
                continue
            sys.stderr.write(f"Error calling NVIDIA API ({model_name}): {e}\n")
            raise
    raise RuntimeError("Failed after 10 attempts")


def main():
    parser = argparse.ArgumentParser(
        description="NVIDIA API Subagent Runner for Dry Heavy Work"
    )
    parser.add_argument("prompt", type=str, help="Task prompt for the subagent")
    parser.add_argument(
        "--profile",
        type=str,
        choices=list(TASK_PROFILES.keys()),
        help="Task profile: proof, kernel, heavy-dev, audit",
    )
    parser.add_argument(
        "--model",
        type=str,
        default="llama",
        help="Model alias: llama, deepseek-r1, deepseek-v4, mixtral, qwen-72b",
    )
    parser.add_argument(
        "--system",
        type=str,
        default=None,
        help="System prompt override",
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        default=None,
        help="Max tokens override",
    )
    parser.add_argument(
        "--temperature",
        type=float,
        default=None,
        help="Temperature override",
    )
    parser.add_argument(
        "--out", type=str, help="Path to write the returned output string"
    )
    args = parser.parse_args()

    if args.profile and args.profile in TASK_PROFILES:
        prof = TASK_PROFILES[args.profile]
        model_alias = prof["model"]
        system_prompt = prof["system_prompt"]
        max_tokens = prof["max_tokens"]
        temperature = prof["temperature"]
    else:
        model_alias = args.model
        system_prompt = args.system or DEFAULT_SYSTEM_PROMPT
        max_tokens = args.max_tokens or 4096
        temperature = args.temperature if args.temperature is not None else 0.2

    if args.system:
        system_prompt = args.system
    if args.max_tokens:
        max_tokens = args.max_tokens
    if args.temperature is not None:
        temperature = args.temperature

    print(
        f"[NVIDIA Subagent] Profile: {args.profile or 'custom'} | Model: {model_alias} | Task: {args.prompt[:80]}..."
    )
    result = call_nvidia(
        prompt=args.prompt,
        system_prompt=system_prompt,
        model_alias=model_alias,
        max_tokens=max_tokens,
        temperature=temperature,
    )

    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(result, encoding="utf-8")
        print(f"[NVIDIA Subagent] Output saved to {out_path}")
    else:
        print("\n--- Subagent Output ---")
        print(result)


if __name__ == "__main__":
    main()

