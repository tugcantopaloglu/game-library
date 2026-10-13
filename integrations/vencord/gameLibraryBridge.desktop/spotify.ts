/* SPDX-License-Identifier: GPL-3.0-or-later */
import { validId } from "./social";

type RecordValue = Record<string, unknown>;

/** Only documented, token-free store accessors belong in this interface. */
export interface SpotifyStoreReader {
    hasConnectedAccount?(): boolean;
    getTrack?(): unknown;
    getActivity?(): unknown;
    getSyncingWith?(): unknown;
    wasAutoPaused?(): boolean;
}

export interface SpotifyProvider {
    account?: string;
    store?: SpotifyStoreReader;
    friendIds(): string[];
    activities(userId: string): unknown[];
    openExternal?(uri: string): void | Promise<void>;
    openProfile?(userId: string): void | Promise<void>;
    /** Supply only an adapter whose client action contract was independently verified. */
    join?(userId: string, activity: RecordValue): void | Promise<void>;
}

export interface SpotifyTrackView {
    id: string;
    title: string;
    artist: string;
    album: string;
    uri: string;
    url: string;
    start: number | null;
    end: number | null;
}

export function validTrackId(value: unknown): value is string {
    return typeof value === "string" && /^[A-Za-z0-9]{22}$/.test(value);
}

/**
 * Discord public web asset web.b19183c9af96925c.js, module 533207, 2026-10-13.
 * This is the native Listen Along button wrapper, including device launch and
 * Premium checks. Never substitute its lower-level ACTIVITY_SYNC dispatcher.
 * A changed/minified client contract disables this adapter until reverified.
 */
