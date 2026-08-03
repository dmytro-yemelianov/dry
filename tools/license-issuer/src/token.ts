// License payload construction and token framing. Byte encoding/signing
// primitives live in ./codec.mjs (shared with scripts/sign.mjs, see that
// file's header) -- this module only assembles the JSON payload fields,
// which MUST match crates/license/src/lib.rs::LicensePayload field-for-field
// (see the Global Constraints in
// docs/superpowers/plans/2026-07-28-commercial-license-product.md).
import { b64url } from "./codec.mjs";

export type Tier = "solo" | "team" | "pilot";

export interface LicensePayload {
  id: string;
  licensee: string;
  email: string;
  tier: Tier;
  machines: number;
  issued: string;
  expires: string;
  issued_unix: number;
  expires_unix: number;
  key_id: string;
}

const ONE_YEAR_SECS = 365 * 24 * 60 * 60;
// Task 6 spec: grants run "1 year + 3 days" -- the extra days are slack for
// payment-processing/email delays around the renewal boundary.
const GRANT_GRACE_SECS = 3 * 24 * 60 * 60;

function isoDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toISOString().slice(0, 10);
}

export interface BuildPayloadOptions {
  id: string;
  licensee: string;
  email: string;
  tier: Tier;
  keyId: string;
  nowUnix: number;
  machines: number;
  /** Term length in seconds; defaults to the standard 1-year + 3-day grant. */
  termSecs?: number;
}

export function buildPayload(opts: BuildPayloadOptions): LicensePayload {
  const issuedUnix = opts.nowUnix;
  const expiresUnix = issuedUnix + (opts.termSecs ?? ONE_YEAR_SECS + GRANT_GRACE_SECS);
  return {
    id: opts.id,
    licensee: opts.licensee,
    email: opts.email,
    tier: opts.tier,
    machines: opts.machines,
    issued: isoDate(issuedUnix),
    expires: isoDate(expiresUnix),
    issued_unix: issuedUnix,
    expires_unix: expiresUnix,
    key_id: opts.keyId,
  };
}

/** base64url(JSON(payload)) -- the middle segment of the token. */
export function encodePayload(payload: LicensePayload): string {
  return b64url(new TextEncoder().encode(JSON.stringify(payload)));
}

/** Frames the final `DRY-LICENSE-V1.<payload>.<sig>` token string. */
export function formatToken(payloadB64: string, sigB64: string): string {
  return `DRY-LICENSE-V1.${payloadB64}.${sigB64}`;
}
