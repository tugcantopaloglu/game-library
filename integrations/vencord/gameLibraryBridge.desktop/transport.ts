/* SPDX-License-Identifier: GPL-3.0-or-later */
import { join, resolve } from "path";

// A configured directory can belong to a packaged launcher. Try the ordinary
// profile as well, so starting the standalone exe does not strand the bridge.
export function configurationPaths(directory: string, localAppData: string | undefined): string[] {
    const roots = [directory, localAppData ? join(localAppData, "GameLibrary") : ""].filter(Boolean);
    return [...new Set(roots.map(root => resolve(root, "integrations/vencord/bridge.json")))];
}

export function parseConfiguration(text: string) {
    if (text.length > 4096) throw new Error("Invalid configuration size");
    const config = JSON.parse(text);
    if (config.protocol !== 1 || !Number.isInteger(config.port) || config.port < 1 || config.port > 65535 || !/^[a-f0-9]{64}$/.test(config.token)) throw new Error("Invalid configuration");
    return {port: config.port as number, token: config.token as string};
}
