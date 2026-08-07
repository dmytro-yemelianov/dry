#!/usr/bin/env bash
# NVIDIA Build API Aider Wrapper for Dry
#
# Usage:
#   scripts/run_aider_nvidia.sh "Message/Task for Aider" file1 file2 ...
#

export OPENAI_API_BASE="https://integrate.api.nvidia.com/v1"
export OPENAI_API_KEY="${NVIDIA_API_KEY}"
AIDER_BIN="/Users/dmytro/Library/Python/3.9/bin/aider"

if [ ! -f "$AIDER_BIN" ]; then
    AIDER_BIN="$(which aider)"
fi

if [ -z "$1" ]; then
    echo "Usage: $0 \"task description\" [file1 file2 ...]"
    exit 1
fi

TASK="$1"
shift

echo "==> Running Aider via NVIDIA Build API (Llama 3.3 70B) for Dry ..."
exec "$AIDER_BIN" \
    --model openai/meta/llama-3.3-70b-instruct \
    --message "$TASK" \
    --no-auto-commits \
    --yes \
    "$@"