export function listenAlongAdapter(value: unknown): SpotifyProvider["join"] {
    if (typeof value !== "function" || value.length !== 3) return undefined;
    const source = Function.prototype.toString.call(value);
    if (!/^async function\s+\w+\(e,t,d\)\{/.test(source) || ![
        "let{hasSpotifyAccount:c,activity:u,user:_}=e;",
        "null!=u&&null!=u.sync_id",
        "await (0,s.dM)(),await (0,s.G8)().catch",
        "(0,r.openModalLazy)(async()=>",
        "Promise.reject(e)",
        "a.OH(u,_.id),(0,l.A)(t,_,u,d)"
    ].every(part => source.includes(part))) return undefined;
    return (userId, activity) => {
        if (!validId(userId) || !spotifyActivity(activity) || typeof object(activity.party)?.id !== "string" || !String(object(activity.party)?.id).startsWith("spotify:")) throw new Error("Spotify birlikte dinleme etkinliği geçersiz");
        return value({ hasSpotifyAccount: true, activity, user: { id: userId } }, "user_activity_sync", undefined);
    };
}

function object(value: unknown): RecordValue | undefined {
    return value !== null && typeof value === "object" && !Array.isArray(value) ? value as RecordValue : undefined;
}

function label(value: unknown, limit = 192): string {
    return typeof value === "string" ? value.slice(0, limit).replace(/[\u0000-\u001f\u007f]/g, " ") : "";
}

function timestamp(value: unknown): number | null {
    return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? value : null;
}

function view(id: string, title: unknown, artist: unknown, album: unknown, start: number | null = null, end: number | null = null): SpotifyTrackView {
    return { id, title: label(title), artist: label(artist), album: label(album), uri: `spotify:track:${id}`, url: `https://open.spotify.com/track/${id}`, start, end };
}

/** Discord listening activity: Spotify itself must supply a public track ID. */
export function spotifyActivity(value: unknown, now = Date.now()): SpotifyTrackView | null {
    const activity = object(value);
    const metadata = object(activity?.metadata);
    if (!activity || activity.name !== "Spotify" || activity.type !== 2 || !validTrackId(activity.sync_id) || (metadata?.type != null && metadata.type !== "track")) return null;
    const times = object(activity.timestamps);
    const start = timestamp(times?.start), end = timestamp(times?.end);
    // Old presence must never advertise a currently available listening session.
    if (start === null || end === null || start > now + 60000 || end <= now || end <= start || end - start > 24 * 60 * 60 * 1000) return null;
    return view(activity.sync_id, activity.details, activity.state, object(activity.assets)?.large_text, start, end);
}

/** getTrack returns metadata only; socket/device accessors containing tokens are never read. */
export function spotifyTrack(value: unknown): SpotifyTrackView | null {
    const track = object(value);
    if (!track || !validTrackId(track.id) || track.isLocal === true || (track.type != null && track.type !== "track")) return null;
    const artists = Array.isArray(track.artists) ? track.artists.slice(0, 8).map(artist => label(object(artist)?.name, 64)).filter(Boolean).join(", ") : "";
    return view(track.id, track.name, artists, object(track.album)?.name);
}

function read<T>(operation: (() => T) | undefined, fallback: T): T {
    try { return operation ? operation() : fallback; } catch { return fallback; }
}

function peer(provider: SpotifyProvider, userId: string, now: number) {
    if (!validId(userId) || !validId(provider.account) || userId === provider.account || !read(() => provider.friendIds().slice(0, 2000).includes(userId), false)) return null;
    const activities = read(() => provider.activities(userId), []);
    if (!Array.isArray(activities)) return null;
    for (const value of activities.slice(0, 16)) {
        const track = spotifyActivity(value, now);
        if (track) return { userId, track, activity: object(value)! };
    }
    return null;
}

function current(provider: SpotifyProvider): SpotifyTrackView | null {
    const store = provider.store;
    if (!validId(provider.account) || !store || read(() => store.hasConnectedAccount?.(), undefined) === false) return null;
    return spotifyTrack(read(() => store.getTrack?.(), undefined));
}

function syncingWith(provider: SpotifyProvider): string | null {
    const synced = object(read(() => provider.store?.getSyncingWith?.(), undefined));
    return validId(synced?.userId) ? synced.userId : null;
}

export class SpotifyControls {
    private pending: { account: string; userId: string; deadline: number } | undefined;
    private result = "";
    private changed = false;

    get waiting() { return !!this.pending; }

    clear() { this.pending = undefined; this.result = ""; this.changed = false; }

    state(provider: SpotifyProvider, now = Date.now()) {
        const store = provider.store;
        const account = validId(provider.account);
        const connection = account ? read(() => store?.hasConnectedAccount?.(), undefined) : undefined;
        const connected = typeof connection === "boolean" ? connection : null;
        const track = current(provider);
        const observed = syncingWith(provider);
        const confirmed = account && connected === true && typeof store?.getSyncingWith === "function";
        const canJoin = confirmed && typeof provider.join === "function" && !this.waiting;
        const ids = read(() => provider.friendIds(), []);
        const friends = Array.isArray(ids) && account ? [...new Set(ids.slice(0, 2000).filter(validId))].slice(0, 200).flatMap(id => {
            const target = peer(provider, id, now);
            return target ? [{ userId: id, track: target.track, canOpenTrack: typeof provider.openExternal === "function", canOpenProfile: typeof provider.openProfile === "function", canListenAlong: canJoin }] : [];
        }) : [];
        return {
            connected, current: track, friends, syncingWith: confirmed ? observed : null,
            canOpenTrack: typeof provider.openExternal === "function" && !!track,
            canOpenProfile: account && typeof provider.openProfile === "function",
            canListenAlong: canJoin,
            autoPaused: account && read(() => store?.wasAutoPaused?.(), false) === true,
            waiting: this.waiting, result: this.result,
            status: !account ? "Discord oturumu bekleniyor" : connected === false ? "Discord > Bağlantılar bölümünden Spotify hesabını bağla." : connected === null ? "Spotify bağlantı durumu bu Discord sürümünde okunamıyor." : observed && confirmed ? "Birlikte dinleme Discord tarafından doğrulandı." : "Spotify bağlı. Birlikte dinleme için Spotify Premium gerekir.",
            listenAlongHint: canJoin ? "Spotify Premium gerekir; Discord sonucu doğrulanana kadar bağlantı beklenir." : "Arkadaşın Discord profilinde Birlikte dinle düğmesini kullan. Spotify Premium gerekir."
        };
    }

    async execute(provider: SpotifyProvider, action: string, data: unknown, now = Date.now()) {
        const payload = object(data);
        if (!payload || !validId(provider.account)) throw new Error("Discord oturumu veya Spotify işlemi geçersiz");
        if (action === "spotify_track") {
            if (!validTrackId(payload.trackId) || typeof provider.openExternal !== "function") throw new Error("Spotify parçası açılamıyor");
            const selected = payload.userId === undefined ? current(provider) : typeof payload.userId === "string" ? peer(provider, payload.userId, now)?.track : null;
            if (selected?.id !== payload.trackId) throw new Error("Spotify parçası değişti; güncel parçayı seç");
            try { await provider.openExternal(`spotify:track:${selected.id}`); }
            catch { throw new Error("Spotify açılamadı; uygulama kurulumunu kontrol et"); }
            this.result = "Parça Spotify'a iletildi.";
            return;
        }
        if (typeof payload.userId !== "string") throw new Error("Spotify arkadaşı geçersiz");
        const target = peer(provider, payload.userId, now);
        if (!target) throw new Error("Arkadaşın güncel Spotify etkinliği bulunamadı");
        if (action === "spotify_profile") {
            if (typeof provider.openProfile !== "function") throw new Error("Discord profili bu sürümde açılamıyor");
            try { await provider.openProfile(target.userId); }
            catch { throw new Error("Discord profili açılamadı"); }
            this.result = "Discord profilinde Birlikte dinle düğmesini kullan. Spotify Premium gerekir.";
            return;
        }
        if (action !== "spotify_join") throw new Error("Desteklenmeyen Spotify işlemi");
        if (this.waiting) throw new Error("Birlikte dinleme sonucu bekleniyor");
        if (!this.state(provider, now).canListenAlong || typeof provider.join !== "function") throw new Error("Birlikte dinleme bu Discord sürümünde doğrulanamıyor; arkadaş profilindeki düğmeyi kullan");
        this.pending = { account: provider.account, userId: target.userId, deadline: now + 45000 };
        const pending = this.pending;
        this.result = "Birlikte dinleme isteği iletildi; Discord onayı bekleniyor.";
        try {
            // Native Spotify launch may wait 30 seconds. Keep bridge polling and
            // authoritative readback alive while that client operation resolves.
            const operation = provider.join(target.userId, target.activity);
            Promise.resolve(operation).catch(() => {
                if (this.pending === pending) {
                    this.pending = undefined;
                    this.result = "Discord birlikte dinleme isteğini kabul etmedi. Spotify bağlantını ve Premium hesabını kontrol et.";
                    this.changed = true;
                }
            });
        }
        catch { this.pending = undefined; this.result = "Discord birlikte dinleme isteğini kabul etmedi. Spotify bağlantını ve Premium hesabını kontrol et."; throw new Error(this.result); }
        this.observe(provider, now);
    }

    observe(provider: SpotifyProvider, now = Date.now()): boolean {
        const pending = this.pending;
        if (!pending) { const changed = this.changed; this.changed = false; return changed; }
        if (provider.account !== pending.account || read(() => provider.store?.hasConnectedAccount?.(), false) !== true) {
            this.pending = undefined;
            this.result = "Discord veya Spotify oturumu değişti; birlikte dinleme doğrulanamadı.";
        } else if (syncingWith(provider) === pending.userId) {
            this.pending = undefined;
            this.result = "Birlikte dinleme Discord tarafından doğrulandı.";
        } else if (now >= pending.deadline) {
            this.pending = undefined;
            this.result = "Discord birlikte dinlemeyi doğrulamadı. Arkadaş profilinden Spotify bağlantısını ve Premium hesabını kontrol et.";
        } else return false;
        return true;
    }
}
