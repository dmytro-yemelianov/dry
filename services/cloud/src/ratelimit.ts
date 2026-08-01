// Best-effort abuse throttle for the anonymous (no bearer token required)
// POST endpoints: /activate and /v1/auth/device.
//
// Backed by a KV counter keyed by client IP. KV has no compare-and-swap, so
// the get-then-put here is NOT atomic: a burst of near-simultaneous requests
// can each read the same pre-increment count and all be admitted. That makes
// this an *approximate* limit, not a hard guarantee -- acceptable for
// throttling abuse/credential-stuffing traffic, not meant to be an exact or
// adversarially-hardened rate limiter.

const WINDOW_SECONDS = 600;

function rateLimitKey(route: string, ip: string): string {
  return `rl:${route}:${ip}`;
}

/** Resolves the caller's IP for rate-limit keying. Falls back to a shared
 * "unknown" bucket when Cloudflare hasn't set the header (e.g. local dev
 * without the Cloudflare network in front of the Worker). */
export function getClientIp(request: Request): string {
  return request.headers.get("cf-connecting-ip") ?? "unknown";
}

/**
 * Returns `true` if `route`+`ip` has already reached `max` attempts within
 * the current (approximate) window and the caller should be rejected with
 * 429; otherwise records this attempt and returns `false`.
 */
export async function checkRateLimit(env: Env, route: string, ip: string, max: number): Promise<boolean> {
  const key = rateLimitKey(route, ip);
  const raw = await env.CODES.get(key);
  const count = raw === null ? 0 : (Number.parseInt(raw, 10) || 0);

  if (count >= max) {
    return true;
  }

  await env.CODES.put(key, String(count + 1), { expirationTtl: WINDOW_SECONDS });
  return false;
}
