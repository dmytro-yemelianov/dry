import { env, exports } from "cloudflare:workers";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { approveDevice, resolveUserCode } from "../src/auth";

type MutableEnv = Omit<Env, "TURNSTILE_DEV_BYPASS"> & { TURNSTILE_DEV_BYPASS: string };
const testEnv = env as unknown as MutableEnv;
const ORIGIN = "http://example.com";

function url(path: string): string {
  return new URL(path, ORIGIN).toString();
}

async function fetchWorker(path: string, init?: RequestInit): Promise<Response> {
  return exports.default.fetch(url(path), init);
}

interface DeviceStartBody {
  device_code: string;
  user_code: string;
  verification_uri: string;
  verification_uri_complete: string;
  expires_in: number;
  interval: number;
}

async function startDeviceFlow(extraHeaders: Record<string, string> = {}): Promise<DeviceStartBody> {
  const response = await fetchWorker("/v1/auth/device", { method: "POST", headers: extraHeaders });
  expect(response.status).toBe(200);
  return response.json();
}

function formBody(fields: Record<string, string>): string {
  return new URLSearchParams(fields).toString();
}

async function activate(
  userCode: string,
  email: string,
  extra: Record<string, string> = {},
  extraHeaders: Record<string, string> = {},
): Promise<Response> {
  return fetchWorker("/activate", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded", ...extraHeaders },
    body: formBody({ user_code: userCode, email, ...extra }),
  });
}

async function pollToken(deviceCode: string): Promise<Response> {
  return fetchWorker("/v1/auth/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: formBody({
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
      device_code: deviceCode,
    }),
  });
}

let accountCounter = 0;
function freshEmail(): string {
  accountCounter += 1;
  return `user-${accountCounter}-${Date.now()}@example.com`;
}

/** Full device-flow round trip: returns a fresh Bearer access token. */
async function grantAccessToken(email = freshEmail()): Promise<string> {
  const start = await startDeviceFlow();
  const approveResponse = await activate(start.user_code, email);
  expect(approveResponse.status).toBe(200);
  const tokenResponse = await pollToken(start.device_code);
  expect(tokenResponse.status).toBe(200);
  const body = (await tokenResponse.json()) as { access_token: string; token_type: string };
  expect(body.token_type).toBe("Bearer");
  expect(body.access_token).toMatch(/^dry_at_[A-Za-z0-9_-]{43}$/);
  return body.access_token;
}

describe("security headers", () => {
  it("applies the security header set to every response, including 404s", async () => {
    const response = await fetchWorker("/does-not-exist");
    expect(response.status).toBe(404);
    expect(response.headers.get("x-content-type-options")).toBe("nosniff");
    expect(response.headers.get("x-frame-options")).toBe("DENY");
    expect(response.headers.get("referrer-policy")).toBe("no-referrer");
    expect(response.headers.get("cache-control")).toBe("no-store");
  });
});

