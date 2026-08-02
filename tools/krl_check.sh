#!/usr/bin/env bash
# Validate emitted KRL against an external KUKA Robot Language grammar.
#
# This is an independent structural oracle, not a Dry self-check: the grammar is
# `kuka/krl.g4` from antlr/grammars-v4 (Jan Schloessin 2010-2011, from the reverse-
# engineering study arXiv:1009.5004; ANTLR4 port by Tom Everett 2016, LGPL-3.0-or-later).
# Nobody here wrote it and nobody here can quietly relax it, which is the point --
# a program accepted by Dry's own parser proves only that Dry agrees with itself.
#
# WHAT THIS DOES NOT ESTABLISH. A KUKA controller is not a parser. This check says the
# program is syntactically a KRL module; it says nothing about whether a KRC would load
# it, whether the poses are reachable, or whether the motion is what was intended. Dry's
# KRL output has never been run on a controller or on a simulator. KUKA OfficeLite is
# proprietary and Windows-only and there is no free implementation to run it against;
# see docs/22-krl-emit.md for what was searched for and what was found.
#
# The grammar is fetched at check time (pinned commit + SHA-256) rather than vendored:
# it is LGPL and this repository is proprietary, and a copy in-tree would be a copy we
# could edit. The ANTLR tool jar is pinned and checksummed the same way, and cached under
# $XDG_CACHE_HOME/dry rather than in the Maven repository layout.
#
# Defaults to every .src under conformance/reports/robot/. Runnable locally:
#   tools/krl_check.sh                        # check the robot goldens
#   tools/krl_check.sh path/to/part.src       # check specific files
#   tools/krl_check.sh --verbose part.src     # also print the program
#
# Requires Java (for the ANTLR tool) and the ANTLR Python runtime:
#   pip install 'antlr4-python3-runtime==4.13.2'
#   # macOS: brew install openjdk   (/usr/bin/java is a stub with no JRE behind it)
#   # Debian/Ubuntu/CI: apt-get install -y default-jre-headless
# Override the discovered pieces with JAVA_BIN, ANTLR4_JAR and DRY_PYTHON. An ANTLR4_JAR
# still has to match the pinned SHA-256 -- a jar this script did not choose is a parser
# generator this script did not choose.
#
# Exit: 0 all files accepted, 1 one or more rejected, 2 usage or environment error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Pinned so a re-run checks the same grammar. Bumping either line is a deliberate change
# to what "structurally well-formed" means here, and must be reported as one.
GRAMMAR_COMMIT="753536777d827ccc0c9b108531ea67375c2039ac"
GRAMMAR_SHA256="b4797b2480bcf9ebe35c55338dfe4935ba759112600f77e3c7a5bce18f037d9e"
GRAMMAR_URL="https://raw.githubusercontent.com/antlr/grammars-v4/${GRAMMAR_COMMIT}/kuka/krl.g4"
ANTLR_VERSION="4.13.2"
ANTLR_URL="https://repo1.maven.org/maven2/org/antlr/antlr4/${ANTLR_VERSION}/antlr4-${ANTLR_VERSION}-complete.jar"
# The jar generates and hosts the parser, so an unverified one defeats the oracle completely --
# checking the grammar's bytes and then handing them to an arbitrary compiler proves nothing.
# Cross-checked against Maven Central's own published .sha1 for this artifact
# (7df86c341abb175a0f4b76a7845074cc45f82ff8) before being recorded here; Maven publishes no .sha256.
ANTLR_SHA256="eae2dfa119a64327444672aff63e9ec35a20180dc5b8090b7a6ab85125df4d76"
# Not $HOME/.m2: writing an artifact this script fetched into the Maven repository layout puts it
# where other tools resolve dependencies from, and it is not Maven's to populate.
ANTLR_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/dry/antlr"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

verbose=0
files=()
while [ $# -gt 0 ]; do
  case "$1" in
    --verbose) verbose=1; shift ;;
    -h|--help) sed -n '2,33p' "${BASH_SOURCE[0]}" | sed -n 's/^# \{0,1\}//p'; exit 0 ;;
    -*) echo "unknown option '$1'" >&2; exit 2 ;;
    *) files+=("$1"); shift ;;
  esac
done

if [ "${#files[@]}" -eq 0 ]; then
  while IFS= read -r f; do files+=("$f"); done < <(find "$ROOT/conformance/reports/robot" -name '*.src' 2>/dev/null | sort)
  [ "${#files[@]}" -gt 0 ] || { echo "no .src files under conformance/reports/robot" >&2; exit 2; }
