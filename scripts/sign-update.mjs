import { readFileSync, writeFileSync, statSync } from "node:fs";
import { createHash, createPrivateKey, createPublicKey, sign } from "node:crypto";
import { pathToFileURL } from "node:url";

export function signedManifest(bytes, version, privatePem, trustedPublicKey) {
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version)) throw new Error("Invalid stable version");
if (bytes.length < 2 || bytes.length > 80 * 1024 * 1024 || bytes.subarray(0, 2).toString() !== "MZ") throw new Error("Invalid release executable");
const key = createPrivateKey(privatePem);
if (key.asymmetricKeyType !== "ed25519") throw new Error("An Ed25519 signing key is required");
const jwk = createPublicKey(key).export({format:"jwk"});
if (Buffer.from(jwk.x,"base64url").toString("base64") !== trustedPublicKey.trim()) throw new Error("Signing key does not match the application's trusted public key");
const manifest = {
    version,
    url: `https://github.com/tugcantopaloglu/game-library/releases/download/v${version}/game-library-native.exe`,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    size: bytes.length,
    minimum_bridge_protocol: 1,
    signature: ""
};
const payload = `${manifest.version}\n${manifest.url}\n${manifest.sha256}\n${manifest.size}\n${manifest.minimum_bridge_protocol}`;
manifest.signature = sign(null, Buffer.from(payload), key).toString("base64");
return manifest;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
const [binary, version, privateKeyFile, output] = process.argv.slice(2);
if (!binary || !version || !privateKeyFile || !output) {
    throw new Error("Usage: node scripts/sign-update.mjs <exe> <version> <private-key-pem> <update.json>");
}
const cargo = readFileSync(new URL("../apps/native/Cargo.toml",import.meta.url),"utf8");
if (cargo.match(/^version = "([^"]+)"/m)?.[1] !== version) throw new Error("Release version differs from native Cargo.toml");
if (statSync(binary).size > 80 * 1024 * 1024) throw new Error("Executable exceeds the update limit");
const privatePem = privateKeyFile === "-" ? process.env.UPDATE_SIGNING_KEY : readFileSync(privateKeyFile);
if (!privatePem) throw new Error("UPDATE_SIGNING_KEY secret is missing");
const manifest = signedManifest(readFileSync(binary),version,privatePem,readFileSync(new URL("../release/update-public-key.txt",import.meta.url),"utf8"));
writeFileSync(output, JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
console.log(`Signed update ${version}: ${output}`);
}
