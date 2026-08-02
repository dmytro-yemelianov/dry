// Tests for the license-issuer Worker (Task 6). Runs under
// @cloudflare/vitest-pool-workers -- miniflare D1 (see wrangler.jsonc's
// "test" env + vitest.config.ts) and a `vi.spyOn` recording stub over the
// `send_email` binding. NO real network, NO real emails: the EMAIL send()
// call is mocked before every test that would otherwise trigger it.
import { env, exports } from "cloudflare:workers";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { Env } from "../src/index";

type TestEnv = Env & { TEST_SCHEMA_STATEMENTS: string[] };
const testEnv = env as unknown as TestEnv;

const ORIGIN = "http://example.com";
// Matches wrangler.jsonc's "test" env vars exactly.
const WEBHOOK_SECRET = "test-webhook-secret";
const ADMIN_TOKEN = "test-admin-token";

// The committed TEST Ed25519 public key (crates/license/tests/fixtures/
// test-signing-key.json) -- explicitly non-secret.
const TEST_VERIFYING_KEY_HEX = "e3d3920c08e704cca8183df61dfe4b9824c443b6ab230c205224432066e24460";

function hexToBytes(hex: string): Uint8Array {
  const clean = hex.length % 2 === 0 ? hex : hex.slice(0, -1);
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = Number.parseInt(clean.substr(i * 2, 2), 16);
  }
  return bytes;
}