describe("device flow state machine", () => {
  it("issues a device/user code pair with the RFC 8628 shape", async () => {
    const start = await startDeviceFlow();
    expect(start.user_code).toMatch(/^[BCDFGHJKLMNPQRSTVWXZ23456789]{4}-[BCDFGHJKLMNPQRSTVWXZ23456789]{4}$/);
    expect(start.device_code).toMatch(/^[A-Za-z0-9_-]{43}$/);
    expect(start.expires_in).toBe(600);
    expect(start.interval).toBe(5);
    expect(start.verification_uri).toBe(`${ORIGIN}/activate`);
    expect(start.verification_uri_complete).toBe(`${ORIGIN}/activate?user_code=${encodeURIComponent(start.user_code)}`);
  });

  it("is authorization_pending before activation, then grants a single-use token that unlocks /v1/me", async () => {
    const start = await startDeviceFlow();

    const pendingResponse = await pollToken(start.device_code);
    expect(pendingResponse.status).toBe(400);
    expect(await pendingResponse.json()).toEqual({ error: "authorization_pending" });

    const email = freshEmail();
    const approveResponse = await activate(start.user_code, email);
    expect(approveResponse.status).toBe(200);

    const grantResponse = await pollToken(start.device_code);
    expect(grantResponse.status).toBe(200);
    const granted = (await grantResponse.json()) as { access_token: string; token_type: string };
    expect(granted.token_type).toBe("Bearer");
    expect(granted.access_token).toMatch(/^dry_at_[A-Za-z0-9_-]{43}$/);

    const meResponse = await fetchWorker("/v1/me", {
      headers: { authorization: `Bearer ${granted.access_token}` },
    });
    expect(meResponse.status).toBe(200);
    const me = (await meResponse.json()) as { account_id: string; email: string; created_at: string };
    expect(me.email).toBe(email);
    expect(me.account_id).toBeTruthy();
    expect(me.created_at).toBeTruthy();

    // Single-use: the device_code cannot be redeemed a second time.
    const secondGrant = await pollToken(start.device_code);
    expect(secondGrant.status).toBe(400);
    expect(await secondGrant.json()).toEqual({ error: "expired_token" });
  });

  it("returns slow_down when polled again before the interval elapses, and recovers after it does", async () => {
    const start = await startDeviceFlow();

    const first = await pollToken(start.device_code);
    expect(await first.json()).toEqual({ error: "authorization_pending" });

    const second = await pollToken(start.device_code);
    expect(second.status).toBe(400);
    expect(await second.json()).toEqual({ error: "slow_down" });

    vi.useFakeTimers();
    try {
      vi.advanceTimersByTime(5_001);
      const third = await pollToken(start.device_code);
      expect(await third.json()).toEqual({ error: "authorization_pending" });
    } finally {
      vi.useRealTimers();
    }
  });

  it("returns expired_token once the device code's TTL has elapsed", async () => {
    const start = await startDeviceFlow();

    vi.useFakeTimers();
    try {
      vi.advanceTimersByTime(600_001);
      const response = await pollToken(start.device_code);
      expect(response.status).toBe(400);
      expect(await response.json()).toEqual({ error: "expired_token" });
    } finally {
      vi.useRealTimers();
    }
  });

  it("rejects an unknown grant_type", async () => {
    const response = await fetchWorker("/v1/auth/token", {
      method: "POST",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: formBody({ grant_type: "authorization_code", device_code: "whatever" }),
    });
    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ error: "unsupported_grant_type" });
  });

  it("returns expired_token for a device_code that was never issued", async () => {
    const response = await pollToken("not-a-real-device-code");
    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ error: "expired_token" });
  });
});

describe("API keys", () => {
  it("creates a key (shown once), lists it without the secret, and enforces QUOTA_KEYS", async () => {
    const token = await grantAccessToken();
    const authHeaders = { authorization: `Bearer ${token}` };

    const createResponse = await fetchWorker("/v1/keys", {
      method: "POST",
      headers: { ...authHeaders, "content-type": "application/json" },
      body: JSON.stringify({ label: "ci" }),
    });
    expect(createResponse.status).toBe(201);
    const created = (await createResponse.json()) as { id: string; key: string };
    expect(created.key).toMatch(/^dry_key_[A-Za-z0-9_-]{43}$/);
    expect(created.id).toBeTruthy();

    const listResponse = await fetchWorker("/v1/keys", { headers: authHeaders });
    expect(listResponse.status).toBe(200);
    const listed = (await listResponse.json()) as {
      keys: Array<{ id: string; label: string | null; created_at: string; key?: string }>;
    };
    expect(listed.keys).toHaveLength(1);
    expect(listed.keys[0].id).toBe(created.id);
    expect(listed.keys[0].label).toBe("ci");
    expect(listed.keys[0].created_at).toBeTruthy();
    expect(listed.keys[0].key).toBeUndefined();

    // QUOTA_KEYS=1 (wrangler.jsonc dev var) — a second key is refused.
    const secondCreate = await fetchWorker("/v1/keys", {
      method: "POST",
      headers: authHeaders,
    });
    expect(secondCreate.status).toBe(403);
    expect(await secondCreate.json()).toMatchObject({ error: "quota_exceeded" });

    // Deleting the key frees the quota back up.
    const deleteResponse = await fetchWorker(`/v1/keys/${created.id}`, {
      method: "DELETE",
      headers: authHeaders,
    });
    expect(deleteResponse.status).toBe(204);

    const afterDeleteList = await fetchWorker("/v1/keys", { headers: authHeaders });
    expect((await afterDeleteList.json()) as { keys: unknown[] }).toEqual({ keys: [] });

    const thirdCreate = await fetchWorker("/v1/keys", { method: "POST", headers: authHeaders });
    expect(thirdCreate.status).toBe(201);
  });

  it("404s deleting a key that doesn't belong to the caller", async () => {
    const token = await grantAccessToken();
    const response = await fetchWorker("/v1/keys/not-a-real-hash", {
      method: "DELETE",
      headers: { authorization: `Bearer ${token}` },
    });
    expect(response.status).toBe(404);
  });
});

