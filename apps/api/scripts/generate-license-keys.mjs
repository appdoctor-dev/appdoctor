// Generates the Ed25519 key pair for signing desktop entitlements.
// The private key is an API secret; the public key is compiled into the desktop app.
import { generateKeyPairSync } from "node:crypto";

const { privateKey, publicKey } = generateKeyPairSync("ed25519");
const secret = privateKey.export({ type: "pkcs8", format: "der" }).toString("base64");
const publicRaw = publicKey.export({ format: "jwk" }).x;

console.log(`LICENSE_SIGNING_KEY=${secret}`);
console.log(`\n# Public key for the desktop app (base64url, raw 32 bytes):\n# ${publicRaw}`);
