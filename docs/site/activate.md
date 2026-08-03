# Activation

A Dry license is a single line of text — an Ed25519-signed token, verified entirely offline by
the CLI binary. There is no phone-home, no account, no network call to check a license: `dry`
verifies the signature against public keys built into the binary and reads the expiry out of the
token itself.

## What we collect: nothing

Activation does not talk to any server. The token you get by email already contains everything
the binary needs (licensee, tier, machine count, expiry) and is verified with a public key
compiled in — checking it out, running it in CI, or running it on an air-gapped machine are all
the same operation. See the [air-gapped FAQ](#air-gapped-use) below.

## Activate locally

After buying a [Solo or Team license](/pricing), you'll get an email with a token that looks like:

```
DRY-LICENSE-V1.eyJpZCI6Ii4uLiJ9.MEUCIQ...
```

Store it once:

```sh
dry license activate 'DRY-LICENSE-V1.eyJpZCI6Ii4uLiJ9.MEUCIQ...'
```

You can also point it at a file containing the token:

```sh
dry license activate ./dry-license.token
```

This verifies the signature and writes the token to your platform's config directory
(`$XDG_CONFIG_HOME/dry/license.token`, or the OS equivalent). Check what's active any time:

```sh
dry license status
```

```
licensee:  Acme Robotics
tier:      team
expires:   2027-08-03
state:     valid
```

## Activate in CI: `DRY_LICENSE`

For CI, skip the file entirely — set the token as a secret environment variable. `DRY_LICENSE`
takes precedence over the stored file, so a single secret is all a pipeline needs:

```yaml
# .github/workflows/gate.yml
- name: Gate G-code
  env:
    DRY_LICENSE: ${{ secrets.DRY_LICENSE }}
  run: dry upload out.gcode --moonraker http://printer.local --json
```

Add the token as `DRY_LICENSE` under your repo's **Settings → Secrets and variables →
Actions**. No file to check in, no key management beyond the one secret. See the
[CI-gate quickstart](/guide/ci-gate-quickstart) for the full workflow.

## Grace period and renewal

A license doesn't stop working the moment it expires. There's a 14-day grace period after
`expires`: `dry license status` and every report prints a warning (`license for … is in its
grace period (N day(s) left)`), but nothing is refused. Only once the grace period elapses does
the CLI fall back to evaluation mode — full functionality, minus the `dry upload` gate, with the
`EVALUATION — not for production gating` banner. Nothing in your pipeline hard-fails because a
card expired over a weekend.

Renewing is the same as activating: request a fresh token (subscriptions auto-renew and reissue
by email; manual renewals go through the same checkout links on [pricing](/pricing)), then run
`dry license activate` again or roll the `DRY_LICENSE` secret.

## Air-gapped use

**Does this need network access?** No. Verification is pure local Ed25519 signature checking
against keys compiled into the `dry` binary — the same code path runs identically on a laptop
with no network, an isolated CI runner, or a machine-side controller with no internet route.

**What if my license expires while the machine is offline?** The 14-day grace period still
applies — it's computed from the token's `expires_unix` field against local wall-clock time, no
network round trip involved.

**Do I need to “check out” a license before going offline?** No — there is no checkout/online
state at all. Activation just writes the verified token to disk (or you set `DRY_LICENSE`); every
subsequent run re-verifies it locally.

**Does `dry review-gcode` / `verify` / `trace-gcode` need a license at all?** No — those run in
full in evaluation mode too. Only `dry upload` (the Moonraker print gate) requires a valid or
grace-period license, and it refuses locally, before any network call, if one isn't found.

**What does the binary send anywhere?** Nothing related to licensing. `dry upload` talks only to
the Moonraker host you point it at.
