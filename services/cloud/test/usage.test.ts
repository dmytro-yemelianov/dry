// Usage metering + job quotas (Task R4). See src/usage.ts's module doc comment
// for the canonical-source reconciliation this suite locks in: `jobs` (not
// `usage_events`) is the source of truth for QUOTA_JOBS_PER_MONTH enforcement;
// `usage_events` is the source for GET /v1/usage's `bytes` reporting.

import { env, exports } from "cloudflare:workers";
import { describe, expect, it } from "vitest";
import {
  countJobsThisMonth,
  formatSqlDateTime,
  secondsUntilNextUtcMonth,
  sumUsageBytesThisMonth,
  utcMonthStart,
  utcNextMonthStart,
} from "../src/usage";

const ORIGIN = "http://example.com";

function url(path: string): string {
  return new URL(path, ORIGIN).toString();
}

async function fetchWorker(path: string, init?: RequestInit): Promise<Response> {
  return exports.default.fetch(url(path), init);
}

function formBody(fields: Record<string, string>): string {
  return new URLSearchParams(fields).toString();
}

let accountCounter = 0;

/** Mirrors test/jobs.test.ts's own helper: a full device-flow round trip
 * returning a fresh Bearer access token for a brand-new account (unique email
 * + unique IP per call, per the R1 test-isolation notes). */
async function grantAccessToken(): Promise<{ token: string; accountId: string }> {
  accountCounter += 1;
  const ip = { "cf-connecting-ip": `198.51.101.${accountCounter}` };
  const email = `usage-user-${accountCounter}-${Date.now()}@example.com`;

  const startResponse = await fetchWorker("/v1/auth/device", { method: "POST", headers: ip });
  expect(startResponse.status).toBe(200);
  const start = (await startResponse.json()) as { device_code: string; user_code: string };

  const approveResponse = await fetchWorker("/activate", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded", ...ip },
    body: formBody({ user_code: start.user_code, email }),
  });
  expect(approveResponse.status).toBe(200);

  const tokenResponse = await fetchWorker("/v1/auth/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: formBody({
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
      device_code: start.device_code,
    }),
  });
  expect(tokenResponse.status).toBe(200);
  const body = (await tokenResponse.json()) as { access_token: string };

  const meResponse = await fetchWorker("/v1/me", { headers: { authorization: `Bearer ${body.access_token}` } });
  const me = (await meResponse.json()) as { account_id: string };

  return { token: body.access_token, accountId: me.account_id };
}

async function submitJob(token: string, gcode: string): Promise<Response> {
  return fetchWorker("/v1/jobs/verify?pack=demo-printer&version=0.1.0&profile=demo-profile", {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-length": String(new TextEncoder().encode(gcode).length),
    },
    body: gcode,
  });
}

async function insertJobRow(accountId: string, createdAt: Date, status = "done"): Promise<void> {
  await env.DB.prepare("INSERT INTO jobs (id, account_id, status, created_at) VALUES (?, ?, ?, ?)")
    .bind(crypto.randomUUID(), accountId, status, formatSqlDateTime(createdAt))
    .run();
}

async function insertUsageEvent(accountId: string, route: string, bytes: number, at: Date): Promise<void> {
  await env.DB.prepare("INSERT INTO usage_events (account_id, route, bytes, at) VALUES (?, ?, ?, ?)")
    .bind(accountId, route, bytes, formatSqlDateTime(at))
    .run();
}

async function countUsageEvents(accountId: string, route?: string): Promise<number> {
  const row = route
    ? await env.DB.prepare("SELECT COUNT(*) AS count FROM usage_events WHERE account_id = ? AND route = ?")
        .bind(accountId, route)
        .first<{ count: number }>()
    : await env.DB.prepare("SELECT COUNT(*) AS count FROM usage_events WHERE account_id = ?")
        .bind(accountId)
        .first<{ count: number }>();
  return row?.count ?? 0;
}

// --- Pure date/window math ---------------------------------------------------

