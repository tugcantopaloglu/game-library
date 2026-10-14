/* SPDX-License-Identifier: GPL-3.0-or-later */
import { readFileSync, statSync } from "fs";
import { createConnection, Socket } from "net";
import { StringDecoder } from "string_decoder";
import { encodeSnapshot } from "./state";
import { configurationPaths, parseConfiguration } from "./transport";

let socket: Socket | undefined;
let retry: ReturnType<typeof setTimeout> | undefined;
let heartbeat: ReturnType<typeof setInterval> | undefined;
let commands: unknown[] = [];
let connected = false;
let directory = "";
let enabled = false;
let generation = 0;
let attempt = 0;
let waiting: ((value: ReturnType<typeof poll>) => void) | undefined;
function notifyCommands() { const resolve=waiting; waiting=undefined; resolve?.(poll()); }

function reconnect() {
    if (!enabled || socket) return;
    try {
        const paths = configurationPaths(directory, process.env.LOCALAPPDATA);
        if (!paths.length) throw new Error("Game Library data directory missing");
        const path = paths[attempt++ % paths.length];
        if (statSync(path).size > 4096) throw new Error("Invalid configuration size");
        const config = parseConfiguration(readFileSync(path, "utf8"));
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
                    if (message.type === "welcome" && message.protocol === 1) { connected = true; generation++; }
                    if (connected && message.type === "command" && commands.length < 16) { commands.push(message); if (waiting) notifyCommands(); }
                } catch { connection.destroy(); }
            }
        });
        connection.on("error", () => {});
        connection.on("timeout", () => connection.destroy());
        connection.on("close", () => {
            if (socket !== connection) return;
            connected = false; socket = undefined; commands = [];
            notifyCommands();
            if (enabled) retry = setTimeout(reconnect, 3000);
        });
    } catch { if (enabled) retry = setTimeout(reconnect, 3000); }
}

export function start(_: unknown, dataDirectory: string) {
    stop(); directory = dataDirectory; attempt = 0; enabled = true; reconnect();
    heartbeat = setInterval(() => { if (connected) socket?.write('{"type":"heartbeat"}\n'); }, 1000);
}

export function poll() {
    const pending = commands; commands = [];
    return { connected, generation, commands: pending };
}
export function connectionState() { return {connected,generation}; }

let avatarJobs=0;
let avatarQueue:Promise<unknown>=Promise.resolve();
export function avatar(_:unknown,address:string) {
    if (avatarJobs>=64) return Promise.resolve(null);
    avatarJobs++;
    const task=avatarQueue.then(()=>fetchAvatar(address));
    avatarQueue=task.catch(()=>null);
    return task.finally(()=>{avatarJobs--;});
}
async function fetchAvatar(address:string) {
    if (!enabled || !connected) return null;
    try {
        const url=new URL(address);
        if (url.protocol!=="https:" || url.hostname!=="cdn.discordapp.com" || url.username || url.password || url.port || !/^\/(avatars|embed\/avatars)\//.test(url.pathname) || address.length>512) return null;
        const {net}=await import("electron");
        const response=await net.fetch(url.href,{credentials:"omit",redirect:"error",signal:AbortSignal.timeout(8000)});
        if (!response.ok || !response.body) return null;
        const reader=response.body.getReader(); const chunks:Buffer[]=[]; let size=0;
        while (true) {const value=await reader.read();if(value.done)break;size+=value.value.byteLength;if(size>65536){await reader.cancel();return null;}chunks.push(Buffer.from(value.value));}
        return Buffer.concat(chunks).toString("base64");
    } catch {return null;}
}

export function waitCommands() {
    if (commands.length) return Promise.resolve(poll());
    if (waiting) notifyCommands();
    return new Promise<ReturnType<typeof poll>>(resolve => { waiting=resolve; });
}

export function snapshot(_: unknown, payload: unknown) {
    if (!connected || !socket || socket.writableLength > 256 * 1024) return false;
    const text = encodeSnapshot(payload, text => Buffer.byteLength(text,"utf8"));
    if (!text || socket.writableLength + Buffer.byteLength(text,"utf8") + 1 > 256 * 1024) return false;
    socket.write(text + "\n");
    return true;
}

export function stop() {
    enabled = false; connected = false; commands = [];
    notifyCommands();
    if (retry) clearTimeout(retry);
    if (heartbeat) clearInterval(heartbeat);
    retry = undefined; heartbeat = undefined;
    const previous = socket; socket = undefined; previous?.destroy();
}