describe("auth middleware", () => {
  it("401s with no Authorization header", async () => {
    const response = await fetchWorker("/v1/me");
    expect(response.status).toBe(401);
  });

  it("401s with a garbage bearer token", async () => {
    const response = await fetchWorker("/v1/me", {
      headers: { authorization: "Bearer not-a-real-token" },
    });
    expect(response.status).toBe(401);
  });

  it("401s a revoked API key used as a bearer token", async () => {
    const accessToken = await grantAccessToken();
    const createResponse = await fetchWorker("/v1/keys", {
      method: "POST",
      headers: { authorization: `Bearer ${accessToken}` },
    });
    const { id, key } = (await createResponse.json()) as { id: string; key: string };

    const beforeRevoke = await fetchWorker("/v1/me", {
      headers: { authorization: `Bearer ${key}` },
    });
    expect(beforeRevoke.status).toBe(200);

    await fetchWorker(`/v1/keys/${id}`, {
      method: "DELETE",
      headers: { authorization: `Bearer ${accessToken}` },
    });

    const afterRevoke = await fetchWorker("/v1/me", {
      headers: { authorization: `Bearer ${key}` },
    });
    expect(afterRevoke.status).toBe(401);
  });
});

describe("Turnstile on /activate", () => {
  const originalBypass = testEnv.TURNSTILE_DEV_BYPASS;

  beforeEach(() => {
    testEnv.TURNSTILE_DEV_BYPASS = "0";
  });

  afterEach(() => {
    testEnv.TURNSTILE_DEV_BYPASS = originalBypass;
    vi.unstubAllGlobals();
  });

  it("403s when siteverify reports failure", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ success: false, "error-codes": ["invalid-input-response"] })),
    );

    const start = await startDeviceFlow();
    const response = await activate(start.user_code, freshEmail(), {
      "cf-turnstile-response": "dummy-token",
    });
    expect(response.status).toBe(403);
  });

  it("403s when no turnstile token is submitted at all", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ success: true })),
    );

    const start = await startDeviceFlow();
    const response = await activate(start.user_code, freshEmail());
    expect(response.status).toBe(403);
  });

  it("approves once siteverify reports success", async () => {
    const siteverify = vi.fn(async (input: RequestInfo | URL) => {
      expect(String(input)).toBe("https://challenges.cloudflare.com/turnstile/v0/siteverify");
      return Response.json({ success: true });
    });
    vi.stubGlobal("fetch", siteverify);

    const start = await startDeviceFlow();
    const response = await activate(start.user_code, freshEmail(), {
      "cf-turnstile-response": "dummy-token",
    });
    expect(response.status).toBe(200);
    expect(siteverify).toHaveBeenCalledTimes(1);

    const granted = await pollToken(start.device_code);
    expect(granted.status).toBe(200);
  });
});

describe("/activate account hygiene", () => {
  it("does not create an account row for an invalid or expired user_code", async () => {
    const email = freshEmail();
    const response = await activate("ZZZZ-ZZZZ", email); // never issued by /v1/auth/device
    expect(response.status).toBe(200); // re-rendered form with an inline error, not a hard failure
    expect(await response.text()).toContain("invalid or has expired");

    const row = await testEnv.DB.prepare("SELECT id FROM accounts WHERE email = ?").bind(email).first();
    expect(row).toBeNull();
  });
});

describe("GET /activate", () => {
  it("renders the form with the user_code prefilled from the query string", async () => {
    const response = await fetchWorker("/activate?user_code=ABCD-EFGH");
    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("text/html");
    const html = await response.text();
    expect(html).toContain("ABCD-EFGH");
  });
});