fi

abs=()
for f in "${files[@]}"; do
  if [ ! -f "$f" ]; then echo "not a file: $f" >&2; exit 2; fi
  abs+=("$(cd "$(dirname "$f")" && pwd)/$(basename "$f")")
done

# --- java -------------------------------------------------------------------------
# On macOS /usr/bin/java exists as a stub that fails with "Unable to locate a Java
# Runtime", so presence on PATH is not enough -- it has to actually run.
java_bin="${JAVA_BIN:-}"
if [ -z "$java_bin" ]; then
  for candidate in java /opt/homebrew/opt/openjdk/bin/java /usr/local/opt/openjdk/bin/java; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -version >/dev/null 2>&1; then
      java_bin="$candidate"; break
    fi
  done
fi
if [ -z "$java_bin" ]; then
  echo "no working Java runtime found (the ANTLR tool needs one)." >&2
  echo "  macOS:  brew install openjdk   then re-run, or set JAVA_BIN" >&2
  echo "  Debian: sudo apt-get install -y default-jre-headless" >&2
  exit 2
fi

python_bin="${DRY_PYTHON:-python3}"
command -v "$python_bin" >/dev/null 2>&1 || { echo "no $python_bin on PATH" >&2; exit 2; }
if ! "$python_bin" -c 'import antlr4' >/dev/null 2>&1; then
  echo "the ANTLR Python runtime is not importable from $python_bin." >&2
  echo "  pip install 'antlr4-python3-runtime==${ANTLR_VERSION}'   (or set DRY_PYTHON)" >&2
  exit 2
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --- the grammar ------------------------------------------------------------------
grammar="$WORK/krl.g4"
if ! curl -sSfL -o "$grammar" "$GRAMMAR_URL"; then
  echo "could not fetch the KRL grammar from $GRAMMAR_URL" >&2
  exit 2
fi
got="$(sha256_of "$grammar")"
if [ "$got" != "$GRAMMAR_SHA256" ]; then
  echo "KRL grammar checksum mismatch: expected $GRAMMAR_SHA256, got $got" >&2
  echo "the pinned commit's content changed, or the download was tampered with; refusing to run" >&2
  exit 2
fi

# --- the ANTLR tool ---------------------------------------------------------------
jar="${ANTLR4_JAR:-$ANTLR_CACHE/antlr4-${ANTLR_VERSION}-complete.jar}"
if [ ! -f "$jar" ]; then
  mkdir -p "$(dirname "$jar")"
  # Download to a sibling and move only once the checksum holds, so an interrupted or tampered
  # fetch never leaves something the next run would trust because it is already on disk.
  if ! curl -sSfL -o "$jar.part" "$ANTLR_URL"; then
    echo "could not fetch the ANTLR tool from $ANTLR_URL; set ANTLR4_JAR to a local copy" >&2
    rm -f "$jar.part"
    exit 2
  fi
  mv "$jar.part" "$jar"
fi
got="$(sha256_of "$jar")"
if [ "$got" != "$ANTLR_SHA256" ]; then
  echo "ANTLR tool checksum mismatch for $jar" >&2
  echo "  expected $ANTLR_SHA256" >&2
  echo "  got      $got" >&2
  echo "the jar generates and hosts the parser, so running an unpinned one would make the check" >&2
  echo "meaningless; refusing to run. Delete it to re-fetch, or update ANTLR_SHA256 deliberately." >&2
  exit 2
fi

gen="$WORK/gen"
if ! "$java_bin" -cp "$jar" org.antlr.v4.Tool -Dlanguage=Python3 -o "$gen" "$grammar"; then
  echo "ANTLR could not generate a parser from the pinned grammar" >&2
  exit 2
fi

# --- check --------------------------------------------------------------------------
args=(--gen "$gen")
[ "$verbose" -eq 1 ] && args+=(--verbose)
if ! "$python_bin" "$ROOT/tools/krl_parse.py" "${args[@]}" "${abs[@]}"; then
  echo "the external KRL grammar rejected one or more files; the emitter produced something that is not a KRL module" >&2
  exit 1
fi
echo "all ${#abs[@]} file(s) accepted by grammars-v4 kuka/krl.g4 @ ${GRAMMAR_COMMIT:0:12} (structure only — never run on a controller)"
