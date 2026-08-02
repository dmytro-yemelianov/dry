// Ed25519 WebCrypto signing for license tokens. Delegates the actual byte
// encoding/signing to ./codec.mjs -- the SAME module scripts/sign.mjs uses
// -- so the Worker and the JS fixture-signing script cannot drift, and both
// are cross-checked against crates/license/src/lib.rs by
// crates/license/tests/cross_stack.rs.
import { b64url, importSigningKey, signBytes } from "./codec.mjs";
import { encodePayload, formatToken, type LicensePayload } from "./token";

/** Signs a license payload and returns the complete `DRY-LICENSE-V1...` token. */
export async function signPayload(payload: LicensePayload, signingKeyPkcs8Base64: string): Promise<string> {
  const signingKey = await importSigningKey(signingKeyPkcs8Base64);
  const payloadB64 = encodePayload(payload);
  const sigBytes = await signBytes(new TextEncoder().encode(payloadB64), signingKey);
  return formatToken(payloadB64, b64url(sigBytes));
}
