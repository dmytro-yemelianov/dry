// Async verify jobs: POST /v1/jobs/verify (upload -> R2 -> D1 `queued` row -> queue
// send), the queue consumer (job -> container -> report -> R2 + D1 `done`/`error`),
// and GET /v1/jobs/{id} (owner-only status/report read).
//
// Interfaces and failure taxonomy per the R3 task brief and its Global Constraints;
// see also containers/verify-runner's README for the runner's own `{error, stage}`
// contract this consumer maps onto.

import { type ContainerStubLike, containerFetch as defaultContainerFetch, getContainerStub as defaultGetContainerStub } from "./container";

/** Global Constraints: "Upload cap: 100 MB (container has 6 GiB; the Worker
 * enforces Content-Length)." Not a var (unlike the QUOTA_* knobs below) — the brief
 * states it as a fixed product invariant, not an operator-tunable setting. */
const MAX_UPLOAD_BYTES = 100 * 1024 * 1024;

function jsonResponse(value: unknown, status = 200, extraHeaders?: HeadersInit): Response {
  const headers = new Headers(extraHeaders);
  headers.set("content-type", "application/json; charset=utf-8");
  return Response.json(value, { status, headers });
}

function parseQuota(raw: string | undefined, fallback: number): number {
  const parsed = Number.parseInt(raw ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

/**
 * MVP default-profile rule (documented per the R3 task brief's explicit request):
 * when `POST /v1/jobs/verify` omits `profile=`, resolve it via the registry's
 * GraphQL API (docs/19-printer-registry-api.md; query shape mirrors
 * `resolve_profile` in crates/cli/src/printer_registry.rs) and pick the FIRST
 * profile listed for the pack version that matches the requested `version` (or the
 * first version entry the registry returns, if none matches exactly) — i.e. "first
 * listed profile of the pack manifest," the deterministic rule the task brief
 * explicitly sanctions as acceptable for MVP. No material/nozzle filtering is
 * applied; a pack that needs a specific profile for correct results should pass
 * `profile=` explicitly rather than rely on this default.
 *
 * Returns `null` on any failure (network error, non-2xx, GraphQL errors, no
 * profiles) — the caller turns that into a `502 profile-unavailable` response
 * without ever writing a job row or touching R2.
 */
export async function resolveDefaultProfileId(env: Env, pack: string, version: string): Promise<string | null> {
  const query = `
    query ResolveDefaultProfile($id: ID!, $version: String) {
      printer(id: $id, version: $version) {
        versions {
          version
          profiles {
            id
          }
        }
      }
    }
  `;

  let response: Response;
  try {
    response = await fetch(`${env.REGISTRY_URL.replace(/\/$/, "")}/graphql`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ query, variables: { id: pack, version } }),
    });
  } catch {
    return null;
  }
  if (!response.ok) return null;

  let payload: unknown;
  try {
    payload = await response.json();
  } catch {
    return null;
  }

  const data = payload as { data?: { printer?: { versions?: unknown } }; errors?: unknown[] };
  if (Array.isArray(data.errors) && data.errors.length > 0) return null;

  const versions = data.data?.printer?.versions;
  if (!Array.isArray(versions) || versions.length === 0) return null;

  const versionEntry =
    (versions as Array<{ version?: unknown; profiles?: unknown }>).find((entry) => entry?.version === version) ??
    versions[0];
  const profiles = (versionEntry as { profiles?: unknown } | undefined)?.profiles;
  if (!Array.isArray(profiles) || profiles.length === 0) return null;

  const id = (profiles[0] as { id?: unknown } | undefined)?.id;
  return typeof id === "string" && id.length > 0 ? id : null;
}

/** `POST /v1/jobs/verify?pack=<id>&version=<ver>[&profile=<profileId>]` (Bearer,
 * raw g-code body). `accountId` is already-authenticated (see index.ts's
 * `requireAuth` call before this is reached). */
