/* SPDX-License-Identifier: GPL-3.0-or-later */
import { readFileSync } from "fs";
import { createConnection, Socket } from "net";
import { join, resolve } from "path";
import { StringDecoder } from "string_decoder";

let socket: Socket | undefined;
let retry: ReturnType<typeof setTimeout> | undefined;
let heartbeat: ReturnType<typeof setInterval> | undefined;
let commands: unknown[] = [];
let connected = false;
let directory = "";
let enabled = false;

function reconnect() {
    if (!enabled || socket) return;
    try {
        const path = resolve(directory || join(process.env.LOCALAPPDATA || "", "GameLibrary"), "integrations/vencord/bridge.json");
        const text = readFileSync(path, "utf8");
        if (text.length > 4096) throw new Error("Invalid configuration size");
        const config = JSON.parse(text);
        if (config.protocol !== 1 || !Number.isInteger(config.port) || config.port < 1 || config.port > 65535 || !/^[a-f0-9]{64}$/.test(config.token)) throw new Error("Invalid configuration");
        let buffer = "";
        const decoder = new StringDecoder("utf8");
        const connection = createConnection({ host: "127.0.0.1", port: config.port });
        socket = connection;
        connection.setTimeout(8000);
        connection.on("connect", () => connection.write(JSON.stringify({ type: "hello", protocol: 1, token: config.token }) + "\n"));
        connection.on("data", bytes => {
            if (socket !== connection) return;
            buffer += decoder.write(bytes);
            if (Buffer.byteLength(buffer, "utf8") > 256 * 1024) { connection.destroy(); return; }
            let newline: number;
            while ((newline = buffer.indexOf("\n")) !== -1) {
                const line = buffer.slice(0, newline); buffer = buffer.slice(newline + 1);
                try {
                    const message = JSON.parse(line);
                    if (message.type === "welcome" && message.protocol === 1) connected = true;
                    if (connected && message.type === "command" && commands.length < 16) commands.push(message);
                } catch { connection.destroy(); }
            }
        });
        connection.on("error", () => {});
        connection.on("timeout", () => connection.destroy());
        connection.on("close", () => {
            if (socket !== connection) return;
            connected = false; socket = undefined; commands = [];
            if (enabled) retry = setTimeout(reconnect, 3000);
        });
    } catch { if (enabled) retry = setTimeout(reconnect, 3000); }
}

export function start(_: unknown, dataDirectory: string) {
    stop(); directory = dataDirectory; enabled = true; reconnect();
    heartbeat = setInterval(() => { if (connected) socket?.write('{"type":"heartbeat"}\n'); }, 1000);
}

export function poll() {
    const pending = commands; commands = [];
    return { connected, commands: pending };
}

export function snapshot(_: unknown, payload: unknown) {
    const text = JSON.stringify(payload);
    if (connected && text.length < 256 * 1024) socket?.write(text + "\n");
}

export function stop() {
    enabled = false; connected = false; commands = [];
    if (retry) clearTimeout(retry);
    if (heartbeat) clearInterval(heartbeat);
    retry = undefined; heartbeat = undefined;
    const previous = socket; socket = undefined; previous?.destroy();
}
