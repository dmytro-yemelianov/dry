// tools/license-issuer/scripts/sign.mjs <key-file> <payload-json-file>
//
// Signs a license payload with the JS/WebCrypto path -- used to produce
// crates/license/tests/fixtures/js-signed-team.token, proving RFC 8032
// cross-compatibility with the Rust verifier before the Worker exists.
// Shares its byte encoding/signing with the Worker via ../src/codec.mjs
// (Task 6) so the fixture generator and the Worker cannot drift apart --
// see that file's header for the full rationale.
import { readFileSync } from 'node:fs';
import { b64url, importSigningKey, signBytes } from '../src/codec.mjs';

const key = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const payload = readFileSync(process.argv[3], 'utf8').trim();

const signingKey = await importSigningKey(key.signing_key_pkcs8_b64);
const p = b64url(new TextEncoder().encode(payload));
const sig = await signBytes(new TextEncoder().encode(p), signingKey);
console.log(`DRY-LICENSE-V1.${p}.${b64url(sig)}`);
