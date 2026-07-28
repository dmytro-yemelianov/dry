---
title: Dry Cloud API
---

# API reference

Set the API origin once:

```bash
export DRY_CLOUD_URL=https://cloud.dry.yemelianov.dev
```

JSON responses use `Content-Type: application/json`. Authenticated endpoints accept
`Authorization: Bearer <access-token-or-api-key>`.

## Device authorization

### `POST /v1/auth/device`

Starts an RFC 8628 device flow.

```bash
curl -sS -X POST "$DRY_CLOUD_URL/v1/auth/device"
```

```json
{
  "device_code": "opaque-device-code",
  "user_code": "ABCD-EFGH",
  "verification_uri": "https://cloud.dry.yemelianov.dev/activate",
  "verification_uri_complete": "https://cloud.dry.yemelianov.dev/activate?user_code=ABCD-EFGH",
  "expires_in": 600,
  "interval": 5
}
```

Open `verification_uri_complete` and approve the code. `GET /activate` displays the
form; `POST /activate` submits the code, email, and Turnstile response.

### `POST /v1/auth/token`

Poll no faster than `interval`.

```bash
curl -sS -X POST "$DRY_CLOUD_URL/v1/auth/token" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'grant_type=urn:ietf:params:oauth:grant-type:device_code' \
  --data-urlencode 'device_code=opaque-device-code'
```

Before approval, the response error is `authorization_pending`; polling too quickly
returns `slow_down`; expired or already-consumed codes return `expired_token`.
Success returns the access token once:

```json
{"access_token":"opaque-access-token","token_type":"Bearer"}
```

## Account and keys

### `GET /v1/me`

```json
{
  "account_id": "account-id",
  "email": "maker@example.com",
  "created_at": "2026-07-28 12:00:00"
}
```

The MVP email is asserted during activation and is not independently verified.

### `POST /v1/keys`

Creates an API key. The plaintext `key` is shown once.

```bash
curl -sS -X POST "$DRY_CLOUD_URL/v1/keys" \
  -H "Authorization: Bearer $DRY_ACCESS_TOKEN" \
  -H 'Content-Type: application/json' \
  --data '{"label":"ci"}'
```

```json
{"id":"sha256-key-id","key":"dry_key_secret-value"}
```

`GET /v1/keys` lists key ids, labels, and creation times, never key material.
`DELETE /v1/keys/{id}` revokes an owned key. The MVP quota is one active key.

## Verification jobs

### `POST /v1/jobs/verify`

Query parameters:

| Parameter | Required | Meaning |
|---|---:|---|
| `pack` | yes | Public printer-pack id |
| `version` | no | Immutable pack version; explicit values match exactly |
| `profile` | no | Profile id; otherwise the pack's first/default profile is used |

When `version` is omitted, the registry's first/default version is resolved and
stored on the job. Pin a version in CI and production integrations for reproducible
behavior.

The request body is raw G-code and must have `Content-Length`. The maximum is
100 MB.

```bash
curl -sS -X POST \
  "$DRY_CLOUD_URL/v1/jobs/verify?pack=$PACK_ID&version=$PACK_VERSION" \
  -H "Authorization: Bearer $DRY_TOKEN" \
  -H 'Content-Type: text/plain' \
  --data-binary @part.gcode
```

```json
{"id":"job-id","status_url":"/v1/jobs/job-id"}
```

Successful submission returns HTTP 202. Invalid lengths return 411 or 413; an
unknown explicit pack version returns 404; registry resolution failures return 502.
At the monthly quota, the response is HTTP 429:

```json
{"error":"quota_exceeded","usage_url":"/v1/usage"}
```

The accompanying `Retry-After` value is the number of seconds until the next UTC
month.

### `GET /v1/jobs/{id}`

Only the owning account can read a job. Queued/running responses include status and
metadata. A completed response inlines the report:

```json
{
  "id": "job-id",
  "status": "done",
  "pack_id": "pack-id",
  "created_at": "2026-07-28 12:00:00",
  "finished_at": "2026-07-28 12:00:03",
  "report": {
    "findings": [
      {
        "rule": "bounds",
        "severity": "error",
        "segment": 12,
        "message": "X is outside the build volume"
      }
    ]
  }
}
```

Failed jobs use `status: "error"` with `error` and `stage`. Current stages are
`profile-unavailable`, `input-invalid`, `engine-error`, and `queue-send-failed`.

## Usage

### `GET /v1/usage`

```json
{
  "month": {"jobs":3,"bytes":12345},
  "quotas": {"jobs_per_month":20,"keys":1}
}
```

The UTC-month job count is the same canonical count used for quota enforcement.
`bytes` is the sum of authenticated request body sizes recorded this month.
