/* SPDX-License-Identifier: GPL-3.0-or-later */
import { definePluginSettings, Settings, SettingsStore } from "@api/Settings";
import { checkForUpdates, update } from "@utils/updater";
import definePlugin, { OptionType, PluginNative } from "@utils/types";
import { findByProps } from "@webpack";
import { ApplicationStreamingStore, ChannelActionCreators, ChannelStore, GuildStore, MediaEngineStore, PermissionStore, PermissionsBits, PresenceStore, ReadStateStore, RelationshipStore, SelectedChannelStore, UserStore, VoiceStateStore } from "@webpack/common";
import { favoriteIds, lastMessageAt, validId, visibleFriends } from "./social";
import { SnapshotCache, StoreListeners, Section } from "./state";

const Native = VencordNative.pluginHelpers.GameLibraryBridge as PluginNative<typeof import("./native")>;
const settings = definePluginSettings({
    dataDirectory: { type: OptionType.STRING, description: "Game Library data directory. Leave empty for the default.", default: "" },
    selectedFriends: { type: OptionType.STRING, description: "Selected friend IDs separated by commas; empty displays recent friends.", default: "" },
    favoriteFriends: { type: OptionType.STRING, description: "Favorite friends synchronized with Game Library.", default: "" },
    favoriteRooms: { type: OptionType.STRING, description: "Followed voice channels synchronized with Game Library.", default: "" }
});
let timer: ReturnType<typeof setInterval> | undefined;
let busy = false;
let result = "";
let cache = new SnapshotCache();
const listeners = new StoreListeners();
let moduleCache: ReturnType<typeof resolveActions> | undefined;
let lastResolve = 0;
let lastFallback = 0;
let catalogQuery: string | null = null;
let catalogAccount: string | undefined;
const invalidateSettings = () => cache.invalidate("friends", "rooms", "controls");

function resolveActions() {
    return {
        voice: findByProps("toggleSelfMute", "toggleSelfDeaf") as any,
        channel: findByProps("selectVoiceChannel", "selectChannel") as any,
        streaming: findByProps("createStream", "stopStream") as any
    };
}

function actions() {
    if (!moduleCache || Date.now() - lastResolve > 30000) {
        moduleCache = resolveActions();
        lastResolve = Date.now();
    }
    return moduleCache;
}

function friendSnapshot() {
    const selected = settings.store.selectedFriends.split(",").map(id => id.trim()).filter(id => /^\d{15,22}$/.test(id));
    const favorites = favoriteIds(settings.store.favoriteFriends);
    const recencyAvailable = typeof ChannelStore.getDMFromUserId === "function" && typeof ReadStateStore?.lastMessageId === "function";
    const recent = RelationshipStore.getFriendIDs().filter(id => validId(id) && (selected.length === 0 || selected.includes(id))).slice(0, 2000).map(id => {
        const channel = recencyAvailable ? ChannelStore.getDMFromUserId(id) : undefined;
        return { id, lastMessageAt: channel ? lastMessageAt(ReadStateStore.lastMessageId(channel)) : null };
    });
    const friends = visibleFriends(recent, favorites).map(({id, lastMessageAt}) => {
        const user = UserStore.getUser(id);
        return { id, lastMessageAt, name: (user?.globalName || user?.username || id).slice(0,64), status: PresenceStore.getStatus(id) || "unknown", activity: PresenceStore.getActivities(id).slice(0, 2).map(activity => activity.name).join(", ").slice(0,128) };
    });
    return { friends, accountId: UserStore.getCurrentUser()?.id, favoriteIds: favorites, recentConversations: recencyAvailable };
}