export async function handlePostVerifyJob(request: Request, env: Env, accountId: string): Promise<Response> {
  const url = new URL(request.url);
  const pack = url.searchParams.get("pack");
  const version = url.searchParams.get("version");
  let profileId = url.searchParams.get("profile");

  if (!pack || !version) {
    return jsonResponse({ error: "invalid_request", detail: "pack and version query params are required" }, 400);
  }

  // Content-Length cap, enforced BEFORE any R2 write or D1 row — per the Global
  // Constraints' "too-large" stage and the R3 brief's "rejected pre-R2-write".
  const contentLengthHeader = request.headers.get("content-length");
  const contentLength = contentLengthHeader === null ? Number.NaN : Number.parseInt(contentLengthHeader, 10);
  if (!Number.isFinite(contentLength)) {
    return jsonResponse({ error: "length_required" }, 411);
  }
  if (contentLength > MAX_UPLOAD_BYTES) {
    return jsonResponse({ error: "too-large", max_bytes: MAX_UPLOAD_BYTES }, 413);
  }

  const quota = parseQuota(env.QUOTA_JOBS_PER_MONTH, 20);
  const countRow = await env.DB.prepare(
    "SELECT COUNT(*) AS count FROM jobs WHERE account_id = ? AND created_at >= strftime('%Y-%m-01 00:00:00', 'now')",
  )
    .bind(accountId)
    .first<{ count: number }>();
  if ((countRow?.count ?? 0) >= quota) {
    return jsonResponse({ error: "quota_exceeded", quota }, 403);
  }

  if (!profileId) {
    profileId = await resolveDefaultProfileId(env, pack, version);
    if (!profileId) {
      return jsonResponse({ error: "profile_unavailable", stage: "profile-unavailable" }, 502);
    }
  }

  if (!request.body) {
    return jsonResponse({ error: "invalid_request", detail: "request body is required" }, 400);
  }

  const jobId = crypto.randomUUID();
  const inputKey = `uploads/${jobId}`;
  await env.STORAGE.put(inputKey, request.body, {
    httpMetadata: { contentType: "text/plain" },
  });

  await env.DB.prepare(
    "INSERT INTO jobs (id, account_id, status, pack_id, pack_version, profile_id, input_r2) VALUES (?, ?, 'queued', ?, ?, ?, ?)",
  )
    .bind(jobId, accountId, pack, version, profileId, inputKey)
    .run();

  await env.VERIFY_JOBS.send({ id: jobId });

  return jsonResponse({ id: jobId, status_url: `/v1/jobs/${jobId}` }, 202);
}

interface JobRow {
  id: string;
  status: string;
  pack_id: string | null;
  created_at: string;
  finished_at: string | null;
  report_r2: string | null;
  error: string | null;
  stage: string | null;
}

/** `GET /v1/jobs/{id}` (Bearer, owner-only — another account's job id 404s exactly
 * like a nonexistent one, matching the `/v1/keys/{id}` DELETE convention in
 * src/index.ts). Report is inlined (parsed JSON, not a raw string) once `done`. */
export async function handleGetJob(env: Env, accountId: string, jobId: string): Promise<Response> {
  const row = await env.DB.prepare(
    "SELECT id, status, pack_id, created_at, finished_at, report_r2, error, stage FROM jobs WHERE id = ? AND account_id = ?",
  )
    .bind(jobId, accountId)
    .first<JobRow>();
  if (!row) return jsonResponse({ error: "not_found" }, 404);

  const body: Record<string, unknown> = {
    id: row.id,
    status: row.status,
    pack_id: row.pack_id,
    created_at: row.created_at,
    finished_at: row.finished_at,
  };

  if (row.status === "done" && row.report_r2) {
    const object = await env.STORAGE.get(row.report_r2);
    if (object) {
      try {
        body.report = JSON.parse(await object.text());
      } catch {
        // Report object exists but isn't valid JSON -- shouldn't happen (we write
        // it ourselves in the consumer below), but don't let a corrupt object 500
        // the status read.
      }
    }
  }
  if (row.error !== null) body.error = row.error;
  if (row.stage !== null) body.stage = row.stage;

  return jsonResponse(body);
}

// --- Queue consumer ---------------------------------------------------------

export interface QueueJobMessage {
  id: string;
}

/** Thrown only when `containerFetch` itself throws (a container-start/network-level
 * failure reaching the runner) -- distinguishes "retry this" from "the runner
 * answered with a normal {error, stage} HTTP response", which is a terminal state,
 * not a transient one. */
class ContainerStartError extends Error {}

/** Injectable dependencies for the queue consumer, so unit tests never need a real
 * container instance (Docker) -- see container.ts's module doc comment and the R3
 * brief's "inject a containerFetch(stub) seam". Both default to the real
 * implementations in container.ts. */
export interface QueueDeps {
  getContainerStub: (env: Env, jobId: string) => ContainerStubLike;
  containerFetch: (stub: ContainerStubLike, url: string, init: RequestInit) => Promise<Response>;
}

const defaultDeps: QueueDeps = {
  getContainerStub: defaultGetContainerStub,
  containerFetch: defaultContainerFetch,
};

/** Global Constraints: "retry only on container-start failures, max 2." Total
 * attempts allowed before giving up and persisting a terminal error, i.e. exactly
 * one retry: attempt 1 fails -> retry; attempt 2 fails -> error + ack. */
const MAX_CONTAINER_ATTEMPTS = 2;

function buildVerifyUrl(registryUrl: string, pack: string, version: string, profileId: string): string {
  const params = new URLSearchParams({ pack, version, profile: profileId, registry: registryUrl });
  // Host/scheme are irrelevant for a container-DO fetch (routed straight to the
  // container's `defaultPort`, never touching real DNS) -- only path+query matter.
  return `http://container/verify?${params.toString()}`;
}

