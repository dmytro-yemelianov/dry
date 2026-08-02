// Shared Ed25519 signing + base64url codec -- the SINGLE source of truth
// for how a license token's bytes are produced. Both `scripts/sign.mjs`
// (Task 2's cross-stack fixture generator) and the Worker's `src/sign.ts`
// import this file, so the two can never drift out of byte-for-byte
// agreement with each other, or with the Rust verifier in
// crates/license/src/lib.rs (see crates/license/tests/cross_stack.rs).
//
// Deliberately plain JS (no TypeScript syntax): Node loads it directly via
// `node scripts/sign.mjs` with no build step, and the Worker's esbuild
// bundle picks it up unchanged. Uses only the standard WebCrypto global
// (`crypto.subtle`), available unflagged in both Node >=20 and workerd --
// nothing here needs a `node:crypto` import or the `nodejs_compat` flag.

/** base64url, no padding. */
export function b64url(bytes) {
  const buf = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  let binary = "";
  for (const byte of buf) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

/** Imports a PKCS8-encoded Ed25519 signing key (base64, as produced by keygen.mjs). */
export async function importSigningKey(signingKeyPkcs8Base64) {
  const der = Uint8Array.from(atob(signingKeyPkcs8Base64), (c) => c.charCodeAt(0));
  return crypto.subtle.importKey("pkcs8", der, "Ed25519", false, ["sign"]);
}

/** Ed25519-signs `messageBytes` with an imported signing key. */
export async function signBytes(messageBytes, signingKey) {
  const sig = await crypto.subtle.sign("Ed25519", signingKey, messageBytes);
  return new Uint8Array(sig);
}