function roomSnapshot() {
    const account = UserStore.getCurrentUser()?.id;
    if (catalogAccount !== account) { catalogAccount = account; catalogQuery = null; }
    const favoriteRoomIds = favoriteIds(settings.store.favoriteRooms).slice(0, 50);
    const counts = new Map<string, number>();
    for (const states of Object.values(VoiceStateStore.getAllVoiceStates())) {
        for (const state of Object.values(states)) if (state.channelId) counts.set(state.channelId, (counts.get(state.channelId) || 0) + 1);
    }
    const describe = (id: string) => {
        const channel = ChannelStore.getChannel(id);
        const available = !!channel && [2,13].includes(channel.type) && typeof PermissionStore?.can === "function" && PermissionStore.can(PermissionsBits.VIEW_CHANNEL, channel);
        return { id, name: available ? (channel.name || "Ses odası").slice(0,128) : "Oda kullanılamıyor", guild: available ? (GuildStore.getGuild(channel.guild_id ?? "")?.name || "").slice(0,128) : "", members: available ? counts.get(id) || 0 : null, available, canJoin: available && PermissionStore.can(PermissionsBits.CONNECT, channel) };
    };
    const active = [...counts.keys()].filter(validId).filter(id => !favoriteRoomIds.includes(id)).map(describe).filter(room => room.available).slice(0,50);
    const rooms = [...favoriteRoomIds.map(describe), ...active];
    let roomCatalog: ReturnType<typeof describe>[] = [];
    let roomCatalogMore = false;
    if (catalogQuery !== null && typeof ChannelStore.getChannelIds === "function") {
        const query = catalogQuery.trim().toLowerCase();
        for (const id of ChannelStore.getChannelIds().slice(0,20000)) {
            const channel = ChannelStore.getChannel(id);
            if (!validId(id) || !channel || ![2,13].includes(channel.type)) continue;
            const room = describe(id);
            if (!room.available || (query && !`${room.guild} ${room.name}`.toLowerCase().includes(query))) continue;
            if (roomCatalog.length >= 100) { roomCatalogMore = true; break; }
            roomCatalog.push(room);
        }
        roomCatalog.sort((a,b) => a.guild.localeCompare(b.guild) || a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
    }
    return { rooms, favoriteRoomIds, roomCatalog, roomCatalogMore, roomCatalogQuery: catalogQuery };
}

function controlSnapshot() {
    const modules = actions();
    return { status: "Yerel Vencord bağlantısı açık", result, voiceChannel: SelectedChannelStore.getVoiceChannelId(),
        muted: MediaEngineStore.isSelfMute(), deafened: MediaEngineStore.isSelfDeaf(),
        stream: ApplicationStreamingStore.getCurrentUserActiveStream(),
        autoUpdate: Settings.autoUpdate,
        capabilities: {
            recent_conversations: typeof ChannelStore?.getDMFromUserId === "function" && typeof ReadStateStore?.lastMessageId === "function",
            favorite_friends: true, favorite_rooms: typeof PermissionStore?.can === "function", room_catalog: typeof ChannelStore?.getChannelIds === "function" && typeof PermissionStore?.can === "function",
            open_dm: typeof ChannelActionCreators.openPrivateChannel === "function",
            mute: typeof modules.voice?.toggleSelfMute === "function", deafen: typeof modules.voice?.toggleSelfDeaf === "function", join_voice: typeof modules.channel?.selectVoiceChannel === "function",
            stream_stop: typeof modules.streaming?.stopStream === "function", stream_start: false, stream_source: false, stream_quality: false, stream_fps: false, stream_audio: false,
            vencord_update: true, auto_update: true
        }
    };
}

function bindStores() {
    let complete = true;
    const bindings: [any, Section[]][] = [
        [UserStore, ["friends", "rooms", "controls"]],
        [RelationshipStore, ["friends"]], [PresenceStore, ["friends"]], [ReadStateStore, ["friends"]],
        [ChannelStore, ["friends", "rooms", "controls"]], [GuildStore, ["rooms"]],
        [VoiceStateStore, ["rooms", "controls"]], [PermissionStore, ["rooms", "controls"]], [MediaEngineStore, ["controls"]],
        [SelectedChannelStore, ["controls"]], [ApplicationStreamingStore, ["controls"]]
    ];
    for (const [store, sections] of bindings) complete = listeners.bind(store, () => cache.invalidate(...sections)) && complete;
    return complete;
}

async function command(message: any) {
    const modules = actions();
    const id = String(message.data?.id || "");
    switch (message.action) {
        case "favorite_friends":
            if (!Array.isArray(message.data?.ids) || message.data.ids.length > 200 || !message.data.ids.every(validId)) throw new Error("Arkadaş favorileri geçersiz");
            settings.store.favoriteFriends = favoriteIds(message.data.ids.join(",")).join(","); break;
        case "favorite_rooms":
            if (!Array.isArray(message.data?.ids) || message.data.ids.length > 50 || !message.data.ids.every(validId)) throw new Error("Oda favorileri geçersiz");
            settings.store.favoriteRooms = favoriteIds(message.data.ids.join(",")).slice(0,50).join(","); break;
        case "room_catalog":
            if (message.data?.query !== null && (typeof message.data?.query !== "string" || [...message.data.query].length > 128)) throw new Error("Oda araması geçersiz");
            catalogQuery = message.data.query;
            cache.invalidate("rooms"); break;
        case "mute": modules.voice.toggleSelfMute(); break;
        case "deafen": modules.voice.toggleSelfDeaf(); break;
        case "join_voice":
            if (!validId(id) || ![2,13].includes(ChannelStore.getChannel(id)?.type) || !PermissionStore.can(PermissionsBits.VIEW_CHANNEL, ChannelStore.getChannel(id)) || !PermissionStore.can(PermissionsBits.CONNECT, ChannelStore.getChannel(id))) throw new Error("Ses kanalı erişilemiyor");
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
        cache = new SnapshotCache();
        moduleCache = undefined;
        lastFallback = 0;
        catalogQuery = null;
        catalogAccount = undefined;
        SettingsStore.addChangeListener("autoUpdate", invalidateSettings);
        SettingsStore.addPrefixChangeListener("plugins.GameLibraryBridge", invalidateSettings);
        await Native.start(settings.store.dataDirectory);
        timer = setInterval(async () => {
            if (busy) return; busy = true;
            try {
                const pending = await Native.poll();
                if (pending.connected) {
                    const complete = bindStores();
                    if (!complete && Date.now() - lastFallback >= 4000) { cache.invalidate(); lastFallback = Date.now(); }
                    for (const message of pending.commands) { try { await command(message); } catch (error) { result = error instanceof Error ? error.message : "İşlem başarısız"; } cache.invalidate("controls"); }
                    const packet = cache.packet(pending.generation, section => section === "friends" ? friendSnapshot() : section === "rooms" ? roomSnapshot() : controlSnapshot());
                    if (!await Native.snapshot(packet)) cache.invalidate();
                }
            } catch { result = "Discord sürümündeki bir yetenek kullanılamıyor"; cache.invalidate(); }
            finally { busy = false; }
        }, 1000);
    },
    stop() {
        if (timer) clearInterval(timer);
        timer = undefined;
        listeners.clear();
        SettingsStore.removeChangeListener("autoUpdate", invalidateSettings);
        SettingsStore.removePrefixChangeListener("plugins.GameLibraryBridge", invalidateSettings);
        cache = new SnapshotCache();
        moduleCache = undefined;
        catalogQuery = null;
        catalogAccount = undefined;
        Native.stop();
    }
});
