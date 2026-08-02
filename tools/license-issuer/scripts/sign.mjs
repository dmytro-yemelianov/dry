// tools/license-issuer/scripts/sign.mjs <key-file> <payload-json-file>
import { webcrypto } from 'node:crypto';
import { readFileSync } from 'node:fs';
const key = JSON.parse(readFileSync(process.argv[2], 'utf8'));
const payload = readFileSync(process.argv[3], 'utf8').trim();
const b64url = (buf) => Buffer.from(buf).toString('base64url');
const sk = await webcrypto.subtle.importKey(
  'pkcs8', Buffer.from(key.signing_key_pkcs8_b64, 'base64'), 'Ed25519', false, ['sign']);
const p = b64url(payload);
const sig = await webcrypto.subtle.sign('Ed25519', sk, Buffer.from(p));
console.log(`DRY-LICENSE-V1.${p}.${b64url(sig)}`);
