/* SPDX-License-Identifier: GPL-3.0-or-later */
import type { LiveStream } from "./stream";
import type { CaptureSource } from "./capture";
import { validId } from "./social";

function code(value: unknown): string { return typeof value === "function" ? Function.prototype.toString.call(value) : ""; }

/** Discord encodes the active stream itself; RTC sessions can be absent or in another store. */
export function currentStreamKey(active: any, account: unknown): string {
    if (!validId(account) || active?.ownerId !== account || !validId(active?.channelId)) return "";
    if (active.streamType === "guild" && validId(active.guildId)) return ["guild", active.guildId, active.channelId, account].join(":");
    if (active.streamType === "call" && active.guildId == null) return ["call", active.channelId, account].join(":");
    return "";
}

/** Use the client's own entitlement check for the same preset/resolution/FPS tuple. */
export function qualityAdapter(check: unknown, preset: unknown) {
    const body = code(check);
    if (!Number.isInteger(preset) || !["canStreamWithSettings", "guildPremiumTier", "canStreamQuality"].every(part => body.includes(part))) return undefined;
    return (resolution: number, fps: number, user: unknown, guildTier: unknown, channel: unknown) => {
        try { return (check as Function)(preset, resolution, fps, user, guildTier, channel) === true; } catch { return false; }
    };
}

export function mediaAdapter(media: any, preset: unknown) {
    const body = code(media?.setGoLiveSource);
    if (!Number.isInteger(preset) || !body.includes("MEDIA_ENGINE_SET_GO_LIVE_SOURCE") || !body.includes("qualityOptions")) return undefined;
    return (source: NonNullable<LiveStream["source"]>, context: LiveStream["context"], sound = source.desktopSource.soundshareId != null) => media.setGoLiveSource({
        desktopSettings:{sourceId:source.desktopSource.id,sound},
        qualityOptions:{preset,resolution:source.quality.resolution,frameRate:source.quality.frameRate},context
    });
}

export function startAdapter(start: any) {
    const body = code(start);
    if (typeof start !== "function" || !["STREAM_START","streamType","guildId","channelId"].every(part=>body.includes(part))) return undefined;
    return (guild: string | null, channel: string, source: CaptureSource, sound = false) => start(guild,channel,{sourceId:source.id,sourceName:source.name,sound});
}

export function stopAdapter(stop: any) {
    const body = code(stop);
    if (!body.includes("STREAM_STOP") || !body.includes("streamKey")) return undefined;
    return (key: string) => stop(key,false,true);
}
