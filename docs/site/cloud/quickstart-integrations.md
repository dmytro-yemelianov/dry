---
title: Dry Cloud integration quickstart
---

# Integration quickstart

This flow turns a device-login access token into a revocable API key, submits raw
G-code, and polls the resulting job.

```bash
export DRY_CLOUD_URL=https://cloud.dry.yemelianov.dev
export DRY_ACCESS_TOKEN='<device-flow access token>'
```

See [Device authorization](/cloud/api#device-authorization) if you do not yet have
an access token.

## 1. Create an API key

```bash
curl -sS -X POST "$DRY_CLOUD_URL/v1/keys" \
  -H "Authorization: Bearer $DRY_ACCESS_TOKEN" \
  -H 'Content-Type: application/json' \
  --data '{"label":"production-ci"}'
```

Save the returned `key` immediately; it is not shown again.

```bash
export DRY_TOKEN='dry_key_secret-value'
```

The MVP allows one active API key. Revoke it with
`DELETE /v1/keys/{id}` before rotating.

## 2. Submit G-code

Use an immutable pack version in automated workflows:

```bash
export PACK_ID='<pack-id>'
export PACK_VERSION='<pack-version>'

curl -sS -X POST \
  "$DRY_CLOUD_URL/v1/jobs/verify?pack=$PACK_ID&version=$PACK_VERSION" \
  -H "Authorization: Bearer $DRY_TOKEN" \
  -H 'Content-Type: text/plain' \
  --data-binary @part.gcode
```

The HTTP 202 response contains a job id and relative `status_url`:

```json
{"id":"job-id","status_url":"/v1/jobs/job-id"}
```

## 3. Poll the job

```bash
curl -sS \
  -H "Authorization: Bearer $DRY_TOKEN" \
  "$DRY_CLOUD_URL/v1/jobs/job-id"
```

Poll with backoff while status is `queued` or `running`. On `done`, consume the
inlined `report.findings`; treat any finding whose `severity` is `error` as a failed
gate. On `error`, log both `stage` and `error`.

## 4. Observe quota

```bash
curl -sS \
  -H "Authorization: Bearer $DRY_TOKEN" \
  "$DRY_CLOUD_URL/v1/usage"
```

The free MVP quota is 20 jobs per UTC month. A quota response is HTTP 429 with a
`Retry-After` header and `/v1/usage` link. Usage billing is planned later and is not
active today.

## Operational notes

- Keep API keys in a secret manager; Dry stores only SHA-256 hashes.
- Pin pack versions for deterministic automation.
- Set an explicit client-side upload and poll timeout.
- A clean Dry report is evidence from the enabled static rules, not certification
  that a machine can run unattended.
- Registry schemas, publishing, and pack artifacts belong to the public
  [printer-registry project](https://github.com/dmytro-yemelianov/dry-printer-registry);
  Dry Cloud consumes that service read-only.
