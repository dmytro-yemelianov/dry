# CI-gate quickstart: 60 minutes to a gated pipeline

This is the fast path from "never used Dry" to "CI fails the build on a bad G-code file." No
license required for the first three steps — buy only once you've seen it catch something on
your own output.

## 1. Install (5 minutes)

Grab the Linux x86_64 CLI binary from the latest
[GitHub Release](https://github.com/dmytro-yemelianov/dry/releases/latest) — no build toolchain,
no package manager:

```sh
ver=$(curl -fsSL https://api.github.com/repos/dmytro-yemelianov/dry/releases/latest | grep -m1 '"tag_name"' | cut -d'"' -f4)
curl -fsSLO "https://github.com/dmytro-yemelianov/dry/releases/download/${ver}/dry-${ver#v}-x86_64-unknown-linux-gnu.tar.gz"
curl -fsSLO "https://github.com/dmytro-yemelianov/dry/releases/download/${ver}/SHA256SUMS"
grep "x86_64-unknown-linux-gnu.tar.gz" SHA256SUMS | sha256sum -c -
tar xzf "dry-${ver#v}-x86_64-unknown-linux-gnu.tar.gz"
./dry-${ver#v}-x86_64-unknown-linux-gnu/dry --help
```

macOS (aarch64 or x86_64) and Windows x86_64 binaries are on the same release page — swap the
target triple (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`). See
[Releasing Dry](https://github.com/dmytro-yemelianov/dry/blob/main/docs/12-releasing.md) for the
full artifact list. Put `dry` on your `PATH` for the rest of this guide.

## 2. Run eval on your own G-code (10 minutes)

No license needed — point it at whatever your slicer already produced:

```sh
dry review-gcode your-part.gcode --json > review.json
```

This runs the full review engine: motion/flow/temperature/retraction checks, source-located
findings, everything. In evaluation mode the JSON is stamped `"mode": "evaluation"` and printed
human output carries an `EVALUATION — not for production gating` banner — the review itself is
not limited or truncated.

## 3. Read the findings (10 minutes)

```sh
dry review-gcode your-part.gcode
```

```
review-gcode: your-part.gcode
  segments:  4213 (3980 moves with length)
  time:      1842.3s (print 1690.1s, travel 152.2s)
  findings:  1 error, 2 warnings
    error   cold-extrusion       line 118   nozzle at 195°C, min-temp 200°C
    warning long-travel-no-retract line 892  travel 34.2mm without retraction
    ...
```

Findings are grouped `error` / `warning`; each carries the source line it traces back to. `dry
review-gcode` exits `1` if any `error`-level finding is present — that's the signal your CI job
gates on. Tune the checks that matter for your machine with the profile/threshold flags (`--bounds`,
`--min-temp`, `--speed-range`, `--max-flow`, …) — see `dry review-gcode --help`, or supply a
`--profile` JSON with your machine/material limits baked in.

## 4. Buy and set the secret (10 minutes)

Once eval has shown you real findings on real output, pick a tier on [/pricing](/pricing) and
check out. The license token arrives by email within a few minutes — see
[Activation](/activate) for the full flow. For CI you only need one repo secret:

1. GitHub repo → **Settings → Secrets and variables → Actions → New repository secret**
2. Name: `DRY_LICENSE`
3. Value: the full `DRY-LICENSE-V1...` token from the activation email

## 5. The gating job (20 minutes)

Drop this in `.github/workflows/gate.yml`. It installs `dry`, runs the review, and fails the
build on any error-level finding — exit code `1` from `dry review-gcode` propagates straight to
the job:

```yaml
name: Gate G-code

on:
  pull_request:
    paths:
      - '**/*.gcode'

jobs:
  review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install dry
        run: |
          ver=$(curl -fsSL https://api.github.com/repos/dmytro-yemelianov/dry/releases/latest \
            | grep -m1 '"tag_name"' | cut -d'"' -f4)
          curl -fsSLO "https://github.com/dmytro-yemelianov/dry/releases/download/${ver}/dry-${ver#v}-x86_64-unknown-linux-gnu.tar.gz"
          tar xzf "dry-${ver#v}-x86_64-unknown-linux-gnu.tar.gz"
          echo "$PWD/dry-${ver#v}-x86_64-unknown-linux-gnu" >> "$GITHUB_PATH"

      - name: Review and gate
        env:
          DRY_LICENSE: ${{ secrets.DRY_LICENSE }}
        shell: bash
        run: |
          set -o pipefail
          dry review-gcode out/part.gcode --json | tee review.json

      - name: Upload review report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: dry-review
          path: review.json
```

`review-gcode` exits non-zero the moment an error-level finding is present, so the `Review and
gate` step is the whole gate. The `set -o pipefail` matters: without it, the step's exit status is
`tee`'s (always 0), and an error-level finding would silently pass the build. Point `--profile` at
a committed machine/material JSON if you want firmware-specific limits (temperature, flow, bounds)
enforced
instead of the defaults.

## 6. Gate the print itself: `dry upload --moonraker`

Reviewing G-code in CI catches problems before a file ever reaches a printer. To gate at the
print side too — the same checks, but refusing to start a print rather than failing a build —
use `dry upload` against a Moonraker-connected printer. This is the one command that requires a
valid or grace-period license (it refuses locally, before any network call, in evaluation mode):

```sh
export DRY_LICENSE="$(cat dry-license.token)"
dry upload out/part.gcode \
  --moonraker http://printer.local \
  --profile machine.json \
  --print \
  --json
```

`--print` starts the print only if the gate is clean (or `--force` to override deliberately);
without `--print` it just uploads the file after gating. Point `--moonraker` at any
Moonraker-API host reachable from wherever this runs — a CI runner with network access to the
farm, or a small always-on machine next to the printers. `--api-key-env` names the environment
variable holding your Moonraker API key (default `MOONRAKER_API_KEY`) if your instance requires
one.

That's the full spine: eval on your own files → buy → one secret → CI gate → print-side gate.
