// Router for the Dry Cloud auth worker: device flow, /activate, API keys,
// and /v1/me. Plain fetch handler with a route table — no framework needed
// at this size (see the R1 task brief).

import { handleActivateGet, handleActivateSubmit } from "./activate";
import { handleDeviceStart, handleToken, requireAuth } from "./auth";
import { VerifyContainer } from "./container";
import { handleGetJob, handlePostVerifyJob, handleQueueBatch, type QueueJobMessage } from "./jobs";
import { generateApiKey, sha256Hex } from "./tokens";

// Re-exported so wrangler can find the Durable Object class this Worker declares
// in wrangler.jsonc's `durable_objects`/`containers` config (the class must be
// exported from the `main` module) -- see src/container.ts.
export { VerifyContainer };

/** Applied to every response this Worker returns. */
export const SECURITY_HEADERS: Readonly<Record<string, string>> = Object.freeze({
  "x-content-type-options": "nosniff",
  "referrer-policy": "no-referrer",
  "x-frame-options": "DENY",
  "cross-origin-opener-policy": "same-origin",
  "cache-control": "no-store",
});

function withSecurityHeaders(response: Response): Response {
  const headers = new Headers(response.headers);
  for (const [name, value] of Object.entries(SECURITY_HEADERS)) {
    if (!headers.has(name)) headers.set(name, value);
  }
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

function jsonResponse(value: unknown, status = 200, extraHeaders?: HeadersInit): Response {
  const headers = new Headers(extraHeaders);
  headers.set("content-type", "application/json; charset=utf-8");
  return Response.json(value, { status, headers });
}

function parseQuota(raw: string | undefined): number {
  const parsed = Number.parseInt(raw ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 1;
}

async function handleCreateKey(request: Request, env: Env, accountId: string): Promise<Response> {
  const quota = parseQuota(env.QUOTA_KEYS);
  const countRow = await env.DB.prepare(
    "SELECT COUNT(*) AS count FROM tokens WHERE account_id = ? AND kind = 'key' AND revoked = 0",
  )
    .bind(accountId)
    .first<{ count: number }>();
  if ((countRow?.count ?? 0) >= quota) {
    return jsonResponse({ error: "quota_exceeded", quota }, 403);
  }

  let label: string | null = null;
  try {
    const body = (await request.json()) as { label?: unknown };
    if (typeof body?.label === "string") label = body.label;
  } catch {
    // No (or invalid) JSON body — the key is still created, just unlabeled.
  }

  const key = generateApiKey();
  const hash = await sha256Hex(key);
  await env.DB.prepare("INSERT INTO tokens (hash, account_id, kind, label) VALUES (?, ?, 'key', ?)")
    .bind(hash, accountId, label)
    .run();

  return jsonResponse({ id: hash, key }, 201);
}

async function handleListKeys(env: Env, accountId: string): Promise<Response> {
  const { results } = await env.DB.prepare(
    "SELECT hash AS id, label, created_at FROM tokens WHERE account_id = ? AND kind = 'key' AND revoked = 0 ORDER BY created_at",
  )
    .bind(accountId)
    .all<{ id: string; label: string | null; created_at: string }>();
  return jsonResponse({ keys: results });
}

async function handleDeleteKey(env: Env, accountId: string, id: string): Promise<Response> {
  const result = await env.DB.prepare(
    "UPDATE tokens SET revoked = 1 WHERE hash = ? AND account_id = ? AND kind = 'key'",
  )
    .bind(id, accountId)
    .run();
  if (!result.meta.changes) {
    return jsonResponse({ error: "not_found" }, 404);
  }
  return new Response(null, { status: 204 });
}

async function handleMe(env: Env, accountId: string): Promise<Response> {
  const account = await env.DB.prepare("SELECT id, email, created_at FROM accounts WHERE id = ?")
    .bind(accountId)
    .first<{ id: string; email: string; created_at: string }>();
  if (!account) return jsonResponse({ error: "not_found" }, 404);
  return jsonResponse({ account_id: account.id, email: account.email, created_at: account.created_at });
}

async function route(request: Request, env: Env): Promise<Response> {
  const url = new URL(request.url);

  if (url.pathname === "/v1/auth/device") {
    return handleDeviceStart(request, env, url.origin);
  }

  if (url.pathname === "/v1/auth/token") {
    return handleToken(request, env);
  }

  if (url.pathname === "/activate") {
    if (request.method === "GET") return handleActivateGet(request, env);
    if (request.method === "POST") return handleActivateSubmit(request, env);
    return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "GET, POST" });
  }

  if (url.pathname === "/v1/keys") {
    if (request.method === "POST") {
      const auth = await requireAuth(request, env);
      if (!auth.ok) return auth.response;
      return handleCreateKey(request, env, auth.accountId);
    }
    if (request.method === "GET") {
      const auth = await requireAuth(request, env);
      if (!auth.ok) return auth.response;
      return handleListKeys(env, auth.accountId);
    }
    return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "GET, POST" });
  }

  const keyMatch = /^\/v1\/keys\/([^/]+)$/.exec(url.pathname);
  if (keyMatch) {
    if (request.method !== "DELETE") {
      return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "DELETE" });
    }
    const auth = await requireAuth(request, env);
    if (!auth.ok) return auth.response;
    return handleDeleteKey(env, auth.accountId, decodeURIComponent(keyMatch[1]));
  }

  if (url.pathname === "/v1/jobs/verify") {
    if (request.method !== "POST") {
      return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "POST" });
    }
    const auth = await requireAuth(request, env);
    if (!auth.ok) return auth.response;
    return handlePostVerifyJob(request, env, auth.accountId);
  }

  // Checked AFTER the exact "/v1/jobs/verify" match above -- otherwise this
  // pattern would swallow it (`verify` looks like a job id to this regex).
  const jobMatch = /^\/v1\/jobs\/([^/]+)$/.exec(url.pathname);
  if (jobMatch) {
    if (request.method !== "GET") {
      return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "GET" });
    }
    const auth = await requireAuth(request, env);
    if (!auth.ok) return auth.response;
    return handleGetJob(env, auth.accountId, decodeURIComponent(jobMatch[1]));
  }

  if (url.pathname === "/v1/me") {
    if (request.method !== "GET") {
      return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "GET" });
    }
    const auth = await requireAuth(request, env);
    if (!auth.ok) return auth.response;
    return handleMe(env, auth.accountId);
  }

  return jsonResponse({ error: "not_found" }, 404);
}

export default {
  async fetch(request, env): Promise<Response> {
    try {
      return withSecurityHeaders(await route(request, env));
    } catch (error) {
      console.error("dry-cloud: unhandled request error", error);
      return withSecurityHeaders(jsonResponse({ error: "internal_error" }, 500));
    }
  },
  async queue(batch, env): Promise<void> {
    await handleQueueBatch(batch, env);
  },
} satisfies ExportedHandler<Env, QueueJobMessage>;
