import { generateKeyPairSync, createPublicKey } from "node:crypto";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, relative, isAbsolute } from "node:path";
import { fileURLToPath } from "node:url";

const [privateFile, publicFile] = process.argv.slice(2);
if (!privateFile || !publicFile) throw new Error("Usage: node scripts/create-update-key.mjs <private-file-outside-repo> <public-file>");
const keyRelative=relative(fileURLToPath(new URL("..",import.meta.url)),resolve(privateFile));
if (!keyRelative.startsWith("..") && !isAbsolute(keyRelative)) throw new Error("Private signing keys must be stored outside the repository");
if (!existsSync(privateFile)) {
    const { privateKey } = generateKeyPairSync("ed25519");
    writeFileSync(privateFile, privateKey.export({ type: "pkcs8", format: "pem" }), { flag: "wx", mode: 0o600 });
}
const jwk = createPublicKey(readFileSync(privateFile)).export({ format: "jwk" });
const publicKey = Buffer.from(jwk.x, "base64url").toString("base64") + "\n";
if (existsSync(publicFile) && readFileSync(publicFile, "utf8").trim() !== publicKey.trim()) throw new Error("Public key differs; refusing to rotate the trusted update key.");
writeFileSync(publicFile, publicKey, { mode: 0o644 });
console.log("Ed25519 release signing key ready. Private key contents are never printed.");
