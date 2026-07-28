// Usage metering + job quotas (Task R4).
//
// **Canonical-source reconciliation (documented per the task brief's explicit
// request):** two tables can both answer "how many jobs has this account
// submitted this month" -- `jobs` (one row per actually-created job) and
// `usage_events` (one row per authed HTTP request, written unconditionally by
// the `authed()` wrapper in index.ts). They are NOT the same number: a
// request that 411/413/404/502/429-rejects before the `jobs` INSERT still
// gets a `usage_events` row (it WAS an authenticated request), but never a
// `jobs` row. Picking `usage_events` as the quota source would therefore let
// a client's own rejected/retried requests count against their quota, which
// is backwards -- and would make quota enforcement and job history disagree.
// `jobs` is the more truthful source for enforcement (it counts only jobs
// that actually got created and will actually consume runner capacity), so
// `countJobsThisMonth` (backed by the `jobs` table) is THE canonical source
// for `QUOTA_JOBS_PER_MONTH` -- used by both src/jobs.ts's pre-insert check
// and this module's own `GET /v1/usage` `month.jobs` figure, so the number a
// caller sees always matches the number the 429 is computed from.
// `usage_events` remains the source for everything `jobs` cannot answer --
// here, `month.bytes` (request byte sizes; `jobs` does not store this) -- and
// is the general request-level audit trail for `job`/`keys`/`auth` traffic.

/** Every route class `usage_events.route` can hold ("Every authed request ->
 * one usage_events row (route class job|keys|auth)" per the task brief).
 * Assigned by path prefix in index.ts's `authed()` wrapper: `/v1/jobs/*` ->
 * "job", `/v1/keys*` -> "keys", everything else authenticated (`/v1/me`,
 * `/v1/usage` itself) -> "auth". */
export type UsageRouteClass = "job" | "keys" | "auth";

function jsonResponse(value: unknown, status = 200, extraHeaders?: HeadersInit): Response {
  const headers = new Headers(extraHeaders);
  headers.set("content-type", "application/json; charset=utf-8");
  return Response.json(value, { status, headers });
}

/** Shared quota-var parser (moved here from its former per-file duplicates in
 * index.ts/src/jobs.ts -- both now import this one copy, and this module
 * needed its own anyway for `buildUsageSummary`'s `quotas` field). Falls back
 * to `fallback` for anything not a positive integer (unset, empty, `"0"`,
 * non-numeric, negative) -- a misconfigured var should never silently mean
 * "unlimited". */
export function parseQuota(raw: string | undefined, fallback: number): number {
  const parsed = Number.parseInt(raw ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

/** Start of `now`'s UTC calendar month, at 00:00:00.000 UTC. */
export function utcMonthStart(now: Date = new Date()): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), 1, 0, 0, 0, 0));
}

/** Start of the UTC calendar month AFTER `now`'s -- the exclusive upper bound
 * of the current month's window. `Date.UTC` normalizes month=12 into
 * January of the following year on its own, so December -> January year
 * rollovers need no special-casing here. */
export function utcNextMonthStart(now: Date = new Date()): Date {
  return new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() + 1, 1, 0, 0, 0, 0));
}

/** Seconds from `now` until the next UTC month starts, rounded up (a client
 * retrying at exactly this many seconds is guaranteed to already be in the
 * new month, never a fraction of a second early). This is a pure function of
 * its `now` argument -- the seam the task brief asks for -- so it can be
 * asserted exactly against a pinned fake "now" in tests, with no
 * `vi.useFakeTimers()`/sleep required. */
export function secondsUntilNextUtcMonth(now: Date = new Date()): number {
  return Math.ceil((utcNextMonthStart(now).getTime() - now.getTime()) / 1000);
}

/** Formats a `Date` to match D1/SQLite's own `datetime('now')` text
 * representation exactly: `YYYY-MM-DD HH:MM:SS`, space-separated, UTC, no
 * fractional seconds and no `T`/`Z` suffix. `jobs.created_at` and
 * `usage_events.at` are both `TEXT DEFAULT (datetime('now'))` (schema.sql),
 * so window-boundary bind parameters must share this exact shape for plain
 * lexicographic `>=`/`<` string comparison to agree with chronological order. */
export function formatSqlDateTime(date: Date): string {
  return date.toISOString().slice(0, 19).replace("T", " ");
}

/** Records one `usage_events` row. Called unconditionally, for every request
 * that passes bearer-token auth, by index.ts's `authed()` wrapper -- BEFORE
 * the wrapped handler runs, so a request that authenticates but is then
 * rejected downstream (quota-exceeded, invalid input, etc.) still counts as
 * "an authed request" for this analytics trail. That is deliberate (see the
 * module doc comment above): it never affects job-quota enforcement, which
 * is computed from `jobs`, not from this table. */
