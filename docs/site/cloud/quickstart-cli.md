---
title: Dry Cloud CLI quickstart
---

# CLI quickstart

Dry Cloud commands are opt-in. Existing local commands remain offline.

## 1. Log in

```bash
dry auth login
```

The CLI prints a short code and activation URL, waits at the server-provided poll
interval, and stores the granted token at `$XDG_CONFIG_HOME/dry/cloud-token`.
On Unix the file mode is `0600`.

For a development or private deployment:

```bash
dry auth login --cloud-url http://127.0.0.1:8787
```

`DRY_CLOUD_URL` overrides the hosted default. `DRY_TOKEN` overrides the saved token,
which is useful in CI.

Confirm the account and current quota:

```bash
dry auth status
```

## 2. Choose a printer pack

Printer packs live in the separate public
[Dry printer registry](https://github.com/dmytro-yemelianov/dry-printer-registry).
Inspect a pack and its immutable versions:

```bash
dry printer inspect <pack-id> --source https://api.dry.yemelianov.dev
```

You can also resolve and hash-check a profile artifact locally:

```bash
dry printer resolve <pack-id> \
  --version <pack-version> \
  --source https://api.dry.yemelianov.dev \
  --out printer-profile.json
```

## 3. Verify in the cloud

```bash
dry cloud verify part.gcode \
  --printer <pack-id> \
  --pack-version <pack-version>
```

The CLI uploads the raw file, polls the asynchronous job with a 1-to-5-second
backoff for at most 10 minutes, then prints the verdict and finding counts. It exits
with code 1 when the report contains error-severity findings, matching local
`dry verify`.

For the complete report:

```bash
dry cloud verify part.gcode \
  --printer <pack-id> \
  --pack-version <pack-version> \
  --json > verify-report.json
```

`--pack-version` may be omitted for interactive use, in which case the service
resolves the registry default. Pin it in CI for reproducible results.

## 4. Log out

```bash
dry auth logout
```

This removes the saved token. If `DRY_TOKEN` is still set, it continues to take
precedence and the CLI prints a warning.

## Local verification stays local

Use the local engine whenever a hosted job is unnecessary:

```bash
dry import-gcode part.gcode --out part.json
dry verify part.json --profile printer-profile.json --json
```

These commands do not read `DRY_CLOUD_URL` and do not access the network.
