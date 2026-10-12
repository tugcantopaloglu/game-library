import { readFileSync, writeFileSync, statSync } from "node:fs";
import { createHash, createPrivateKey, sign } from "node:crypto";

const [binary, version, privateKeyFile, output] = process.argv.slice(2);
if (!binary || !version || !privateKeyFile || !output || !/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error("Usage: node scripts/sign-update.mjs <exe> <version> <private-key-pem> <update.json>");
}
const bytes = readFileSync(binary);
if (bytes.length === 0 || bytes.length > 80 * 1024 * 1024 || bytes.subarray(0, 2).toString() !== "MZ") throw new Error("Invalid release executable");
const manifest = {
    version,
    url: `https://github.com/tugcantopaloglu/game-library/releases/download/v${version}/game-library-native.exe`,
    sha256: createHash("sha256").update(bytes).digest("hex"),
    size: statSync(binary).size,
    minimum_bridge_protocol: 1,
    signature: ""
};
const payload = `${manifest.version}\n${manifest.url}\n${manifest.sha256}\n${manifest.size}\n${manifest.minimum_bridge_protocol}`;
const key = createPrivateKey(readFileSync(privateKeyFile));
if (key.asymmetricKeyType !== "ed25519") throw new Error("An Ed25519 signing key is required");
manifest.signature = sign(null, Buffer.from(payload), key).toString("base64");
writeFileSync(output, JSON.stringify(manifest, null, 2) + "\n", { flag: "wx" });
console.log(`Signed update ${version}: ${output}`);