export async function recordUsageEvent(
  env: Env,
  accountId: string,
  route: UsageRouteClass,
  bytes = 0,
): Promise<void> {
  await env.DB.prepare("INSERT INTO usage_events (account_id, route, bytes) VALUES (?, ?, ?)")
    .bind(accountId, route, bytes)
    .run();
}

/** THE canonical count for `QUOTA_JOBS_PER_MONTH` enforcement (see the module
 * doc comment) -- counts `jobs` rows in `[thisMonthStart, nextMonthStart)`,
 * regardless of a job's terminal status (queued/done/error all still consumed
 * a submission slot for the month). Window bounds are computed in JS from
 * `now` and passed as bind parameters, rather than relying on SQLite's own
 * `datetime('now')` -- this is itself the test seam: callers (including
 * tests) can pass an explicit `now` to evaluate the window against a
 * specific instant without needing to fake the database's own clock. */
export async function countJobsThisMonth(env: Env, accountId: string, now: Date = new Date()): Promise<number> {
  const start = formatSqlDateTime(utcMonthStart(now));
  const nextStart = formatSqlDateTime(utcNextMonthStart(now));
  const row = await env.DB.prepare(
    "SELECT COUNT(*) AS count FROM jobs WHERE account_id = ? AND created_at >= ? AND created_at < ?",
  )
    .bind(accountId, start, nextStart)
    .first<{ count: number }>();
  return row?.count ?? 0;
}

/** Analytics source for `GET /v1/usage`'s `month.bytes` -- summed from
 * `usage_events` (the only table that records request byte sizes at all;
 * `jobs` doesn't). Same `[thisMonthStart, nextMonthStart)` window as
 * `countJobsThisMonth`, so both figures in one `/v1/usage` response describe
 * the same month. `COALESCE(..., 0)` so an account with zero rows this month
 * gets `0`, not `null`. */
export async function sumUsageBytesThisMonth(env: Env, accountId: string, now: Date = new Date()): Promise<number> {
  const start = formatSqlDateTime(utcMonthStart(now));
  const nextStart = formatSqlDateTime(utcNextMonthStart(now));
  const row = await env.DB.prepare(
    "SELECT COALESCE(SUM(bytes), 0) AS total FROM usage_events WHERE account_id = ? AND at >= ? AND at < ?",
  )
    .bind(accountId, start, nextStart)
    .first<{ total: number }>();
  return row?.total ?? 0;
}

export interface UsageSummary {
  month: { jobs: number; bytes: number };
  quotas: { jobs_per_month: number; keys: number };
}

/** `GET /v1/usage`'s body. `month.jobs` intentionally comes from the SAME
 * canonical source (`countJobsThisMonth`) the 429 quota check itself uses, so
 * a caller polling this endpoint always sees the exact number that would
 * trip (or is about to trip) `quota_exceeded` -- never a different figure
 * from a second, independent counting method. */
export async function buildUsageSummary(env: Env, accountId: string, now: Date = new Date()): Promise<UsageSummary> {
  const [jobs, bytes] = await Promise.all([
    countJobsThisMonth(env, accountId, now),
    sumUsageBytesThisMonth(env, accountId, now),
  ]);
  return {
    month: { jobs, bytes },
    quotas: {
      jobs_per_month: parseQuota(env.QUOTA_JOBS_PER_MONTH, 20),
      keys: parseQuota(env.QUOTA_KEYS, 1),
    },
  };
}

/** `GET /v1/usage` (Bearer) -- already-authenticated `accountId`, same
 * calling convention as every other authed handler (see index.ts's
 * `authed()` wrapper). */
export async function handleGetUsage(env: Env, accountId: string): Promise<Response> {
  return jsonResponse(await buildUsageSummary(env, accountId));
}

/** The exact 429 shape the task brief specifies for a quota-exceeded job
 * submission: `{"error":"quota_exceeded","usage_url":"/v1/usage"}` plus a
 * `Retry-After` header giving the number of seconds until the UTC month
 * (and therefore the quota window) rolls over. Deliberately narrower than the
 * older ad hoc `{error, quota}` 403 this replaces in src/jobs.ts (see that
 * file's quota check) -- a caller that wants the actual quota NUMBER now
 * fetches `usage_url` instead of it being duplicated ad hoc on every 429. */
export function quotaExceededResponse(now: Date = new Date()): Response {
  const retryAfter = secondsUntilNextUtcMonth(now);
  return jsonResponse(
    { error: "quota_exceeded", usage_url: "/v1/usage" },
    429,
    { "retry-after": String(retryAfter) },
  );
}
