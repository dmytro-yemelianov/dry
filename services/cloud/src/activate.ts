// GET/POST /activate — the RFC 8628 "verification_uri" page: a minimal,
// self-contained HTML form (no external CSS/JS beyond the Turnstile widget
// itself) that confirms the user_code and collects the account email.

import { approveDevice, resolveUserCode } from "./auth";

const TURNSTILE_SITEVERIFY_URL = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function isDevBypass(env: Env): boolean {
  return env.TURNSTILE_DEV_BYPASS === "1";
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function securityHeadersForPage(env: Env): HeadersInit {
  const csp = isDevBypass(env)
    ? "default-src 'none'; form-action 'self'"
    : "default-src 'none'; script-src https://challenges.cloudflare.com; " +
      "frame-src https://challenges.cloudflare.com; form-action 'self'";
  return {
    "content-type": "text/html; charset=utf-8",
    "content-security-policy": csp,
  };
}

function page(body: string, env: Env, status = 200): Response {
  return new Response(
    `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Activate Dry Cloud</title>
</head>
<body>
${body}
</body>
</html>`,
    { status, headers: securityHeadersForPage(env) },
  );
}

function renderForm(env: Env, opts: { userCode?: string; error?: string } = {}): Response {
  const prefill = opts.userCode ? escapeHtml(opts.userCode) : "";
  const errorHtml = opts.error ? `<p role="alert">${escapeHtml(opts.error)}</p>` : "";
  const turnstileWidget = isDevBypass(env)
    ? ""
    : `<div class="cf-turnstile" data-sitekey="${escapeHtml(env.TURNSTILE_SITE_KEY)}"></div>
<script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>`;

  return page(
    `<h1>Activate your device</h1>
<p>Enter the code shown on your device and your email address.</p>
${errorHtml}
<form method="POST" action="/activate">
<label for="user_code">Device code</label>
<input id="user_code" name="user_code" value="${prefill}" placeholder="XXXX-XXXX" required maxlength="9" autocomplete="off">
<label for="email">Email</label>
<input id="email" name="email" type="email" required>
${turnstileWidget}
<button type="submit">Activate</button>
</form>`,
    env,
  );
}

/** `GET /activate` — optionally pre-filled via `?user_code=`. */
export function handleActivateGet(request: Request, env: Env): Response {
  const userCode = new URL(request.url).searchParams.get("user_code") ?? undefined;
  return renderForm(env, { userCode });
}

async function verifyTurnstile(env: Env, token: string, remoteIp: string | null): Promise<boolean> {
  if (!token) return false;

  const body = new URLSearchParams();
  body.set("secret", env.TURNSTILE_SECRET_KEY);
  body.set("response", token);
  if (remoteIp) body.set("remoteip", remoteIp);

  let response: Response;
  try {
    response = await fetch(TURNSTILE_SITEVERIFY_URL, { method: "POST", body });
  } catch {
    return false;
  }
  if (!response.ok) return false;

  const result = (await response.json()) as { success?: boolean };
  return result.success === true;
}

async function upsertAccount(env: Env, email: string): Promise<string> {
  const existing = await env.DB.prepare("SELECT id FROM accounts WHERE email = ?")
    .bind(email)
    .first<{ id: string }>();
  if (existing) return existing.id;

  const id = crypto.randomUUID();
  await env.DB.prepare("INSERT INTO accounts (id, email) VALUES (?, ?)").bind(id, email).run();
  return id;
}

/** `POST /activate` — Turnstile-verified (unless dev bypass) approval. */
export async function handleActivateSubmit(request: Request, env: Env): Promise<Response> {
  const form = await request.formData();
  const userCodeInput = String(form.get("user_code") ?? "");
  const email = String(form.get("email") ?? "")
    .trim()
    .toLowerCase();
  const turnstileToken = String(form.get("cf-turnstile-response") ?? "");

  if (!isDevBypass(env)) {
    const verified = await verifyTurnstile(env, turnstileToken, request.headers.get("cf-connecting-ip"));
    if (!verified) {
      return page("<h1>Verification failed</h1><p>Please try again.</p>", env, 403);
    }
  }

  if (!EMAIL_PATTERN.test(email)) {
    return renderForm(env, { userCode: userCodeInput, error: "Enter a valid email address." });
  }

  // Resolve (read-only) before touching `accounts` — an invalid or expired
  // code must never have the side effect of upserting an account row.
  const resolved = await resolveUserCode(env, userCodeInput);
  if (resolved === "not_found" || resolved === "expired") {
    return renderForm(env, {
      userCode: userCodeInput,
      error: "That code is invalid or has expired. Request a new one and try again.",
    });
  }

  const accountId = await upsertAccount(env, email);
  await approveDevice(env, resolved, accountId);

  return page("<h1>Device activated</h1><p>You can return to your terminal.</p>", env);
}
