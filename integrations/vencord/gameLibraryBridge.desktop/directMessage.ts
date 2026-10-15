/* SPDX-License-Identifier: GPL-3.0-or-later */
import { unreadCount, validId } from "./social";

type Fields = Record<string, unknown>;
export const CHAT_MESSAGE_LIMIT = 30;
export const CHAT_HISTORY_LIMIT = 150;
export const CHAT_BYTE_LIMIT = 96 * 1024;

export interface ChatProvider {
    account?: string;
    friendIds(): string[];
    dmChannelId(userId: string): unknown;
    channel(channelId: string): unknown;
    messages(channelId: string): unknown;
    openDm?(userId: string): unknown;
    fetch?(channelId: string, limit: number, before?: string): unknown;
    unread?(channelId: string): number;
    send?(channelId: string, content: string, nonce: string): unknown;
    nonce?(): string;
    selectedChannel?(): unknown;
    focus?(): unknown;
}

function object(value: unknown): Fields | undefined {
    return value !== null && typeof value === "object" && !Array.isArray(value) ? value as Fields : undefined;
}
// Vencord lazy webpack exports proxy a function even when the module is an object.
function actionsObject(value: unknown): Fields | undefined {
    return typeof value === "function" ? value as unknown as Fields : object(value);
}
function code(value: unknown): string { return typeof value === "function" ? Function.prototype.toString.call(value) : ""; }
function read<T>(operation: () => T, fallback: T): T { try { return operation(); } catch { return fallback; } }
function text(value: unknown, maximum: number) {
    return typeof value === "string" ? value.slice(0, maximum).replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, "").toWellFormed() : "";
}

/** Current public Discord client takes an options object, never a bare user ID. */
export function directMessageAdapter(value: unknown): ChatProvider["openDm"] {
    const actions = actionsObject(value), method = actions?.openPrivateChannel;
    if (typeof method !== "function" || !/\brecipientIds\b/.test(code(method))) return undefined;
    return userId => {
        if (!validId(userId)) throw new Error("Discord arkadaşı geçersiz");
        return method.call(actions, { recipientIds: [userId], joinCall: false, joinCallVideo: false, navigateToChannel: true });
    };
}

/** Preserve Discord's own fetch/send paths, moderation and account credentials. */
export function chatMessageAdapters(value: unknown): Pick<ChatProvider, "fetch" | "send"> {
    const actions = actionsObject(value), fetch = actions?.fetchMessages, send = actions?.sendMessage;
    const fetchSupported = typeof fetch === "function";
    const sendSupported = typeof send === "function";
    return {
        fetch: fetchSupported ? (channelId, limit, before) => {
            if (!validId(channelId) || limit !== CHAT_MESSAGE_LIMIT || before !== undefined && !validId(before)) throw new Error("Sohbet yenileme isteği geçersiz");
            return (fetch as Function).call(actions, { channelId, limit, skipLocalFetch: true, ...(before ? { before } : {}) });
        } : undefined,
        send: sendSupported ? (channelId, content, nonce) => {
            if (!validId(channelId) || !validId(nonce) || !validContent(content)) throw new Error("Sohbet gönderme isteği geçersiz");
            return (send as Function).call(actions, channelId, { content, invalidEmojis: [], validNonShortcutEmojis: [], tts: false }, false, { nonce, allowedMentions: { parse: [], replied_user: false } });
        } : undefined
    };
}

/** Public native window wrapper uses restore(windowId), focus(windowId, force). */
export function discordFocusAdapter(value: unknown): ChatProvider["focus"] {
    const native = object(value), focus = native?.focus, restore = native?.restore;
    if (typeof focus !== "function" || typeof restore !== "function" || !code(focus).includes(".window.focus(") || !code(restore).includes(".window.restore(")) return undefined;
    return () => { restore.call(native, undefined); focus.call(native, undefined, false); };
}