/** Maps the verify-runner's HTTP status back to the Global Constraints' three
 * stages. Deliberately keyed off status code (the runner's *documented HTTP
 * contract*), not the response body's own `stage` field, so a malformed or missing
 * body from a buggy/compromised runner can't spoof an unexpected stage. */
function stageForStatus(status: number): "profile-unavailable" | "input-invalid" | "engine-error" {
  if (status === 422) return "input-invalid";
  if (status === 502) return "profile-unavailable";
  return "engine-error";
}

async function readRunnerErrorMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body?.error === "string" && body.error.length > 0) return body.error;
  } catch {
    // fall through to the generic message below
  }
  return `verify-runner returned HTTP ${response.status}`;
}

async function markJobError(env: Env, jobId: string, stage: string, message: string): Promise<void> {
  await env.DB.prepare("UPDATE jobs SET status = 'error', error = ?, stage = ?, finished_at = datetime('now') WHERE id = ?")
    .bind(message, stage, jobId)
    .run();
}

interface QueueJobRow {
  pack_id: string;
  pack_version: string;
  profile_id: string;
  input_r2: string;
}

/** Processes exactly one job message end to end. Throws `ContainerStartError` (and
 * only that) when the container call itself fails to complete -- every other
 * outcome (missing row, missing R2 object, a runner HTTP error response, a
 * successful report) is handled to completion here and never throws. */
async function processJob(
  jobId: string,
  env: Env,
  getStub: QueueDeps["getContainerStub"],
  callContainer: QueueDeps["containerFetch"],
): Promise<void> {
  const row = await env.DB.prepare("SELECT pack_id, pack_version, profile_id, input_r2 FROM jobs WHERE id = ?")
    .bind(jobId)
    .first<QueueJobRow>();
  if (!row) {
    // The job row is gone (shouldn't normally happen -- nothing else deletes jobs
    // rows). Nothing to process; treat the message as handled.
    return;
  }

  const object = await env.STORAGE.get(row.input_r2);
  if (!object) {
    await markJobError(env, jobId, "engine-error", `input object ${row.input_r2} not found in R2`);
    return;
  }

  const verifyUrl = buildVerifyUrl(env.REGISTRY_URL, row.pack_id, row.pack_version, row.profile_id);
  const stub = getStub(env, jobId);

  let response: Response;
  try {
    // Direct-stream transfer path (the Worker forwards the R2 object's body
    // straight through to the container's fetch call) -- see
    // itest/jobs-local.sh and its report for the empirical finding on whether
    // this holds up for large (10-50 MB) bodies, per the Global Constraints'
    // known-risk note on Worker-container transfers.
    response = await callContainer(stub, verifyUrl, {
      method: "POST",
      body: object.body,
      headers: { "content-type": "text/plain" },
    });
  } catch (error) {
    throw new ContainerStartError(error instanceof Error ? error.message : String(error));
  }

  if (response.status === 200) {
    const reportBytes = await response.arrayBuffer();
    const reportKey = `reports/${jobId}.json`;
    await env.STORAGE.put(reportKey, reportBytes, { httpMetadata: { contentType: "application/json" } });
    await env.DB.prepare("UPDATE jobs SET status = 'done', report_r2 = ?, finished_at = datetime('now') WHERE id = ?")
      .bind(reportKey, jobId)
      .run();
    return;
  }

  const stage = stageForStatus(response.status);
  const message = await readRunnerErrorMessage(response);
  await markJobError(env, jobId, stage, message);
}

/** Queue consumer for `verify-jobs`: `message.ack()` always follows a terminal
 * state (success or a persisted error), per the R3 brief; the ONLY retry path is a
 * container-start failure (network error reaching the container), bounded to
 * `MAX_CONTAINER_ATTEMPTS` total attempts. A batch never throws out of this
 * function -- every message is individually ack'd or retried, so one bad message
 * can never take the rest of the batch down with it (see the Cloudflare Queues
 * "uncaught error retries the whole batch" gotcha). */
export async function handleQueueBatch(batch: MessageBatch<QueueJobMessage>, env: Env, deps: Partial<QueueDeps> = {}): Promise<void> {
  const getStub = deps.getContainerStub ?? defaultDeps.getContainerStub;
  const callContainer = deps.containerFetch ?? defaultDeps.containerFetch;

  for (const message of batch.messages) {
    try {
      await processJob(message.body.id, env, getStub, callContainer);
      message.ack();
    } catch (error) {
      if (error instanceof ContainerStartError && message.attempts < MAX_CONTAINER_ATTEMPTS) {
        message.retry();
        continue;
      }
      const detail = error instanceof Error ? error.message : String(error);
      try {
        await markJobError(env, message.body.id, "engine-error", detail);
      } catch {
        // Best-effort: even if persisting the error itself fails, still ack --
        // an infinite redelivery loop is worse than one job stuck "queued".
      }
      message.ack();
    }
  }
}
