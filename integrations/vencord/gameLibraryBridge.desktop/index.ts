/* SPDX-License-Identifier: GPL-3.0-or-later */
import { definePluginSettings, Settings } from "@api/Settings";
import { checkForUpdates, update } from "@utils/updater";
import definePlugin, { OptionType, PluginNative } from "@utils/types";
import { findByProps } from "@webpack";
import { ApplicationStreamingStore, ChannelActionCreators, ChannelStore, GuildStore, MediaEngineStore, PresenceStore, ReadStateStore, RelationshipStore, SelectedChannelStore, UserStore, VoiceStateStore } from "@webpack/common";
import { favoriteIds, lastMessageAt, validId, visibleFriends } from "./social";

const Native = VencordNative.pluginHelpers.GameLibraryBridge as PluginNative<typeof import("./native")>;
const settings = definePluginSettings({
    dataDirectory: { type: OptionType.STRING, description: "Game Library data directory. Leave empty for the default.", default: "" },
    selectedFriends: { type: OptionType.STRING, description: "Selected friend IDs separated by commas; empty displays recent friends.", default: "" },
    favoriteFriends: { type: OptionType.STRING, description: "Favorite friends synchronized with Game Library.", default: "" }
});
let timer: ReturnType<typeof setInterval> | undefined;
let busy = false;
let result = "";

function actions() {
    return {
        voice: findByProps("toggleSelfMute", "toggleSelfDeaf") as any,
        channel: findByProps("selectVoiceChannel", "selectChannel") as any,
        streaming: findByProps("createStream", "stopStream") as any
    };
}

function snapshot() {
    const modules = actions();
    const selected = settings.store.selectedFriends.split(",").map(id => id.trim()).filter(id => /^\d{15,22}$/.test(id));
    const favorites = favoriteIds(settings.store.favoriteFriends);
    const recencyAvailable = typeof ChannelStore.getDMFromUserId === "function" && typeof ReadStateStore.lastMessageId === "function";
    const recent = RelationshipStore.getFriendIDs().filter(id => validId(id) && (selected.length === 0 || selected.includes(id))).slice(0, 2000).map(id => {
        const channel = recencyAvailable ? ChannelStore.getDMFromUserId(id) : undefined;
        return { id, lastMessageAt: channel ? lastMessageAt(ReadStateStore.lastMessageId(channel)) : null };
    });
    const friends = visibleFriends(recent, favorites).map(({id, lastMessageAt}) => {
        const user = UserStore.getUser(id);
        return { id, lastMessageAt, name: (user?.globalName || user?.username || id).slice(0,64), status: PresenceStore.getStatus(id) || "unknown", activity: PresenceStore.getActivities(id).slice(0, 2).map(activity => activity.name).join(", ").slice(0,128) };
    });
    const rooms = Object.entries(VoiceStateStore.getAllVoiceStates()).flatMap(([guildId, states]) => {
        const counts = new Map<string, number>();
        Object.values(states).forEach(state => { if (state.channelId) counts.set(state.channelId, (counts.get(state.channelId) || 0) + 1); });
        return [...counts].map(([id, members]) => ({ id, name: ChannelStore.getChannel(id)?.name || id, guild: GuildStore.getGuild(guildId)?.name || "", members }));
    }).slice(0, 50);
    return { type: "snapshot", protocol: 1, version: 1, status: "Yerel Vencord bağlantısı açık", result,
        friends, rooms, accountId: UserStore.getCurrentUser()?.id, favoriteIds: favorites, voiceChannel: SelectedChannelStore.getVoiceChannelId(),
        muted: MediaEngineStore.isSelfMute(), deafened: MediaEngineStore.isSelfDeaf(),
        stream: ApplicationStreamingStore.getCurrentUserActiveStream(),
        autoUpdate: Settings.autoUpdate,
        capabilities: { recent_conversations: recencyAvailable, favorite_friends: true, open_dm: typeof ChannelActionCreators.openPrivateChannel === "function", mute: typeof modules.voice?.toggleSelfMute === "function", deafen: typeof modules.voice?.toggleSelfDeaf === "function", join_voice: typeof modules.channel?.selectVoiceChannel === "function", stream_stop: typeof modules.streaming?.stopStream === "function", stream_start: false, stream_source: false, stream_quality: false, stream_fps: false, stream_audio: false, vencord_update: true, auto_update: true }
    };
}

async function command(message: any) {
    const modules = actions();
    const id = String(message.data?.id || "");
    switch (message.action) {
        case "favorite_friends":
            if (!Array.isArray(message.data?.ids) || message.data.ids.length > 200 || !message.data.ids.every(validId)) throw new Error("Arkadaş favorileri geçersiz");
            settings.store.favoriteFriends = favoriteIds(message.data.ids.join(",")).join(","); break;
        case "mute": modules.voice.toggleSelfMute(); break;
        case "deafen": modules.voice.toggleSelfDeaf(); break;
        case "join_voice":
            if (!/^\d{15,22}$/.test(id) || ![2,13].includes(ChannelStore.getChannel(id)?.type)) throw new Error("Ses kanalı erişilemiyor");
            modules.channel.selectVoiceChannel(id); break;
        case "open_dm":
            if (!RelationshipStore.getFriendIDs().includes(id)) throw new Error("Seçili arkadaş bulunamadı");
            ChannelActionCreators.openPrivateChannel(id); break;
        case "stream_stop": {
            const stream = ApplicationStreamingStore.getCurrentUserActiveStream() as any;
            if (!stream?.streamKey) throw new Error("Aktif yayın bulunamadı");
            modules.streaming.stopStream(stream.streamKey); break;
        }
        case "auto_update": Settings.autoUpdate = message.data?.enabled === true; break;
        case "vencord_update":
            if (await checkForUpdates()) await update();
            result = "Vencord güncellendi; etkinleşmesi için Discord'u yeniden aç."; return;
        default: throw new Error("Desteklenmeyen köprü işlemi");
    }
    result = "İşlem Discord'a iletildi; güncel durum bağlantıdan okunuyor.";
}

export default definePlugin({
    name: "GameLibraryBridge", description: "Local Game Library connection with bounded messages, selected friends and version-gated controls.",
    authors: [{ name: "Game Library", id: 0n }], settings,
    async start() {
        await Native.start(settings.store.dataDirectory);
        timer = setInterval(async () => {
            if (busy) return; busy = true;
            try {
                const pending = await Native.poll();
                if (pending.connected) {
                    for (const message of pending.commands) { try { await command(message); } catch (error) { result = error instanceof Error ? error.message : "İşlem başarısız"; } }
                    await Native.snapshot(snapshot());
                }
            } catch { result = "Discord sürümündeki bir yetenek kullanılamıyor"; }
            finally { busy = false; }
        }, 1000);
    },
    stop() { if (timer) clearInterval(timer); timer = undefined; Native.stop(); }
});
