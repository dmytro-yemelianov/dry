// Cloudflare Worker: signs Dry license tokens from Lemon Squeezy webhooks
// (and via a manual admin endpoint), logs every issuance to D1, and emails
// the token to the buyer. Implements Task 6 of
// docs/superpowers/plans/2026-07-28-commercial-license-product.md.
import { buildPayload, type LicensePayload, type Tier } from "./token";
import { signPayload } from "./sign";

export interface Env {
  DB: D1Database;
  EMAIL: SendEmailBinding;
  /** HMAC-SHA256 secret Lemon Squeezy signs webhook bodies with. */
  LS_WEBHOOK_SECRET: string;
  /** PKCS8 Ed25519 signing key, base64 (as produced by scripts/keygen.mjs). */
  SIGNING_KEY_PKCS8_B64: string;
  /** Bearer token for POST /admin/issue. */
  ADMIN_TOKEN: string;
  KEY_ID: string;
  /** JSON map of Lemon Squeezy variant id -> Dry tier, e.g. {"123":"solo"}. */
  TIER_BY_VARIANT: string;
  MAIL_FROM: string;
  MAIL_TO_BCC?: string;
}

/** Minimal shape of the `send_email` binding's MessageBuilder-style send(). */
interface SendEmailBinding {
  send(message: {
    to: string;
    from: { email: string; name?: string };
    bcc?: string;
    subject: string;
    text?: string;
    html?: string;
  }): Promise<unknown>;
}

const SECURITY_HEADERS: HeadersInit = {
  "x-content-type-options": "nosniff",
  "referrer-policy": "no-referrer",
  "cache-control": "no-store",
};

function jsonResponse(value: unknown, status = 200): Response {
  return Response.json(value, { status, headers: SECURITY_HEADERS });
}

function textResponse(body: string, status: number): Response {
  return new Response(body, {
    status,
    headers: { ...SECURITY_HEADERS, "content-type": "text/plain; charset=utf-8" },
  });
}

function hexToBytes(hex: string): Uint8Array | undefined {
  if (hex.length === 0 || hex.length % 2 !== 0) return undefined;
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    const byte = Number.parseInt(hex.substr(i * 2, 2), 16);
    if (Number.isNaN(byte)) return undefined;
    bytes[i] = byte;
  }
  return bytes;
}

/** Constant-time string compare, for bearer tokens (HMAC verification below uses subtle.verify). */
function timingSafeEqual(a: string, b: string): boolean {
  const aBytes = new TextEncoder().encode(a);
  const bBytes = new TextEncoder().encode(b);
  if (aBytes.length !== bBytes.length) return false;
  let diff = 0;
  for (let i = 0; i < aBytes.length; i++) diff |= aBytes[i] ^ bBytes[i];
  return diff === 0;
}

async function verifyLemonSqueezySignature(secret: string, rawBody: string, signatureHex: string): Promise<boolean> {
  const sigBytes = hexToBytes(signatureHex.trim());
  if (!sigBytes) return false;
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["verify"],
  );
  return crypto.subtle.verify("HMAC", key, sigBytes, new TextEncoder().encode(rawBody));
}

const MACHINES_BY_TIER: Record<Tier, number> = { solo: 3, team: 25, pilot: 25 };

async function insertLicense(env: Env, payload: LicensePayload, orderId: string | undefined): Promise<void> {
  await env.DB.prepare(
    "INSERT INTO licenses (id, email, licensee, tier, expires_unix, order_id) VALUES (?, ?, ?, ?, ?, ?)",
  )
    .bind(payload.id, payload.email, payload.licensee, payload.tier, payload.expires_unix, orderId ?? null)
    .run();
}

async function emailLicense(env: Env, payload: LicensePayload, token: string): Promise<void> {
  await env.EMAIL.send({
    to: payload.email,
    from: { email: env.MAIL_FROM, name: "Dry License" },
    bcc: env.MAIL_TO_BCC || undefined,
    subject: `Your Dry ${payload.tier} license`,
    text:
      `Hi ${payload.licensee},\n\n` +
      `Your Dry license token (tier: ${payload.tier}, expires ${payload.expires}):\n\n${token}\n\n` +
      `Save it with: dry license activate ${token}\n`,
    html:
      `<p>Hi ${payload.licensee},</p>` +
      `<p>Your Dry license token (tier: ${payload.tier}, expires ${payload.expires}):</p>` +
      `<pre>${token}</pre>` +
      `<p>Save it with: <code>dry license activate ${token}</code></p>`,
  });
}

async function issueLicense(
  env: Env,
  opts: { licensee: string; email: string; tier: Tier; machines?: number; days?: number; orderId?: string },
): Promise<{ payload: LicensePayload; token: string }> {
  const nowUnix = Math.floor(Date.now() / 1000);
  const termSecs = opts.days !== undefined ? opts.days * 24 * 60 * 60 : undefined;
  const payload = buildPayload({
    id: crypto.randomUUID(),
    licensee: opts.licensee,
    email: opts.email,
    tier: opts.tier,
    keyId: env.KEY_ID,
    nowUnix,
    machines: opts.machines ?? MACHINES_BY_TIER[opts.tier],
    termSecs,
  });
  const token = await signPayload(payload, env.SIGNING_KEY_PKCS8_B64);
  await insertLicense(env, payload, opts.orderId);
  await emailLicense(env, payload, token);
  return { payload, token };
}

