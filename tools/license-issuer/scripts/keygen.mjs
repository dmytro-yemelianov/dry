// Generates an Ed25519 keypair for license signing.
// PRODUCTION USE: run once, store signing_key as a Cloudflare Worker secret +
// offline backup; paste verifying_key_hex + key_id into crates/license/src/lib.rs.
// NEVER commit a production signing key.
import { webcrypto } from 'node:crypto';

const keyId = process.argv[2] ?? `key-${new Date().toISOString().slice(0, 10)}`;
const pair = await webcrypto.subtle.generateKey('Ed25519', true, ['sign', 'verify']);
const rawPub = Buffer.from(await webcrypto.subtle.exportKey('raw', pair.publicKey));
const pkcs8 = Buffer.from(await webcrypto.subtle.exportKey('pkcs8', pair.privateKey));
console.log(JSON.stringify({
  key_id: keyId,
  verifying_key_hex: rawPub.toString('hex'),
  signing_key_pkcs8_b64: pkcs8.toString('base64'),
}, null, 2));
