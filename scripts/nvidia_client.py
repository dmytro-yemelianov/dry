#!/usr/bin/env python3
"""
NVIDIA Build API Client for Dry (Toolpath Compiler Infrastructure) Subagent Orchestration
"""
import os
import sys
import json
import time
import urllib.request
import urllib.error

NVIDIA_API_KEY = os.environ.get("NVIDIA_API_KEY", "")
BASE_URL = "https://integrate.api.nvidia.com/v1/chat/completions"

DEFAULT_SYSTEM_PROMPT = (
    "You are a senior toolpath compiler engineer and formal verification specialist working on Dry, "
    "a typed, units-aware, multi-level intermediate representation (Dry IR) and Rust engine for algorithmic machine toolpaths."
)

def query_nvidia_model(
    prompt: str,
    system_prompt: str = DEFAULT_SYSTEM_PROMPT,
    model: str = "meta/llama-3.3-70b-instruct",
    max_tokens: int = 4096,
    temperature: float = 0.2
) -> str:
    headers = {
        "Authorization": f"Bearer {NVIDIA_API_KEY}",
        "Content-Type": "application/json",
        "Accept": "application/json"
    }
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": prompt}
        ],
        "max_tokens": max_tokens,
        "temperature": temperature
    }
    for attempt in range(5):
        req = urllib.request.Request(BASE_URL, data=json.dumps(payload).encode('utf-8'), headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=120) as resp:
                res_data = json.loads(resp.read().decode('utf-8'))
                return res_data['choices'][0]['message']['content']
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as e:
            if attempt < 4:
                sleep_time = (attempt + 1) * 3
                sys.stderr.write(f"NVIDIA API request issue ({e}), retrying in {sleep_time}s (attempt {attempt+1}/5)...\n")
                time.sleep(sleep_time)
                continue
            sys.stderr.write(f"Error calling NVIDIA API ({model}): {e}\n")
            raise
    raise RuntimeError("Failed after 5 attempts")

if __name__ == "__main__":
    if len(sys.argv) > 1:
        user_prompt = " ".join(sys.argv[1:])
        print(query_nvidia_model(user_prompt))
    else:
        print("Dry NVIDIA client harness ready.")

