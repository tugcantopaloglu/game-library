import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, readdir, writeFile, copyFile } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const source = process.argv[2];
if (!source) throw new Error("Usage: node scripts/package-vencord-bridge.mjs <official Vencord checkout>");
const checkout = resolve(source);
const git = (...args) => execFileSync("git", ["-C", checkout, ...args], { encoding: "utf8" }).trim();
if (git("remote", "get-url", "origin") !== "https://github.com/Vendicated/Vencord.git") {
    throw new Error("Expected the official Vencord checkout.");
}
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const plugin = join(root, "integrations/vencord/gameLibraryBridge.desktop");
const pluginSources = {};
for (const name of (await readdir(plugin)).filter(name => name.endsWith(".ts")).sort()) {
    const bytes = await readFile(join(plugin, name));
    const builtSource = await readFile(join(checkout, "src/userplugins/gameLibraryBridge.desktop", name));
    if (!bytes.equals(builtSource)) throw new Error(`Build uses a different plugin source: ${name}`);
    pluginSources[name] = sha256(bytes);
}
const index = await readFile(join(plugin, "index.ts"), "utf8");
const build = index.match(/bridgeBuild:\s*"([0-9.]+)"/)?.[1];
if (!build) throw new Error("Bridge build is missing.");
const names = ["patcher.js", "preload.js", "renderer.js", "renderer.css"];
const prepared = [];
for (const name of names) {
    const bytes = await readFile(join(checkout, "dist", name));
    if (!bytes.length || bytes.length > 8 * 1024 * 1024) throw new Error(`Invalid bundle file: ${name}`);
    if (name === "renderer.js" && (!bytes.includes("GameLibraryBridge") || !bytes.includes(`bridgeBuild:"${build}"`))) {
        throw new Error("The compiled renderer does not contain the current bridge.");
    }
    prepared.push({ name, bytes });
}
const destination = join(root, "integrations/vencord/prebuilt");
await mkdir(destination, { recursive: true });
for (const { name, bytes } of prepared) await writeFile(join(destination, `${name}.gz`), gzipSync(bytes, { level: 9 }));
for (const name of ["patcher.js.LEGAL.txt", "renderer.js.LEGAL.txt"]) {
    await copyFile(join(checkout, "dist", name), join(destination, name));
}
await copyFile(join(checkout, "LICENSE"), join(destination, "LICENSE"));
const manifest = {
    protocol: 1,
    build,
    vencord_revision: git("rev-parse", "HEAD"),
    plugin_sources: pluginSources,
    files: prepared.map(({ name, bytes }) => ({ name, size: bytes.length, sha256: sha256(bytes) }))
};
await writeFile(join(destination, "bundle.json"), JSON.stringify(manifest, null, 2) + "\n");
process.stdout.write(`Packaged bridge ${build} from Vencord ${manifest.vencord_revision}.\n`);