async function hmacHex(secret: string, body: string): Promise<string> {
  const key = await crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(body));
  return [...new Uint8Array(sig)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

interface DecodedToken {
  payload: {
    id: string;
    licensee: string;
    email: string;
    tier: string;
    machines: number;
    issued: string;
    expires: string;
    issued_unix: number;
    expires_unix: number;
    key_id: string;
  };
  verified: boolean;
}

async function decodeAndVerifyToken(token: string): Promise<DecodedToken> {
  const parts = token.split(".");
  expect(parts).toHaveLength(3);
  const [prefix, payloadB64, sigB64] = parts;
  expect(prefix).toBe("DRY-LICENSE-V1");

  const payloadJson = new TextDecoder().decode(
    Uint8Array.from(atob(payloadB64.replaceAll("-", "+").replaceAll("_", "/")), (c) => c.charCodeAt(0)),
  );
  const payload = JSON.parse(payloadJson);

  const sigBytes = Uint8Array.from(atob(sigB64.replaceAll("-", "+").replaceAll("_", "/")), (c) => c.charCodeAt(0));
  const publicKey = await crypto.subtle.importKey(
    "raw",
    hexToBytes(TEST_VERIFYING_KEY_HEX),
    "Ed25519",
    false,
    ["verify"],
  );
  const verified = await crypto.subtle.verify(
    "Ed25519",
    publicKey,
    sigBytes,
    new TextEncoder().encode(payloadB64),
  );
  return { payload, verified };
}

function url(path: string): string {
  return new URL(path, ORIGIN).toString();
}

async function postWebhook(body: unknown, opts: { signature?: string } = {}): Promise<Response> {
  const raw = JSON.stringify(body);
  const signature = opts.signature ?? (await hmacHex(WEBHOOK_SECRET, raw));
  return exports.default.fetch(url("/webhook/lemonsqueezy"), {
    method: "POST",
    headers: { "content-type": "application/json", "X-Signature": signature },
    body: raw,
  });
}

async function postAdminIssue(body: unknown, bearer?: string): Promise<Response> {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (bearer !== undefined) headers.Authorization = `Bearer ${bearer}`;
  return exports.default.fetch(url("/admin/issue"), {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
}

function orderCreatedEvent(opts: { orderId: string; email: string; name: string; variantId: number }) {
  return {
    meta: { event_name: "order_created" },
    data: {
      id: opts.orderId,
      type: "orders",
      attributes: {
        user_email: opts.email,
        user_name: opts.name,
        first_order_item: { variant_id: opts.variantId },
      },
    },
  };
}

function refundEvent(orderId: string) {
  return {
    meta: { event_name: "order_refunded" },
    data: { id: "irrelevant-refund-id", type: "refunds", attributes: { order_id: orderId } },
  };
}

beforeAll(async () => {
  for (const statement of testEnv.TEST_SCHEMA_STATEMENTS) {
    await testEnv.DB.exec(statement);
  }
});

beforeEach(async () => {
  await testEnv.DB.exec("DELETE FROM licenses");
  vi.spyOn(testEnv.EMAIL, "send").mockResolvedValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("POST /webhook/lemonsqueezy", () => {
  it("rejects a bad HMAC signature: 401, no D1 row, no email", async () => {
    const response = await postWebhook(
      orderCreatedEvent({ orderId: "bad-sig-order", email: "x@example.com", name: "X", variantId: 200 }),
      { signature: "0".repeat(64) },
    );
    expect(response.status).toBe(401);

    const rows = await testEnv.DB.prepare("SELECT * FROM licenses").all();
    expect(rows.results).toHaveLength(0);
    expect(testEnv.EMAIL.send).not.toHaveBeenCalled();
  });

  it("order_created with a valid signature issues, stores, and emails a verifiable token", async () => {
    const response = await postWebhook(
      orderCreatedEvent({ orderId: "order-1", email: "buyer@example.com", name: "Buyer Co", variantId: 200 }),
    );
    expect(response.status).toBe(200);

    const rows = await testEnv.DB.prepare("SELECT * FROM licenses WHERE order_id = ?")
      .bind("order-1")
      .all();
    expect(rows.results).toHaveLength(1);
    expect(rows.results[0].email).toBe("buyer@example.com");
    expect(rows.results[0].tier).toBe("team");
    expect(rows.results[0].revoked).toBe(0);

    expect(testEnv.EMAIL.send).toHaveBeenCalledTimes(1);
    const message = vi.mocked(testEnv.EMAIL.send).mock.calls[0][0] as { to: string; text: string };
    expect(message.to).toBe("buyer@example.com");

    const tokenMatch = message.text.match(/DRY-LICENSE-V1\.[\w-]+\.[\w-]+/);
    expect(tokenMatch).not.toBeNull();
    const { payload, verified } = await decodeAndVerifyToken(tokenMatch![0]);
    expect(verified).toBe(true);
    expect(payload.tier).toBe("team");
    expect(payload.machines).toBe(25);
    expect(payload.expires_unix - payload.issued_unix).toBe(365 * 24 * 60 * 60 + 3 * 24 * 60 * 60);
  });

  it("escapes an HTML-hostile licensee name in the emailed html body", async () => {
    const response = await postWebhook(
      orderCreatedEvent({
        orderId: "order-xss",
        email: "victim@example.com",
        name: "<img src=x onerror=1>",
        variantId: 200,
      }),
    );
    expect(response.status).toBe(200);

    expect(testEnv.EMAIL.send).toHaveBeenCalledTimes(1);
    const message = vi.mocked(testEnv.EMAIL.send).mock.calls[0][0] as { html: string };
    expect(message.html).not.toContain("<img src=x onerror=1>");
    expect(message.html).toContain("&lt;img src=x onerror=1&gt;");
  });

  it("a retried order_created for an already-issued order is a no-op: one D1 row, one email", async () => {
    const event = orderCreatedEvent({
      orderId: "order-retry",
      email: "retry@example.com",
      name: "Retry Co",
      variantId: 200,
    });

    const first = await postWebhook(event);
    expect(first.status).toBe(200);
    const second = await postWebhook(event);
    expect(second.status).toBe(200);
    const secondBody = (await second.json()) as { ok: boolean; duplicate?: boolean };
    expect(secondBody.duplicate).toBe(true);

    const rows = await testEnv.DB.prepare("SELECT * FROM licenses WHERE order_id = ?")
      .bind("order-retry")
      .all();
    expect(rows.results).toHaveLength(1);
    expect(testEnv.EMAIL.send).toHaveBeenCalledTimes(1);
  });

  it("subscription_payment_success maps through the same issuance path", async () => {
    const event = {
      meta: { event_name: "subscription_payment_success" },
      data: {
        id: "invoice-1",
        type: "subscription-invoices",
        attributes: {
          user_email: "renewer@example.com",
          user_name: "Renewer Co",
          variant_id: 100,
          order_id: "order-original",
        },
      },
    };
    const response = await postWebhook(event);
    expect(response.status).toBe(200);

    const rows = await testEnv.DB.prepare("SELECT * FROM licenses WHERE order_id = ?")
      .bind("order-original")
      .all();
    expect(rows.results).toHaveLength(1);
    expect(rows.results[0].tier).toBe("solo");
  });

  it("order_refunded revokes the matching license row", async () => {
    const issue = await postWebhook(
      orderCreatedEvent({ orderId: "order-to-refund", email: "gone@example.com", name: "Gone Co", variantId: 300 }),
    );
    expect(issue.status).toBe(200);

    const refund = await postWebhook(refundEvent("order-to-refund"));
    expect(refund.status).toBe(200);

    const rows = await testEnv.DB.prepare("SELECT revoked FROM licenses WHERE order_id = ?")
      .bind("order-to-refund")
      .all();
    expect(rows.results).toHaveLength(1);
    expect(rows.results[0].revoked).toBe(1);
  });

  it("subscription_expired revokes the matching license row", async () => {
    const issue = await postWebhook(
      orderCreatedEvent({ orderId: "order-to-expire", email: "bye@example.com", name: "Bye Co", variantId: 200 }),
    );
    expect(issue.status).toBe(200);

    const expiredEvent = {
      meta: { event_name: "subscription_expired" },
      data: { id: "sub-1", type: "subscriptions", attributes: { order_id: "order-to-expire" } },
    };
    const expired = await postWebhook(expiredEvent);
    expect(expired.status).toBe(200);

    const rows = await testEnv.DB.prepare("SELECT revoked FROM licenses WHERE order_id = ?")
      .bind("order-to-expire")
      .all();
    expect(rows.results[0].revoked).toBe(1);
  });
});

describe("POST /admin/issue", () => {
  it("rejects requests without a bearer token", async () => {
    const response = await postAdminIssue({ licensee: "Manual Co", email: "m@example.com", tier: "pilot" });
    expect(response.status).toBe(401);
    expect(testEnv.EMAIL.send).not.toHaveBeenCalled();
  });

  it("rejects the wrong bearer token", async () => {
    const response = await postAdminIssue(
      { licensee: "Manual Co", email: "m@example.com", tier: "pilot" },
      "wrong-token",
    );
    expect(response.status).toBe(401);
  });

  it("issues a verifiable token with a valid bearer token", async () => {
    const response = await postAdminIssue(
      { licensee: "Manual Co", email: "m@example.com", tier: "pilot", machines: 5, days: 30 },
      ADMIN_TOKEN,
    );
    expect(response.status).toBe(200);
    const body = (await response.json()) as { ok: boolean; token: string };
    expect(body.ok).toBe(true);

    const { payload, verified } = await decodeAndVerifyToken(body.token);
    expect(verified).toBe(true);
    expect(payload.tier).toBe("pilot");
    expect(payload.machines).toBe(5);
    expect(payload.expires_unix - payload.issued_unix).toBe(30 * 24 * 60 * 60);

    expect(testEnv.EMAIL.send).toHaveBeenCalledTimes(1);
  });
});

describe("unknown routes", () => {
  it("404s", async () => {
    const response = await exports.default.fetch(url("/nope"), { method: "GET" });
    expect(response.status).toBe(404);
  });
});
