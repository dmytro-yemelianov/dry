// Token/key/device-code generation, hashing, and comparison.
//
// Formats (binding — see docs/superpowers/specs/2026-07-28-dry-cloud-registry-design.md
// and the R1 task brief's Global Constraints):
//   - access tokens: `dry_at_<43 base64url chars>`  (32 random bytes)
//   - API keys:      `dry_key_<43 base64url chars>` (32 random bytes)
//   - user codes:    `XXXX-XXXX` drawn from BCDFGHJKLMNPQRSTVWXZ23456789
//     (vowels and visually-ambiguous characters removed so codes are easy to
//     read aloud and type back in).
//
// Only SHA-256 hashes of tokens/keys are ever persisted (schema.sql `tokens.hash`).

const TOKEN_BYTE_LENGTH = 32; // -> 43 base64url characters, no padding
const USER_CODE_ALPHABET = "BCDFGHJKLMNPQRSTVWXZ23456789";
const USER_CODE_LENGTH = 8; // rendered as XXXX-XXXX

/** Cryptographically random bytes, base64url-encoded without padding. */
export function randomBase64Url(byteLength: number): string {
  const bytes = new Uint8Array(byteLength);
  crypto.getRandomValues(bytes);
  return base64UrlEncode(bytes);
}

function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

/** Opaque RFC 8628 `device_code` — never shown to the user, only polled with. */
export function generateDeviceCode(): string {
  return randomBase64Url(TOKEN_BYTE_LENGTH);
}

/** Human-entered RFC 8628 `user_code`, formatted `XXXX-XXXX`. */
export function generateUserCode(): string {
  const bytes = new Uint8Array(USER_CODE_LENGTH);
  crypto.getRandomValues(bytes);
  let code = "";
  for (let i = 0; i < USER_CODE_LENGTH; i++) {
    code += USER_CODE_ALPHABET[bytes[i] % USER_CODE_ALPHABET.length];
  }
  return `${code.slice(0, 4)}-${code.slice(4)}`;
}

/**
 * Normalizes a user-typed code for lookup: uppercases, strips whitespace and
 * dashes, validates every character is in the user-code alphabet and the
 * length is right, then re-inserts the canonical dash. Returns null for
 * anything that can't possibly be a valid code (defense against KV-key
 * injection via odd input, and forgiving of copy/paste noise).
 */
export function normalizeUserCode(input: string): string | null {
  const cleaned = input.trim().toUpperCase().replace(/[\s-]/g, "");
  if (cleaned.length !== USER_CODE_LENGTH) return null;
  for (const char of cleaned) {
    if (!USER_CODE_ALPHABET.includes(char)) return null;
  }
  return `${cleaned.slice(0, 4)}-${cleaned.slice(4)}`;
}

export function generateAccessToken(): string {
  return `dry_at_${randomBase64Url(TOKEN_BYTE_LENGTH)}`;
}

export function generateApiKey(): string {
  return `dry_key_${randomBase64Url(TOKEN_BYTE_LENGTH)}`;
}

/** Lowercase hex SHA-256 digest of a UTF-8 string. */
export async function sha256Hex(input: string): Promise<string> {
  const data = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return bytesToHex(new Uint8Array(digest));
}

function bytesToHex(bytes: Uint8Array): string {
  let hex = "";
  for (const byte of bytes) hex += byte.toString(16).padStart(2, "0");
  return hex;
}

function hexToBytes(hex: string): Uint8Array {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

/**
 * Constant-time comparison of two hex-encoded SHA-256 hashes. Both inputs are
 * expected to be 64 hex characters (32 bytes); mismatched lengths are
 * rejected up front (equal-length buffers are required by
 * `crypto.subtle.timingSafeEqual`) without a variable-time string compare —
 * a length mismatch is not secret-dependent since digests are fixed-size.
 */
export function timingSafeEqualHex(a: string, b: string): boolean {
  if (a.length !== b.length) return false;
  return crypto.subtle.timingSafeEqual(hexToBytes(a), hexToBytes(b));
}
