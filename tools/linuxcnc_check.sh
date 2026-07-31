#!/usr/bin/env bash
# Validate emitted RS-274 G-code against the reference LinuxCNC interpreter (rs274).
#
# This is an independent oracle, not a syntax check: rs274 re-reads the G-code and
# emits canonical machine operations (STRAIGHT_FEED, ARC_FEED, SET_FEED_RATE, ...),
# so a file only passes if LinuxCNC agrees it describes real motion.
#
# Defaults to every .ngc under conformance/reports/cnc/. Runnable locally:
#   tools/linuxcnc_check.sh                        # check the CNC goldens
#   tools/linuxcnc_check.sh path/to/part.ngc       # check specific files
#   tools/linuxcnc_check.sh --canon part.ngc       # also print canonical ops
#   tools/linuxcnc_check.sh --out /tmp/canon       # keep canonical output
#
# Requires `rs274` (Debian package linuxcnc-uspace). On macOS, where LinuxCNC cannot
# run natively, the script re-executes itself inside a Lima guest named by
# $DRY_LINUXCNC_LIMA (default: linuxcnc) that must have this repo mounted at the
# same path. To create that guest:
#
#   limactl start --name=linuxcnc --vm-type=vz --mount-type=virtiofs \
#     --mount="$PWD:w" --cpus=4 --memory=4 template://debian-13
#   limactl shell linuxcnc -- sudo apt-get install -y linuxcnc-uspace   # after adding
#     # deb [arch=arm64,amd64 signed-by=/usr/share/keyrings/linuxcnc.gpg] \
#     #   https://www.linuxcnc.org/ trixie base 2.9-uspace 2.9-rt
#
# Only paths inside the repository can be checked from the host, since that is all
# the guest can see.
#
# Exit: 0 all files accepted, 1 one or more rejected, 2 usage or environment error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIMA_GUEST="${DRY_LINUXCNC_LIMA:-linuxcnc}"

show_canon=0
out_dir=""
files=()
while [ $# -gt 0 ]; do
  case "$1" in
    --canon) show_canon=1; shift ;;
    --out) out_dir="${2:-}"; [ -n "$out_dir" ] || { echo "--out requires a directory" >&2; exit 2; }; shift 2 ;;
    -h|--help) sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*) echo "unknown option '$1'" >&2; exit 2 ;;
    *) files+=("$1"); shift ;;
  esac
done

# Default target: the CNC goldens. Other conformance vectors are FDM/Marlin flavoured
# (E words, M117, G4 S) and are deliberately not valid RS-274 input.
if [ "${#files[@]}" -eq 0 ]; then
  while IFS= read -r f; do files+=("$f"); done < <(find "$ROOT/conformance/reports/cnc" -name '*.ngc' | sort)
  [ "${#files[@]}" -gt 0 ] || { echo "no .ngc files under conformance/reports/cnc" >&2; exit 2; }
fi

# Resolve to absolute paths before any cd, so relative args survive the temp-dir hop
# and the hand-off into the Lima guest.
abs=()
for f in "${files[@]}"; do
  if [ ! -f "$f" ]; then echo "not a file: $f" >&2; exit 2; fi
  abs+=("$(cd "$(dirname "$f")" && pwd)/$(basename "$f")")
done

# rs274 only exists on Linux. Delegate into the Lima guest when running on the host.
if ! command -v rs274 >/dev/null 2>&1; then
  if [ -n "${DRY_LINUXCNC_NESTED:-}" ]; then
    echo "rs274 not found inside guest '$LIMA_GUEST'; install it with: sudo apt-get install linuxcnc-uspace" >&2
    exit 2
  fi
  if command -v limactl >/dev/null 2>&1 && limactl list --format '{{.Name}}' 2>/dev/null | grep -qx "$LIMA_GUEST"; then
    # Only the repository is mounted into the guest, so anything outside it would
    # reappear as a confusing "not a file" once re-executed on the other side.
    for f in "${abs[@]}"; do
      case "$f" in
        "$ROOT"/*) ;;
        *) echo "outside the repository, so not visible inside guest '$LIMA_GUEST': $f" >&2; exit 2 ;;
      esac
    done
    args=()
    [ "$show_canon" -eq 1 ] && args+=(--canon)
    [ -n "$out_dir" ] && args+=(--out "$out_dir")
    # macOS ships bash 3.2, where "${args[@]}" on an empty array trips `set -u`.
    exec limactl shell "$LIMA_GUEST" -- env DRY_LINUXCNC_NESTED=1 \
      "$ROOT/tools/linuxcnc_check.sh" ${args[@]+"${args[@]}"} "${abs[@]}"
  fi
  echo "rs274 not found and no Lima guest '$LIMA_GUEST'; install linuxcnc-uspace or create the guest" >&2
  exit 2
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
[ -n "$out_dir" ] && mkdir -p "$out_dir"

# Minimal tool table: the goldens select T1. Override with DRY_TOOL_TBL for richer setups.
TBL="${DRY_TOOL_TBL:-$WORK/dry.tbl}"
[ -f "$TBL" ] || printf 'T1 P1 Z0.000000 D6.000000 ;6mm flat endmill\n' > "$TBL"

status=0
for f in "${abs[@]}"; do
  name="$(basename "$f")"
  canon="$WORK/$name.canon"
  # rs274 -g is batch mode (without it the interpreter prompts); it writes a .var
  # file into the working directory, so run it somewhere disposable.
  if err="$(cd "$WORK" && rs274 -g -t "$TBL" "$f" "$canon" 2>&1)"; then
    # canon lines look like: "   17 N..... STRAIGHT_TRAVERSE(0.0000, ...)"
    ops="$(grep -cE '^[[:space:]]*[0-9]+ N[^ ]* [A-Z_]+\(' "$canon" 2>/dev/null || true)"
    motion="$(grep -cE '^[[:space:]]*[0-9]+ N[^ ]* (STRAIGHT_FEED|STRAIGHT_TRAVERSE|ARC_FEED)\(' "$canon" 2>/dev/null || true)"
    # A clean exit with no canonical output means nothing was actually interpreted.
    if [ "${ops:-0}" -eq 0 ]; then
      echo "REJECTED: $name interpreted without emitting any canonical operation" >&2
      status=1
      continue
    fi
    echo "ok: $name ($ops canonical ops, $motion motion)"
    [ "$show_canon" -eq 1 ] && sed 's/^/    /' "$canon"
  else
    echo "REJECTED: $name" >&2
    printf '%s\n' "$err" | sed 's/^/    /' >&2
    status=1
  fi
  [ -n "$out_dir" ] && [ -f "$canon" ] && cp "$canon" "$out_dir/"
done

if [ "$status" -ne 0 ]; then
  echo "LinuxCNC rejected one or more files; the emitter produced G-code the reference interpreter will not run" >&2
  exit 1
fi
# rs274 has no --version flag; take it from the package when the host is Debian.
ver="$(dpkg-query -W -f='${Version}' linuxcnc-uspace 2>/dev/null || true)"
echo "all ${#abs[@]} file(s) accepted by LinuxCNC${ver:+ }${ver}"