describe("Turnstile dev-bypass fails closed in production", () => {
  const originalEnvironment = testEnv.ENVIRONMENT;
  const originalBypass = testEnv.TURNSTILE_DEV_BYPASS;

  beforeEach(() => {
    testEnv.ENVIRONMENT = "production";
    testEnv.TURNSTILE_DEV_BYPASS = "1";
  });

  afterEach(() => {
    testEnv.ENVIRONMENT = originalEnvironment;
    testEnv.TURNSTILE_DEV_BYPASS = originalBypass;
  });

  it("500s and does not approve the device when the bypass var is set under ENVIRONMENT=production", async () => {
    // Dedicated IP: the rate limiter's KV counters are shared (not reset)
    // across the whole test file, so this uses its own bucket rather than
    // the "unknown" one other tests in this file also draw from.
    const ip = { "cf-connecting-ip": "203.0.113.20" };
    const start = await startDeviceFlow(ip);
    const email = freshEmail();

    const response = await activate(start.user_code, email, {}, ip);
    expect(response.status).toBe(500);
    expect(await response.json()).toEqual({ error: "misconfigured: dev bypass in production" });

    // Not approved: the device is still pending, not granted.
    const poll = await pollToken(start.device_code);
    expect(poll.status).toBe(400);
    expect(await poll.json()).toEqual({ error: "authorization_pending" });

    // No account row was created for the email either.
    const row = await testEnv.DB.prepare("SELECT id FROM accounts WHERE email = ?").bind(email).first();
    expect(row).toBeNull();
  });

  it("500s GET /activate too, before rendering a form that would hide the misconfiguration", async () => {
    const response = await fetchWorker("/activate");
    expect(response.status).toBe(500);
    expect(await response.json()).toEqual({ error: "misconfigured: dev bypass in production" });
  });
});

describe("device-code grant is atomically single-use", () => {
  it("the D1 grants table gate rejects a second mint even if the KV record is re-approved before the first grant's delete", async () => {
    // Dedicated IP: see the note in the previous describe block.
    const ip = { "cf-connecting-ip": "203.0.113.21" };
    const start = await startDeviceFlow(ip);
    const email = freshEmail();

    const approveResponse = await activate(start.user_code, email, {}, ip);
    expect(approveResponse.status).toBe(200);

    // Capture the approved record before the (only) legitimate grant call
    // deletes it, so we can simulate a concurrent poller that read the KV
    // entry before the winner's KV deletes landed.
    const resolved = await resolveUserCode(env, start.user_code);
    if (resolved === "not_found" || resolved === "expired") {
      throw new Error("expected an approved device code");
    }
    const accountId = resolved.record.accountId;
    if (!accountId) {
      throw new Error("expected the approved record to carry an accountId");
    }

    const firstGrant = await pollToken(start.device_code);
    expect(firstGrant.status).toBe(200);

    // Re-seed the KV record exactly as it was right after approval -- as if
    // a second concurrent request had read "approved" before the first
    // grant's KV deletes ran. KV has no compare-and-swap, so without the D1
    // gate this alone would be enough to mint a second token.
    await approveDevice(env, resolved, accountId);

    const secondGrant = await pollToken(start.device_code);
    expect(secondGrant.status).toBe(400);
    expect(await secondGrant.json()).toEqual({ error: "expired_token" });

    const { results } = await testEnv.DB.prepare(
      "SELECT hash FROM tokens WHERE account_id = ? AND kind = 'at'",
    )
      .bind(accountId)
      .all<{ hash: string }>();
    expect(results).toHaveLength(1);
  });
});

describe("rate limiting on the anonymous endpoints", () => {
  it("429s the 11th POST /v1/auth/device from the same IP within the window", async () => {
    const headers = { "cf-connecting-ip": "203.0.113.10" };
    let last: Response | undefined;
    for (let i = 0; i < 11; i++) {
      last = await fetchWorker("/v1/auth/device", { method: "POST", headers });
    }
    expect(last?.status).toBe(429);
    expect(await last?.json()).toEqual({ error: "rate_limited" });
    expect(last?.headers.get("retry-after")).toBe("600");
  });

  it("429s the 11th POST /activate from the same IP within the window", async () => {
    const ip = "203.0.113.11";
    let last: Response | undefined;
    for (let i = 0; i < 11; i++) {
      last = await activate("ZZZZ-ZZZZ", freshEmail(), {}, { "cf-connecting-ip": ip });
    }
    expect(last?.status).toBe(429);
    expect(await last?.json()).toEqual({ error: "rate_limited" });
    expect(last?.headers.get("retry-after")).toBe("600");
  });
});
