// RFC 8628 device authorization flow, plus the shared bearer-token auth
// middleware used by every authenticated route.

import { checkRateLimit, getClientIp } from "./ratelimit";
import {
  generateAccessToken,
  generateDeviceCode,
  generateUserCode,
  normalizeUserCode,
  sha256Hex,
  timingSafeEqualHex,
} from "./tokens";

export const DEVICE_CODE_TTL_SECONDS = 600;
export const POLL_INTERVAL_SECONDS = 5;

interface DeviceRecord {
  userCode: string;
  status: "pending" | "approved";
  accountId?: string;
  /** Absolute epoch-ms deadline, fixed at creation — the KV entry's own TTL
   * is refreshed on every write (see below) so it never falls below D1/KV's
   * 60s minimum; this field is the actual, authoritative expiry check. */
  expiresAt: number;
  /** Epoch-ms of the last poll, for RFC 8628 `slow_down` tracking. */
  lastPolledAt?: number;
}

function deviceKey(deviceCode: string): string {
  return `dev:${deviceCode}`;
}

function userKey(userCode: string): string {
  return `usr:${userCode}`;
}

async function putDeviceRecord(env: Env, deviceCode: string, record: DeviceRecord): Promise<void> {
  // Always refresh with the fixed nominal TTL rather than the shrinking
  // remainder — KV requires >= 60s, and remaining-time-to-expiresAt can dip
  // below that near the end of a poll cycle. Correctness comes from the
  // `expiresAt` field being checked explicitly, not from this physical TTL.
  await env.CODES.put(deviceKey(deviceCode), JSON.stringify(record), {
    expirationTtl: DEVICE_CODE_TTL_SECONDS,
  });
}

function jsonResponse(value: unknown, status = 200, extraHeaders?: HeadersInit): Response {
  const headers = new Headers(extraHeaders);
  headers.set("content-type", "application/json; charset=utf-8");
  return Response.json(value, { status, headers });
}

/** `POST /v1/auth/device` — issues a device_code/user_code pair. */
export async function handleDeviceStart(request: Request, env: Env, origin: string): Promise<Response> {
  if (request.method !== "POST") {
    return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "POST" });
  }

  const ip = getClientIp(request);
  if (await checkRateLimit(env, "device", ip, 10)) {
    return jsonResponse({ error: "rate_limited" }, 429, { "retry-after": "600" });
  }

  const deviceCode = generateDeviceCode();
  const userCode = generateUserCode();
  const record: DeviceRecord = {
    userCode,
    status: "pending",
    expiresAt: Date.now() + DEVICE_CODE_TTL_SECONDS * 1000,
  };

  await putDeviceRecord(env, deviceCode, record);
  await env.CODES.put(userKey(userCode), deviceCode, {
    expirationTtl: DEVICE_CODE_TTL_SECONDS,
  });

  const verificationUri = `${origin}/activate`;
  return jsonResponse({
    device_code: deviceCode,
    user_code: userCode,
    verification_uri: verificationUri,
    verification_uri_complete: `${verificationUri}?user_code=${encodeURIComponent(userCode)}`,
    expires_in: DEVICE_CODE_TTL_SECONDS,
    interval: POLL_INTERVAL_SECONDS,
  });
}

export type ResolvedDeviceCode = { deviceCode: string; record: DeviceRecord };

/**
 * Read-only lookup used by `/activate` to check a user-entered code BEFORE
 * doing anything account-affecting (e.g. upserting by email) — so a bad or
 * expired code never has a side effect beyond the KV read, and can't be used
 * to spam rows into `accounts`.
 */
export async function resolveUserCode(
  env: Env,
  userCodeInput: string,
): Promise<ResolvedDeviceCode | "not_found" | "expired"> {
  const userCode = normalizeUserCode(userCodeInput);
  if (!userCode) return "not_found";

  const deviceCode = await env.CODES.get(userKey(userCode));
  if (!deviceCode) return "not_found";

  const raw = await env.CODES.get(deviceKey(deviceCode));
  if (!raw) return "not_found";

  const record = JSON.parse(raw) as DeviceRecord;
  if (Date.now() > record.expiresAt) {
    await env.CODES.delete(deviceKey(deviceCode));
    await env.CODES.delete(userKey(userCode));
    return "expired";
  }

  return { deviceCode, record };
}

/**
 * Called by the `/activate` POST handler once Turnstile has passed, the code
 * has been resolved with `resolveUserCode`, and the account has been
 * resolved. Marks the device code approved so the next `/v1/auth/token` poll
 * can mint an access token.
 */
