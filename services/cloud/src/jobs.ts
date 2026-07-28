// Async verify jobs: POST /v1/jobs/verify (upload -> R2 -> D1 `queued` row -> queue
// send), the queue consumer (job -> container -> report -> R2 + D1 `done`/`error`),
// and GET /v1/jobs/{id} (owner-only status/report read).
//
// Interfaces and failure taxonomy per the R3 task brief and its Global Constraints;
// see also containers/verify-runner's README for the runner's own `{error, stage}`
// contract this consumer maps onto. One stage, `queue-send-failed`, is
// Worker-only (never returned by the runner) — see `markJobError`'s call site
// in `handlePostVerifyJob` (R3 review Fix 2b).

import { type ContainerStubLike, containerFetch as defaultContainerFetch, getContainerStub as defaultGetContainerStub } from "./container";
import { countJobsThisMonth, parseQuota, quotaExceededResponse } from "./usage";

/** Global Constraints: "Upload cap: 100 MB (container has 6 GiB; the Worker
 * enforces Content-Length)." Not a var (unlike the QUOTA_* knobs below) — the brief
 * states it as a fixed product invariant, not an operator-tunable setting. */
const MAX_UPLOAD_BYTES = 100 * 1024 * 1024;

/** Every `jobs.stage` value this Worker can ever write. The runner's own
 * `{error, stage}` contract (containers/verify-runner) only ever produces the
 * first three (mapped in `stageForStatus` below); `queue-send-failed` is
 * Worker-only, written directly by `handlePostVerifyJob` when the job never
 * even reaches the queue (see Fix 2b). Centralized here (rather than as an
 * inline literal union on `stageForStatus`'s return type) so every writer of
 * `jobs.stage` is typed against the SAME set. */
export type JobStage = "profile-unavailable" | "input-invalid" | "engine-error" | "queue-send-failed";

function jsonResponse(value: unknown, status = 200, extraHeaders?: HeadersInit): Response {
  const headers = new Headers(extraHeaders);
  headers.set("content-type", "application/json; charset=utf-8");
  return Response.json(value, { status, headers });
}

/** Outcome of `resolveDefaultProfileId` — deliberately NOT just `string | null`
 * (as an earlier revision had it) so the caller can distinguish two different
 * failure shapes with different HTTP responses (R3 review Fix 3):
 * `version_not_found` (the registry answered fine, but has no entry for the
 * EXACT requested `version` — a client input error, `404`) vs. `unavailable`
 * (a network/parse/GraphQL-level failure, or a matching version with no
 * profiles at all — the registry itself is the problem, `502`). */
export type ResolveProfileOutcome =
  | { ok: true; profileId: string }
  | { ok: false; reason: "version_not_found" }
  | { ok: false; reason: "unavailable" };

/**
 * MVP default-profile rule (documented per the R3 task brief's explicit request):
 * when `POST /v1/jobs/verify` omits `profile=`, resolve it via the registry's
 * GraphQL API (docs/19-printer-registry-api.md; query shape mirrors
 * `resolve_profile` in crates/cli/src/printer_registry.rs) and pick the FIRST
 * profile listed for the pack version — i.e. "first listed profile of the pack
 * manifest," the deterministic rule the task brief explicitly sanctions as
 * acceptable for MVP. No material/nozzle filtering is applied; a pack that
 * needs a specific profile for correct results should pass `profile=`
 * explicitly rather than rely on this default.
 *
 * **R3 review Fix 3: exact `version` match ONLY.** An earlier revision fell
 * back to the registry's first-listed version entry when the requested
 * `version` had no exact match — silently resolving a profile for a DIFFERENT
 * pack version than the caller asked for. That fallback is removed: no exact
 * match now fails fast (`{ ok: false, reason: "version_not_found" }`), which
 * `handlePostVerifyJob` turns into a `404 "pack version not found"` BEFORE any
 * R2 write or D1 row — see that function's ordering comment.
 */
export async function resolveDefaultProfileId(env: Env, pack: string, version: string): Promise<ResolveProfileOutcome> {
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
    return { ok: false, reason: "unavailable" };
  }
  if (!response.ok) return { ok: false, reason: "unavailable" };

  let payload: unknown;
  try {
    payload = await response.json();
  } catch {
    return { ok: false, reason: "unavailable" };
  }

  const data = payload as { data?: { printer?: { versions?: unknown } }; errors?: unknown[] };
  if (Array.isArray(data.errors) && data.errors.length > 0) return { ok: false, reason: "unavailable" };

  const versions = data.data?.printer?.versions;
  if (!Array.isArray(versions) || versions.length === 0) return { ok: false, reason: "version_not_found" };

  // Exact match ONLY -- no `?? versions[0]` fallback (see the Fix 3 doc above).
  const versionEntry = (versions as Array<{ version?: unknown; profiles?: unknown }>).find(
    (entry) => entry?.version === version,
  );
  if (!versionEntry) return { ok: false, reason: "version_not_found" };

  const profiles = versionEntry.profiles;
  if (!Array.isArray(profiles) || profiles.length === 0) return { ok: false, reason: "unavailable" };

  const id = (profiles[0] as { id?: unknown } | undefined)?.id;
  if (typeof id !== "string" || id.length === 0) return { ok: false, reason: "unavailable" };

  return { ok: true, profileId: id };
}