// --- Lemon Squeezy webhook parsing -----------------------------------------
//
// ASSUMED field shape (Lemon Squeezy webhook docs -- `data.attributes`
// nests the resource's fields, `meta.event_name` names the event; no live
// test-mode purchase has exercised these yet, see plan Task 10):
// - order_created: `data.type === "orders"`; `data.id` is the order id;
//   customer email/name at `attributes.user_email` / `attributes.user_name`;
//   the purchased variant id at `attributes.first_order_item.variant_id`.
// - subscription_payment_success: `data.type === "subscription-invoices"`;
//   customer email/name at `attributes.user_email` / `attributes.user_name`;
//   variant id at `attributes.variant_id`; the ORIGINAL order id (used to
//   correlate refunds/expirations against the same license row) at
//   `attributes.order_id`.
// - order_refunded / subscription_expired: same shape, used only to look up
//   `order_id` and revoke -- no new license is issued for these.
// Adjust the extractors below if a real payload's field names differ.

interface LemonSqueezyEvent {
  meta?: { event_name?: string };
  data?: {
    id?: string;
    type?: string;
    attributes?: Record<string, unknown>;
  };
}

function tierForVariant(env: Env, variantId: unknown): Tier | undefined {
  if (variantId === undefined || variantId === null) return undefined;
  let map: Record<string, string>;
  try {
    map = JSON.parse(env.TIER_BY_VARIANT);
  } catch {
    return undefined;
  }
  const tier = map[String(variantId)];
  return tier === "solo" || tier === "team" || tier === "pilot" ? tier : undefined;
}

function extractCustomer(attrs: Record<string, unknown>): { email: string; name: string } | undefined {
  const email = attrs.user_email;
  if (typeof email !== "string" || email.length === 0) return undefined;
  const name = typeof attrs.user_name === "string" && attrs.user_name.length > 0 ? attrs.user_name : email;
  return { email, name };
}

function extractVariantId(attrs: Record<string, unknown>): unknown {
  const firstItem = attrs.first_order_item as Record<string, unknown> | undefined;
  return attrs.variant_id ?? firstItem?.variant_id;
}

function extractOrderId(event: LemonSqueezyEvent): string | undefined {
  if (event.data?.type === "orders") return event.data.id;
  const orderId = event.data?.attributes?.order_id;
  if (typeof orderId === "string") return orderId;
  if (typeof orderId === "number") return String(orderId);
  return undefined;
}

async function handleWebhook(request: Request, env: Env): Promise<Response> {
  const rawBody = await request.text();
  const signature = request.headers.get("X-Signature") ?? "";
  const valid = await verifyLemonSqueezySignature(env.LS_WEBHOOK_SECRET, rawBody, signature);
  if (!valid) return textResponse("invalid signature", 401);

  let event: LemonSqueezyEvent;
  try {
    event = JSON.parse(rawBody);
  } catch {
    return textResponse("malformed payload", 400);
  }

  const eventName = event.meta?.event_name;
  const attrs = event.data?.attributes ?? {};

  if (eventName === "order_created" || eventName === "subscription_payment_success") {
    const customer = extractCustomer(attrs);
    const tier = tierForVariant(env, extractVariantId(attrs));
    if (!customer || !tier) {
      // Can't map this event to a license -- accept it (so LS doesn't retry
      // forever) but log for manual follow-up.
      console.warn(`license-issuer: unmapped ${eventName} event ${event.data?.id ?? "?"}`);
      return jsonResponse({ ok: true, skipped: true });
    }
    await issueLicense(env, {
      licensee: customer.name,
      email: customer.email,
      tier,
      orderId: extractOrderId(event),
    });
    return jsonResponse({ ok: true });
  }

  if (eventName === "order_refunded" || eventName === "subscription_expired") {
    const orderId = extractOrderId(event);
    if (orderId) {
      await env.DB.prepare("UPDATE licenses SET revoked = 1 WHERE order_id = ?").bind(orderId).run();
    }
    return jsonResponse({ ok: true });
  }

  // Unhandled event types are accepted, not errors -- LS sends many we don't act on.
  return jsonResponse({ ok: true, ignored: eventName ?? null });
}

interface AdminIssueBody {
  licensee: string;
  email: string;
  tier: Tier;
  machines?: number;
  days?: number;
}

function isValidTier(value: unknown): value is Tier {
  return value === "solo" || value === "team" || value === "pilot";
}

async function handleAdminIssue(request: Request, env: Env): Promise<Response> {
  const auth = request.headers.get("Authorization") ?? "";
  if (!env.ADMIN_TOKEN || !timingSafeEqual(auth, `Bearer ${env.ADMIN_TOKEN}`)) {
    return textResponse("unauthorized", 401);
  }

  let body: Partial<AdminIssueBody>;
  try {
    body = await request.json();
  } catch {
    return textResponse("malformed body", 400);
  }

  if (typeof body.licensee !== "string" || typeof body.email !== "string" || !isValidTier(body.tier)) {
    return textResponse("licensee, email, tier are required", 400);
  }

  const { payload, token } = await issueLicense(env, {
    licensee: body.licensee,
    email: body.email,
    tier: body.tier,
    machines: typeof body.machines === "number" ? body.machines : undefined,
    days: typeof body.days === "number" ? body.days : undefined,
  });

  // Return the token in the response too, for manual delivery if email fails.
  return jsonResponse({ ok: true, token, payload });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    if (request.method === "POST" && url.pathname === "/webhook/lemonsqueezy") {
      return handleWebhook(request, env);
    }
    if (request.method === "POST" && url.pathname === "/admin/issue") {
      return handleAdminIssue(request, env);
    }
    return textResponse("not found", 404);
  },
};