describe("UTC month window math (pure functions, fake 'now' injected via a seam)", () => {
  it("utcMonthStart/utcNextMonthStart bracket an ordinary mid-month instant", () => {
    const now = new Date("2026-07-15T12:34:56Z");
    expect(utcMonthStart(now).toISOString()).toBe("2026-07-01T00:00:00.000Z");
    expect(utcNextMonthStart(now).toISOString()).toBe("2026-08-01T00:00:00.000Z");
  });

  it("rolls over correctly at a December -> January year boundary", () => {
    const now = new Date("2026-12-31T23:59:59Z");
    expect(utcMonthStart(now).toISOString()).toBe("2026-12-01T00:00:00.000Z");
    expect(utcNextMonthStart(now).toISOString()).toBe("2027-01-01T00:00:00.000Z");
  });

  it("treats the exact instant of month rollover as already in the new month", () => {
    const now = new Date("2026-08-01T00:00:00Z");
    expect(utcMonthStart(now).toISOString()).toBe("2026-08-01T00:00:00.000Z");
    expect(utcNextMonthStart(now).toISOString()).toBe("2026-09-01T00:00:00.000Z");
  });

  it("secondsUntilNextUtcMonth equals the exact remaining seconds for a pinned fake 'now'", () => {
    expect(secondsUntilNextUtcMonth(new Date("2026-07-31T23:59:00Z"))).toBe(60);
    expect(secondsUntilNextUtcMonth(new Date("2026-12-31T23:59:59Z"))).toBe(1);
    expect(secondsUntilNextUtcMonth(new Date("2026-02-01T00:00:00Z"))).toBe(28 * 24 * 60 * 60); // 2026 is not a leap year
  });

  it("formatSqlDateTime matches D1's own datetime('now') text format", async () => {
    const formatted = formatSqlDateTime(new Date("2026-07-15T12:34:56Z"));
    expect(formatted).toBe("2026-07-15 12:34:56");

    const row = await env.DB.prepare("SELECT datetime('now') AS now").first<{ now: string }>();
    // Same shape: "YYYY-MM-DD HH:MM:SS", 19 characters, space-separated.
    expect(row?.now).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/);
    expect(formatted).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/);
  });
});

// --- Canonical quota source: `jobs` table, not `usage_events` ---------------

describe("countJobsThisMonth: canonical job-quota source is the `jobs` table", () => {
  it("counts only rows within [this UTC month start, next UTC month start)", async () => {
    const { accountId } = await grantAccessToken();
    const now = new Date();
    const thisMonthStart = utcMonthStart(now);
    const lastInstantOfPrevMonth = new Date(thisMonthStart.getTime() - 1000);

    // Excluded: one second before this month started.
    await insertJobRow(accountId, lastInstantOfPrevMonth);
    // Included: exactly at this month's start (inclusive boundary).
    await insertJobRow(accountId, thisMonthStart);
    // Included: mid-month "now".
    await insertJobRow(accountId, now);
    // Excluded: exactly at next month's start (exclusive boundary).
    await insertJobRow(accountId, utcNextMonthStart(now));

    expect(await countJobsThisMonth(env, accountId, now)).toBe(2);
  });

  it("is unaffected by `usage_events` volume alone -- usage_events is the analytics source, not the quota source", async () => {
    const { accountId } = await grantAccessToken();
    const now = new Date();

    // Simulate a much larger usage_events volume than the (real) jobs count --
    // if the quota check were wrongly wired to usage_events, this would
    // already look like the account blew through QUOTA_JOBS_PER_MONTH (20 in
    // the dev vars) even though zero actual job rows exist.
    for (let i = 0; i < 30; i++) {
      await insertUsageEvent(accountId, "job", 100, now);
    }

    expect(await countJobsThisMonth(env, accountId, now)).toBe(0);
  });
});

describe("sumUsageBytesThisMonth", () => {
  it("sums only this month's usage_events rows for the account", async () => {
    const { accountId } = await grantAccessToken();
    const now = new Date();
    const thisMonthStart = utcMonthStart(now);
    const lastInstantOfPrevMonth = new Date(thisMonthStart.getTime() - 1000);

    await insertUsageEvent(accountId, "job", 500, lastInstantOfPrevMonth); // excluded
    await insertUsageEvent(accountId, "job", 1000, now); // included
    await insertUsageEvent(accountId, "keys", 0, now); // included, contributes 0
    await insertUsageEvent(accountId, "job", 2000, utcNextMonthStart(now)); // excluded

    expect(await sumUsageBytesThisMonth(env, accountId, now)).toBe(1000);
  });

  it("returns 0 (not null) for an account with no usage_events rows", async () => {
    const { accountId } = await grantAccessToken();
    expect(await sumUsageBytesThisMonth(env, accountId)).toBe(0);
  });
});

// --- GET /v1/usage ------------------------------------------------------------

describe("GET /v1/usage", () => {
  it("401s with no Authorization header", async () => {
    const response = await fetchWorker("/v1/usage");
    expect(response.status).toBe(401);
  });

  it("reports zeroed month usage and the configured quotas for a brand-new account", async () => {
    const { token } = await grantAccessToken();
    const response = await fetchWorker("/v1/usage", { headers: { authorization: `Bearer ${token}` } });
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body).toEqual({
      month: { jobs: 0, bytes: 0 },
      quotas: { jobs_per_month: 20, keys: 1 }, // wrangler.jsonc dev vars
    });
  });

  it("reflects a real job submission's count and byte size", async () => {
    const { token } = await grantAccessToken();
    const gcode = "G1 X10 Y10\nG1 X20 Y20\n";

    const submitResponse = await submitJob(token, gcode);
    expect(submitResponse.status).toBe(202);

    const response = await fetchWorker("/v1/usage", { headers: { authorization: `Bearer ${token}` } });
    const body = (await response.json()) as { month: { jobs: number; bytes: number } };
    expect(body.month.jobs).toBe(1);
    expect(body.month.bytes).toBe(new TextEncoder().encode(gcode).length);
  });
});

// --- usage_events written per authed request ---------------------------------