export function validContent(value: unknown): value is string {
    return typeof value === "string" && value.length <= 4000 && [...value].length <= 2000 && value.trim().length > 0 && !/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(value) && value === value.toWellFormed();
}

export function oneToOneChannel(value: unknown, userId: string, account: string): string | null {
    const channel = object(value);
    return validId(account) && validId(userId) && account !== userId && validId(channel?.id) && channel.type === 1 && channel.guild_id == null && Array.isArray(channel.recipients) && channel.recipients.length === 1 && channel.recipients[0] === userId ? channel.id : null;
}

function friend(provider: ChatProvider, userId: string) {
    return validId(provider.account) && validId(userId) && userId !== provider.account && read(() => provider.friendIds().slice(0, 2000).includes(userId), false);
}
function cache(provider: ChatProvider, channelId: string) { return object(read(() => provider.messages(channelId), undefined)); }
function rawMessages(provider: ChatProvider, channelId: string): Fields[] {
    const entries = cache(provider, channelId)?._array;
    return Array.isArray(entries) ? entries.slice(-1000).map(object).filter((value): value is Fields => !!value) : [];
}

/** Signed Discord attachment links stay scoped to this message's channel and ID. */
export function attachmentUrl(value: unknown, channelId: string, attachmentId: string): string | null {
    if (typeof value !== "string" || value.length > 2048 || !validId(channelId) || !validId(attachmentId)) return null;
    try {
        const url = new URL(value);
        return url.protocol === "https:" && ["cdn.discordapp.com", "media.discordapp.net"].includes(url.hostname)
            && !url.username && !url.password && !url.port && !url.hash
            && url.pathname.startsWith(`/attachments/${channelId}/${attachmentId}/`) ? url.href : null;
    } catch { return null; }
}
function chatAttachments(value: unknown, channelId: string) {
    if (!Array.isArray(value)) return [];
    return value.slice(0, 4).flatMap(value => {
        const item = object(value), id = item?.id;
        if (!validId(id)) return [];
        const url = attachmentUrl(item?.url, channelId, id);
        if (!url) return [];
        const size = typeof item?.size === "number" && Number.isSafeInteger(item.size) && item.size >= 0 ? item.size : 0;
        const image = ["image/png", "image/jpeg"].includes(String(item?.content_type)) && size > 0 && size <= 8 * 1024 * 1024;
        return [{ id, name: text(item?.filename, 128) || "Dosya", url, size, image }];
    });
}

export function chatMessages(provider: ChatProvider, channelId: string, userId: string, account: string, now = Date.now(), limit = CHAT_MESSAGE_LIMIT) {
    if (provider.account !== account || !friend(provider, userId) || oneToOneChannel(read(() => provider.channel(channelId), undefined), userId, account) !== channelId) return [];
    const rows = rawMessages(provider, channelId).filter(message => message.channel_id === channelId && validId(message.id) && message.state === "SENT" && [account, userId].includes(String(object(message.author)?.id))).sort((a, b) => BigInt(a.id as string) < BigInt(b.id as string) ? -1 : 1).slice(-Math.min(CHAT_HISTORY_LIMIT, limit)).map(message => {
        const author = object(message.author)!;
        const timestamp = Number((BigInt(message.id as string) >> 22n) + 1420070400000n);
        return { id: message.id as string, author: text(author.globalName ?? author.global_name ?? author.username ?? author.id, 64), content: text(message.content, 4000), timestamp: Math.min(timestamp, now), own: author.id === account, attachments: chatAttachments(message.attachments, channelId) };
    }).sort((a, b) => a.id === b.id ? 0 : BigInt(a.id) < BigInt(b.id) ? -1 : 1);
    const bounded: typeof rows = [];
    let bytes = 2;
    for (const row of rows.reverse()) {
        const size = new TextEncoder().encode(JSON.stringify(row)).byteLength + 1;
        if (bytes + size > CHAT_BYTE_LIMIT) break;
        bounded.push(row); bytes += size;
    }
    return bounded.reverse();
}

