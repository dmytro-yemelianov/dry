---
title: Dry Cloud
---

# Dry Cloud

Dry Cloud is the opt-in hosted verification service for Dry. Submit G-code against a
versioned printer pack, let the native Dry engine verify it asynchronously, and
retrieve the same structured report format used by local Dry.

Local commands remain local: `dry verify`, simulation, import, rewrite, and emission
do not contact Dry Cloud. Network access happens only when you run `dry auth`,
`dry cloud`, or an explicitly networked printer-registry command.

## What is available

- RFC 8628 device login for the CLI, plus opaque API keys for integrations.
- Asynchronous verification jobs for raw G-code files up to 100 MB.
- Printer/profile resolution through the separate public
  [Dry printer registry](https://github.com/dmytro-yemelianov/dry-printer-registry).
- Monthly usage reporting and a free quota of 20 verification jobs.
- Reports with stable finding rules, severities, segments, and messages.

Start with the [CLI quickstart](/cloud/quickstart-cli), use the
[integration quickstart](/cloud/quickstart-integrations) for `curl`, or go directly
to the [API reference](/cloud/api).

## Pricing state

The MVP provides free quotas: 20 verification jobs per UTC month and one active API
key per account. Usage-based billing is planned for a later phase; it is not active
today. Quota and usage values are exposed by `GET /v1/usage` so clients do not need
to hard-code them.

## Current trust boundary

Cloud verification runs the same deterministic import-and-verify path as the local
CLI, but a clean report means only that the enabled Dry rules found no violations.
It is not machine certification and does not replace controlled machine validation.

The MVP activation form accepts an asserted email address but does not yet send an
email-verification challenge. Treat account email as a user-provided identifier, not
as independently verified identity.

The production hostname is planned as `https://cloud.dry.yemelianov.dev` and becomes
usable after the deployment task is complete. Until then, examples use
`$DRY_CLOUD_URL` so the same commands work against development and production.