describe("usage_events: one row per authed request, route-classed job|keys|auth", () => {
  it("writes a `keys` row for POST /v1/keys", async () => {
    const { token, accountId } = await grantAccessToken();
    const before = await countUsageEvents(accountId, "keys");

    const response = await fetchWorker("/v1/keys", {
      method: "POST",
      headers: { authorization: `Bearer ${token}` },
    });
    expect(response.status).toBe(201);

    expect(await countUsageEvents(accountId, "keys")).toBe(before + 1);
  });

  it("writes an `auth` row for GET /v1/me", async () => {
    const { token, accountId } = await grantAccessToken();
    const before = await countUsageEvents(accountId, "auth");

    const response = await fetchWorker("/v1/me", { headers: { authorization: `Bearer ${token}` } });
    expect(response.status).toBe(200);

    expect(await countUsageEvents(accountId, "auth")).toBe(before + 1);
  });

  it("writes a `job` row with the uploaded byte size for POST /v1/jobs/verify", async () => {
    const { token, accountId } = await grantAccessToken();
    const gcode = "G1 X1\nG1 X2\nG1 X3\n";

    const response = await submitJob(token, gcode);
    expect(response.status).toBe(202);

    const row = await env.DB.prepare(
      "SELECT route, bytes FROM usage_events WHERE account_id = ? AND route = 'job' ORDER BY id DESC LIMIT 1",
    )
      .bind(accountId)
      .first<{ route: string; bytes: number }>();
    expect(row?.route).toBe("job");
    expect(row?.bytes).toBe(new TextEncoder().encode(gcode).length);
  });

  it("writes a `job` row for GET /v1/jobs/{id} (no request body -> 0 bytes)", async () => {
    const { token, accountId } = await grantAccessToken();
    const submitResponse = await submitJob(token, "G1 X1\n");
    const { id } = (await submitResponse.json()) as { id: string };

    const before = await countUsageEvents(accountId, "job");
    const response = await fetchWorker(`/v1/jobs/${id}`, { headers: { authorization: `Bearer ${token}` } });
    expect(response.status).toBe(200);

    expect(await countUsageEvents(accountId, "job")).toBe(before + 1);
  });

  it("writes NO row when auth itself fails (401)", async () => {
    const before = await env.DB.prepare("SELECT COUNT(*) AS count FROM usage_events").first<{ count: number }>();

    const response = await fetchWorker("/v1/me", { headers: { authorization: "Bearer not-a-real-token" } });
    expect(response.status).toBe(401);

    const after = await env.DB.prepare("SELECT COUNT(*) AS count FROM usage_events").first<{ count: number }>();
    expect(after?.count).toBe(before?.count);
  });
});

// --- Job quota: 429 shape + Retry-After -------------------------------------

describe("job quota exceeded: 429 shape aligned to {error, usage_url} + Retry-After", () => {
  async function exhaustJobQuota(accountId: string): Promise<void> {
    const now = new Date();
    for (let i = 0; i < 20; i++) {
      // QUOTA_JOBS_PER_MONTH=20 (wrangler.jsonc dev vars) -- inserted directly
      // into `jobs` (the canonical quota source) rather than via 20 real
      // submissions, to keep this test fast; src/jobs.ts's own happy-path
      // test already covers a real submission end to end.
      await insertJobRow(accountId, now);
    }
  }

  it("429s with Retry-After and the exact {error, usage_url} body once the account is at quota", async () => {
    const { token, accountId } = await grantAccessToken();
    await exhaustJobQuota(accountId);

    const before = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs WHERE account_id = ?")
      .bind(accountId)
      .first<{ count: number }>();

    const response = await submitJob(token, "G1 X1\n");
    expect(response.status).toBe(429);
    expect(await response.json()).toEqual({ error: "quota_exceeded", usage_url: "/v1/usage" });

    const retryAfterHeader = response.headers.get("retry-after");
    expect(retryAfterHeader).toBeTruthy();
    const retryAfter = Number.parseInt(retryAfterHeader ?? "", 10);
    // Cross-check against the pure function for real "now" (a few seconds'
    // tolerance for wall-clock drift during the test itself).
    expect(Math.abs(retryAfter - secondsUntilNextUtcMonth(new Date()))).toBeLessThanOrEqual(5);

    // No new job row was created for the rejected submission -- rejected
    // strictly before any R2 write or D1 insert, same ordering guarantee as
    // the other pre-write checks in src/jobs.ts.
    const after = await env.DB.prepare("SELECT COUNT(*) AS count FROM jobs WHERE account_id = ?")
      .bind(accountId)
      .first<{ count: number }>();
    expect(after?.count).toBe(before?.count);
  });

  it("still records a usage_events row for the quota-exceeded (but authenticated) request", async () => {
    const { token, accountId } = await grantAccessToken();
    await exhaustJobQuota(accountId);

    const before = await countUsageEvents(accountId, "job");
    const response = await submitJob(token, "G1 X1\n");
    expect(response.status).toBe(429);

    expect(await countUsageEvents(accountId, "job")).toBe(before + 1);
  });
});
