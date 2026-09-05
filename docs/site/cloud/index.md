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

## What is implemented

- RFC 8628 device login for the CLI, plus opaque API keys for integrations.
- Asynchronous verification jobs for raw G-code files up to 100 MB.
- Printer/profile resolution through the separate public
  [Dry printer registry](https://github.com/dmytro-yemelianov/dry-printer-registry).
- Monthly usage reporting and a configurable baseline quota of 20 verification jobs.
- Reports with stable finding rules, severities, segments, and messages.

Start with the [CLI quickstart](/cloud/quickstart-cli), use the
[integration quickstart](/cloud/quickstart-integrations) for `curl`, or go directly
to the [API reference](/cloud/api).

## Pricing state

The checked-in deployment configuration uses 20 verification jobs per UTC month and
one active API key per account. These are pre-launch defaults, not a public offer.
Usage-based billing is not active. Quota and usage values are exposed by
`GET /v1/usage` so clients do not need to hard-code them.

## Current trust boundary

Cloud verification runs the same deterministic import-and-verify path as the local
CLI, but a clean report means only that the enabled Dry rules found no violations.
It is not machine certification and does not replace controlled machine validation.

The MVP activation form accepts an asserted email address but does not yet send an
email-verification challenge. Treat account email as a user-provided identifier, not
as independently verified identity.

The Worker, persistence bindings, Queue dispatch and private container contract are
implemented and tested, but no live production origin is claimed. Provisioning,
authenticated staging smoke, rollback evidence and the data-handling policy remain
launch gates. Examples therefore use `$DRY_CLOUD_URL`; set it only to an origin you
operate or whose deployment has been announced.
