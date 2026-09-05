# Operations and data handling

**Status:** D7 draft, reconciled 2026-09-05. The topology is implemented but no live production
deployment is claimed.

## Data flow

1. `services/cloud` authenticates the caller, enforces quota and rejects a missing or greater-than-
   100-MB `Content-Length`.
2. The raw G-code body is stored in R2 under `uploads/<job-id>`; D1 stores ownership, requested pack,
   immutable version, profile and job status.
3. A Queue message identifies the job. `services/cloud` reads the R2 object and streams its body to
   the private `containers/verify-runner`; the runner fetches the profile, writes the input to a
   bounded tempfile, runs the native engine and returns the report.
4. `services/cloud` stores that report under `reports/<job-id>.json`; D1 records completion or a
   fail-closed stage.
5. Only the owning account can poll the job and retrieve the report.

Uploaded programs and reports are customer intellectual property. Reports may repeat coordinates,
feedrates and machine constraints from the input; treating reports as harmless metadata is unsafe.

## Stored data

- **D1:** asserted account email, opaque-token hashes, API-key hashes/labels, job metadata and monthly
  usage events.
- **R2:** raw uploads and completed report JSON.
- **Queue/DLQ:** job identifiers and processing metadata, not raw G-code.
- **Runner tempfile:** one input during processing; deleted when the request scope ends.
- **Logs/metrics:** operational identifiers, stages, durations and counts. Raw bodies, tokens and full
  findings must not be logged.

## Policy gaps — launch blockers

Before real customer data is accepted, approve and implement:

- retention periods for uploads, reports, job/account metadata and logs;
- user-visible deletion and account-erasure procedures, including queue/DLQ races;
- data-residency/jurisdiction selection for D1, R2, Queues, Containers and logs;
- incident response, access review and support-data handling;
- log redaction tests and a statement of which finding fields may enter diagnostics.

There is no implied indefinite retention and no current production promise. Repository defaults are
implementation scaffolding, not an approved policy.

## Health and diagnosis

- Public `GET /healthz` proves only the Worker/control-plane route is serving the exact health
  contract.
- The runner's private `/healthz` and `/metrics` diagnose container health and refusal stages.
- A true end-to-end smoke must authenticate, upload a bounded fixture, observe queue processing and
  retrieve the expected report. It must run in staging without logging the fixture or token.

Common fail-closed stages are `profile-unavailable`, `input-invalid`, `engine-error`,
`queue-send-failed`, `unauthorized` and `rate_limited`. A malformed auth/contract input must never
fall back to permissive defaults.

## Capacity

The public upload contract is 100 MB. Measured import amplification is roughly 43–50×, so production
and staging configure a Cloudflare `standard-3` container (8 GiB) rather than a Worker isolate. This
is a sizing rationale, not an SLO. The load benchmark must gain asserted latency/error thresholds on
production-like infrastructure before launch.

## Deployment and rollback

The workflow validates both named Wrangler environments and can deploy staging from `main` or
production from a version tag when the protected environment supplies credentials and resource ids.
A configured deploy must pass the exact control-plane health response.

Rollback is not proven by documentation. Before launch, deploy two distinguishable staging
revisions, roll back to the prior revision, execute the authenticated job smoke, and record the
commands, restored revision, image digest and recovery time. Until that evidence exists, D4 remains
partially complete.