/** `POST /v1/jobs/verify?pack=<id>&version=<ver>[&profile=<profileId>]` (Bearer,
 * raw g-code body). `accountId` is already-authenticated (see index.ts's
 * `requireAuth` call before this is reached).
 *
 * **Ordering invariant (R3 review Fix 3 — verified, kept, documented here):**
 * Content-Length cap -> quota check -> default-profile RESOLUTION -> R2 write
 * -> D1 insert -> queue send. Every check runs, in that order, before the
 * first state-changing operation (the R2 write) — a rejection at any earlier
 * stage (400/411/413/403/404/502) therefore NEVER leaves behind an orphaned R2
 * object or D1 row. Keep new checks above the R2 write unless a check
 * genuinely needs the uploaded bytes to decide (none currently do). */
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

  // R4: canonical quota source is the `jobs` table (see src/usage.ts's module
  // doc comment for the full jobs-vs-usage_events reconciliation), counted
  // over the current UTC calendar month via `countJobsThisMonth`. A
  // quota-exceeded submission now 429s with the shared `{error,
  // usage_url}` + `Retry-After` shape (`quotaExceededResponse`) rather than
  // the older ad hoc `403 {error, quota}` -- see that function's doc comment.
  const quota = parseQuota(env.QUOTA_JOBS_PER_MONTH, 20);
  const jobCount = await countJobsThisMonth(env, accountId);
  if (jobCount >= quota) {
    return quotaExceededResponse();
  }

  if (!profileId) {
    const resolved = await resolveDefaultProfileId(env, pack, version);
    if (!resolved.ok) {
      // Fix 3: a `version_not_found` outcome means the registry itself
      // answered fine but has no exact match for the requested `version` — a
      // client input error (404), fundamentally different from `unavailable`
      // (a registry-side failure, 502). Both happen strictly before the R2
      // write below, so neither ever leaves an orphaned upload or job row.
      if (resolved.reason === "version_not_found") {
        return jsonResponse({ error: "pack version not found" }, 404);
      }
      return jsonResponse({ error: "profile_unavailable", stage: "profile-unavailable" }, 502);
    }
    profileId = resolved.profileId;
  }

  if (!request.body) {
    return jsonResponse({ error: "invalid_request", detail: "request body is required" }, 400);
  }

  const jobId = crypto.randomUUID();
  const inputKey = `uploads/${jobId}`;
  await env.STORAGE.put(inputKey, request.body, {
    httpMetadata: { contentType: "text/plain" },
  });

  try {
    await env.DB.prepare(
      "INSERT INTO jobs (id, account_id, status, pack_id, pack_version, profile_id, input_r2) VALUES (?, ?, 'queued', ?, ?, ?, ?)",
    )
      .bind(jobId, accountId, pack, version, profileId, inputKey)
      .run();
  } catch (error) {
    // Fix 2a: the R2 object was already written above -- if the D1 insert
    // that's supposed to record it then fails, clean up the now-orphaned
    // upload (best-effort: a failed cleanup is logged, not thrown, so the
    // REAL error -- the D1 failure -- is what surfaces) and rethrow, so
    // index.ts's generic top-level try/catch turns this into a plain 500
    // rather than a false 202.
    try {
      await env.STORAGE.delete(inputKey);
    } catch (cleanupError) {
      console.error("dry-cloud: failed to clean up orphaned R2 upload after a D1 insert failure", inputKey, cleanupError);
    }
    throw error;
  }

  try {
    await env.VERIFY_JOBS.send({ id: jobId });
  } catch (error) {
    // Fix 2b: the D1 row already exists (`queued`) but the message never
    // reached the queue, so the job would otherwise sit `queued` forever with
    // no consumer ever picking it up. Mark it terminal (`error` +
    // `queue-send-failed`) so `GET /v1/jobs/{id}` reports the real state, and
    // respond 500 (NOT 202 — a 202 promises a status_url that will never
    // progress).
    const message = "job could not be enqueued";
    await markJobError(env, jobId, "queue-send-failed", message);
    return jsonResponse({ error: message }, 500);
  }

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
function stageForStatus(status: number): Exclude<JobStage, "queue-send-failed"> {
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

async function markJobError(env: Env, jobId: string, stage: JobStage, message: string): Promise<void> {
  await env.DB.prepare("UPDATE jobs SET status = 'error', error = ?, stage = ?, finished_at = datetime('now') WHERE id = ?")
    .bind(message, stage, jobId)
    .run();
}

interface QueueJobRow {
  status: string;
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
  const row = await env.DB.prepare("SELECT status, pack_id, pack_version, profile_id, input_r2 FROM jobs WHERE id = ?")
    .bind(jobId)
    .first<QueueJobRow>();
  if (!row) {
    // The job row is gone (shouldn't normally happen -- nothing else deletes jobs
    // rows). Nothing to process; treat the message as handled.
    return;
  }

  // Fix 1 (redelivery idempotency): Cloudflare Queues' at-least-once delivery
  // means a message can be redelivered for a job that ALREADY reached a
  // terminal state (e.g. `ack()` succeeded on the consumer side but the
  // platform re-delivers anyway, or a retry lands after a prior attempt's
  // error was already persisted). Re-running the container call for an
  // already-`done`/`error` job would be wasted work at best (spins up a fresh
  // container for nothing) and DATA LOSS at worst (a redelivered success could
  // overwrite a `done` job's `report_r2` with a different report, or a
  // redelivered failure could clobber a `done` job's status with `error`).
  // Detect it before touching the container or R2/D1 state at all, log it,
  // and treat the message as handled (ack, no retry, report untouched).
  if (row.status === "done" || row.status === "error") {
    console.warn("dry-cloud: skipping redelivered terminal job", jobId);
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