type Selection = { account: string; userId: string; channelId: string; revision: number };
type Sending = { selection: Selection; nonce: string; content: string; deadline: number };

/** All actions require an explicit UI command; Promise completion cannot change another chat. */
export class ChatControls {
    private selection: Selection | undefined;
    private revision = 1;
    private opening = false;
    private openingDeadline = 0;
    private fetching: { selection: Selection; deadline: number; before?: string } | undefined;
    private history: ReturnType<typeof chatMessages> = [];
    private historyLimit = CHAT_MESSAGE_LIMIT;
    private hasOlder = true;
    private sending: Sending | undefined;
    private lastSend: { key: string; until: number } | undefined;
    private sentNonce: string | null = null;
    private status = "Sohbet için bir arkadaş seç.";
    private changed = false;

    clear() {
        this.revision++; this.selection = undefined; this.opening = false; this.fetching = undefined; this.sending = undefined; this.lastSend = undefined;
        this.sentNonce = null; this.history = []; this.historyLimit = CHAT_MESSAGE_LIMIT; this.hasOlder = true;
        this.status = "Sohbet için bir arkadaş seç."; this.changed = true;
    }
    private active(provider: ChatProvider, selection: Selection) {
        return this.selection === selection && provider.account === selection.account && friend(provider, selection.userId);
    }
    private validSelection(provider: ChatProvider): Selection | undefined {
        const selection = this.selection;
        if (!selection) return undefined;
        if (!this.active(provider, selection) || selection.channelId && oneToOneChannel(read(() => provider.channel(selection.channelId), undefined), selection.userId, selection.account) !== selection.channelId) {
            this.clear(); this.status = "Discord hesabı veya sohbet erişimi değişti. Arkadaşını tekrar seç.";
            return undefined;
        }
        return selection;
    }
    state(provider: ChatProvider, now = Date.now()) {
        const selected = this.validSelection(provider);
        const ready = selected?.channelId ? cache(provider, selected.channelId)?.ready === true : false;
        if (selected?.channelId) {
            // Discord remains authoritative for edits and deletions; do not retain removed text.
            this.history = chatMessages(provider, selected.channelId, selected.userId, selected.account, now, this.historyLimit);
        }
        return {
            accountId: selected?.account ?? (validId(provider.account) ? provider.account : ""), revision: selected?.revision ?? this.revision,
            userId: selected?.userId ?? "", channelId: selected?.channelId ?? "",
            messages: selected?.channelId ? this.history : [],
            canLoadOlder: !!selected?.channelId && !!this.history.length && this.hasOlder && this.historyLimit < CHAT_HISTORY_LIMIT && !this.fetching && typeof provider.fetch === "function",
            historyLoading: !!this.fetching?.before,
            unread: selected?.channelId ? unreadCount(read(() => provider.unread?.(selected.channelId), 0)) : 0,
            loading: this.opening || !!this.fetching,
            pending: !!this.sending,
            sentNonce: selected ? this.sentNonce : null,
            canOpenDiscord: !!selected && typeof provider.openDm === "function",
            canSend: !!selected?.channelId && ready && typeof provider.send === "function" && !this.sending && !this.opening,
            status: this.status
        };
    }
    private assertScope(data: Fields, selected: Selection) {
        for (const [key, expected] of [["userId", selected.userId], ["channelId", selected.channelId], ["accountId", selected.account], ["revision", selected.revision]] as const) {
            if (data[key] !== undefined && data[key] !== expected) throw new Error("Sohbet değişti; güncel sohbeti seç");
        }
    }
    private beginFetch(provider: ChatProvider, selected: Selection, now: number, before?: string) {
        if (!this.active(provider, selected) || !selected.channelId || typeof provider.fetch !== "function") return;
        const fetching = { selection: selected, deadline: now + 20000, before };
        this.fetching = fetching; this.status = before ? "Önceki mesajlar yükleniyor…" : "Mesajlar yükleniyor…"; this.changed = true;
        try {
            Promise.resolve(provider.fetch(selected.channelId, CHAT_MESSAGE_LIMIT, before)).then(value => {
                if (this.active(provider, selected) && this.fetching === fetching) {
                    this.fetching = undefined;
                    if (before && value !== false && cache(provider, selected.channelId)?.error !== true) {
                        const rows = chatMessages(provider, selected.channelId, selected.userId, selected.account, now, CHAT_HISTORY_LIMIT);
                        const older = rows.filter(message => BigInt(message.id) < BigInt(before));
                        this.hasOlder = older.length >= CHAT_MESSAGE_LIMIT;
                        this.historyLimit = Math.min(CHAT_HISTORY_LIMIT, this.historyLimit + CHAT_MESSAGE_LIMIT);
                    }
                    this.status = value === false || cache(provider, selected.channelId)?.error === true ? "Mesajlar yüklenemedi. Yenile düğmesini kullan." : "Birebir sohbet açık.";
                    this.changed = true;
                }
            }, () => {
                if (this.active(provider, selected) && this.fetching === fetching) { this.fetching = undefined; this.status = "Mesajlar yüklenemedi. Yenile düğmesini kullan."; this.changed = true; }
            });
        } catch { this.fetching = undefined; this.status = "Mesajlar yüklenemedi. Yenile düğmesini kullan."; this.changed = true; }
    }
    async execute(provider: ChatProvider, action: string, value: unknown, now = Date.now()) {
        const data = object(value);
        if (!data) throw new Error("Sohbet isteği geçersiz");
        if (action === "chat_close") {
            const selected = this.validSelection(provider);
            if (selected) this.assertScope(data, selected);
            this.clear(); return;
        }
        if (action === "chat_open" || action === "open_dm") {
            const userId = data.userId ?? data.id;
            if (typeof userId !== "string" || !friend(provider, userId)) throw new Error("Seçili arkadaş bulunamadı");
            if (data.accountId !== undefined && data.accountId !== provider.account || data.revision !== undefined && data.revision !== this.revision) throw new Error("Discord hesabı veya sohbet değişti; arkadaşını tekrar seç");
            this.clear();
            const selected: Selection = { account: provider.account!, userId, channelId: "", revision: this.revision };
            this.selection = selected; this.status = "Birebir sohbet açılıyor…"; this.opening = true; this.openingDeadline = now + 30000;
            const finish = (channelId: unknown) => {
                if (!this.active(provider, selected)) return;
                if (!validId(channelId) || oneToOneChannel(read(() => provider.channel(channelId), undefined), userId, selected.account) !== channelId) {
                    this.clear(); this.status = "Discord seçili arkadaşın birebir sohbetini doğrulamadı."; return;
                }
                selected.channelId = channelId; this.opening = false; this.status = "Birebir sohbet açık."; this.changed = true;
                if (action === "open_dm") {
                    if (read(() => provider.selectedChannel?.(), undefined) !== channelId) this.status = "Sohbet hazır; Discord'da seçili kanal doğrulanamadı.";
                    else try { provider.focus?.(); } catch { this.status = "Sohbet açık; Discord penceresini elle öne getir."; }
                }
                if (cache(provider, channelId)?.ready !== true) this.beginFetch(provider, selected, now);
            };
            const existing = read(() => provider.dmChannelId(userId), undefined);
            if (action === "chat_open" && validId(existing) && oneToOneChannel(read(() => provider.channel(existing), undefined), userId, selected.account) === existing) { finish(existing); return; }
            if (typeof provider.openDm !== "function") { this.clear(); this.status = "Birebir sohbet bu Discord sürümünde açılamıyor."; throw new Error(this.status); }
            try {
                Promise.resolve(provider.openDm(userId)).then(finish, () => {
                    if (this.active(provider, selected)) { this.clear(); this.status = "Birebir sohbet açılamadı. Discord bağlantısını kontrol et."; }
                });
            } catch { this.clear(); this.status = "Birebir sohbet açılamadı. Discord bağlantısını kontrol et."; throw new Error(this.status); }
            return;
        }
        const selected = this.validSelection(provider);
        if (!selected?.channelId) throw new Error("Önce bir arkadaşın birebir sohbetini aç");
        this.assertScope(data, selected);
        if (action === "chat_older") {
            const state = this.state(provider, now);
            if (!state.canLoadOlder || this.sending) throw new Error("Önceki mesajlar şu anda yüklenemiyor");
            this.beginFetch(provider, selected, now, this.history[0].id); return;
        }
        if (action === "chat_refresh") { if (!this.fetching) this.beginFetch(provider, selected, now); return; }
        if (action !== "chat_send") throw new Error("Desteklenmeyen sohbet işlemi");
        if (data.userId !== selected.userId || data.channelId !== selected.channelId || !validContent(data.content)) throw new Error("Mesaj veya hedef sohbet geçersiz (en fazla 2000 karakter)");
        if (!this.state(provider, now).canSend || typeof provider.send !== "function") throw new Error("Sohbet hazır değil veya önceki mesajın sonucu bekleniyor");
        const key = `${selected.account}:${selected.userId}:${selected.channelId}:${data.content}`;
        if (this.lastSend?.key === key && now < this.lastSend.until) throw new Error("Aynı mesaj zaten gönderildi; güncel sonucu bekle");
        const random = new Uint32Array(1);
        const nonce = provider.nonce ? provider.nonce() : (globalThis.crypto.getRandomValues(random), (((BigInt(now) - 1420070400000n) << 22n) | BigInt(random[0] & 0x3fffff)).toString());
        if (!validId(nonce)) throw new Error("Mesaj kimliği oluşturulamadı");
        const pending: Sending = { selection: selected, nonce, content: data.content, deadline: now + 30000 };
        this.sending = pending; this.lastSend = { key, until: now + 30000 }; this.status = "Mesaj gönderiliyor; Discord onayı bekleniyor."; this.changed = true;
        const fail = () => {
            if (this.active(provider, selected) && this.sending === pending) { this.sending = undefined; this.status = "Discord mesajı doğrulamadı. Otomatik tekrar gönderilmedi."; this.changed = true; }
        };
        try { Promise.resolve(provider.send(selected.channelId, data.content, nonce)).catch(fail); }
        catch { fail(); throw new Error(this.status); }
        this.observe(provider, now);
    }
    observe(provider: ChatProvider, now = Date.now()): boolean {
        const selected = this.validSelection(provider);
        if (this.opening && now >= this.openingDeadline) { this.clear(); this.status = "Birebir sohbet açma işlemi zaman aşımına uğradı."; }
        if (this.fetching && now >= this.fetching.deadline) { this.fetching = undefined; this.status = "Mesajlar yüklenemedi. Yenile düğmesini kullan."; this.changed = true; }
        const pending = this.sending;
        if (selected?.channelId && pending?.selection === selected) {
            const observed = rawMessages(provider, selected.channelId).find(message => message.channel_id === selected.channelId && message.nonce === pending.nonce && object(message.author)?.id === selected.account && message.content === pending.content);
            if (observed?.state === "SENT" && validId(observed.id)) {
                this.sentNonce = pending.nonce;
                this.sending = undefined; this.status = "Mesaj Discord tarafından doğrulandı."; this.changed = true;
                if (this.lastSend) this.lastSend.until = now + 2000;
            } else if (observed?.state === "SEND_FAILED" || now >= pending.deadline) {
                this.sending = undefined; this.status = "Discord mesajı doğrulamadı. Otomatik tekrar gönderilmedi."; this.changed = true;
            }
        }
        const changed = this.changed; this.changed = false; return changed;
    }
}