export async function approveDevice(
  env: Env,
  resolved: ResolvedDeviceCode,
  accountId: string,
): Promise<void> {
  const record: DeviceRecord = { ...resolved.record, status: "approved", accountId };
  await putDeviceRecord(env, resolved.deviceCode, record);
}

/** `POST /v1/auth/token` — RFC 8628 device_code grant. */
export async function handleToken(request: Request, env: Env): Promise<Response> {
  if (request.method !== "POST") {
    return jsonResponse({ error: "method_not_allowed" }, 405, { allow: "POST" });
  }

  const contentType = request.headers.get("content-type") ?? "";
  if (!contentType.toLowerCase().includes("application/x-www-form-urlencoded")) {
    return jsonResponse({ error: "invalid_request" }, 400);
  }

  const form = await request.formData();
  const grantType = form.get("grant_type");
  const deviceCode = form.get("device_code");

  if (
    grantType !== "urn:ietf:params:oauth:grant-type:device_code" ||
    typeof deviceCode !== "string" ||
    !deviceCode
  ) {
    return jsonResponse({ error: "unsupported_grant_type" }, 400);
  }

  const raw = await env.CODES.get(deviceKey(deviceCode));
  if (!raw) {
    return jsonResponse({ error: "expired_token" }, 400);
  }

  const record = JSON.parse(raw) as DeviceRecord;

  if (Date.now() > record.expiresAt) {
    await env.CODES.delete(deviceKey(deviceCode));
    await env.CODES.delete(userKey(record.userCode));
    return jsonResponse({ error: "expired_token" }, 400);
  }

  if (record.status === "approved" && record.accountId) {
    // KV has no compare-and-swap, so it cannot be the atomic gate for
    // single-use redemption by itself: two concurrent pollers can both
    // observe `status === "approved"` before either one's KV deletes land.
    // D1's PRIMARY KEY is the actual atomic gate here -- exactly one INSERT
    // into `grants` can succeed for a given device_code, and only that
    // caller is allowed to mint a token.
    try {
      const grant = await env.DB.prepare("INSERT INTO grants (device_code) VALUES (?)").bind(deviceCode).run();
      if (!grant.meta.changes) {
        return jsonResponse({ error: "expired_token" }, 400);
      }
    } catch {
      // UNIQUE/PRIMARY KEY constraint violation: another request already won
      // the race and is minting (or has minted) the token for this code.
      return jsonResponse({ error: "expired_token" }, 400);
    }

    // Single-use: both KV entries are deleted the moment a token is granted,
    // so a replayed poll with the same device_code sees `expired_token`.
    await env.CODES.delete(deviceKey(deviceCode));
    await env.CODES.delete(userKey(record.userCode));

    const accessToken = generateAccessToken();
    const hash = await sha256Hex(accessToken);
    await env.DB.prepare("INSERT INTO tokens (hash, account_id, kind) VALUES (?, ?, 'at')")
      .bind(hash, record.accountId)
      .run();

    return jsonResponse({ access_token: accessToken, token_type: "Bearer" });
  }

  // Still pending: enforce the poll interval before anything else.
  const now = Date.now();
  const polledTooSoon =
    record.lastPolledAt !== undefined && now - record.lastPolledAt < POLL_INTERVAL_SECONDS * 1000;

  record.lastPolledAt = now;
  await putDeviceRecord(env, deviceCode, record);

  if (polledTooSoon) {
    return jsonResponse({ error: "slow_down" }, 400);
  }
  return jsonResponse({ error: "authorization_pending" }, 400);
}

export type AuthResult = { ok: true; accountId: string } | { ok: false; response: Response };

/**
 * Shared bearer-token middleware for every authenticated route: hashes the
 * presented token with SHA-256, looks up the account by that hash (D1
 * primary key), then re-verifies the match with `crypto.subtle.timingSafeEqual`
 * before trusting the row and checking `revoked`.
 */
export async function requireAuth(request: Request, env: Env): Promise<AuthResult> {
  const unauthorized = (): AuthResult => ({
    ok: false,
    response: jsonResponse({ error: "unauthorized" }, 401),
  });

  const header = request.headers.get("authorization") ?? "";
  const match = /^Bearer\s+(\S+)$/.exec(header);
  if (!match) return unauthorized();

  const presentedHash = await sha256Hex(match[1]);
  const row = await env.DB.prepare("SELECT hash, account_id, revoked FROM tokens WHERE hash = ?")
    .bind(presentedHash)
    .first<{ hash: string; account_id: string; revoked: number }>();
  if (!row) return unauthorized();

  if (!timingSafeEqualHex(presentedHash, row.hash)) return unauthorized();
  if (row.revoked) return unauthorized();

  return { ok: true, accountId: row.account_id };
}
