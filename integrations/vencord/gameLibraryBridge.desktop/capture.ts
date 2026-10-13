/* SPDX-License-Identifier: GPL-3.0-or-later */
export interface CaptureSource { id: string; name: string; kind: "window" | "screen"; }
export interface CaptureProvider {
    account?: string;
    channel?: string;
    listSources?: () => Promise<unknown>;
}

export function sources(value: unknown): CaptureSource[] {
    if (!Array.isArray(value)) throw new Error("Discord kaynak listesi okunamadı");
    const result: CaptureSource[] = [];
    const seen = new Set<string>();
    for (const source of value.slice(0, 2000)) {
        if (!source || typeof source.id !== "string" || !/^(window|screen):\d+:\d+$/.test(source.id) || source.id.length > 128 || seen.has(source.id)) continue;
        seen.add(source.id);
        result.push({id:source.id,name:String(source.name || (source.id.startsWith("screen:") ? "Ekran" : "Pencere")).toWellFormed().slice(0,128),kind:source.id.startsWith("screen:") ? "screen" : "window"});
        if (result.length === 100) break;
    }
    return result.sort((a,b)=>a.kind.localeCompare(b.kind) || a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
}

export async function bounded<T>(call: () => T | Promise<T>): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
        return await Promise.race([Promise.resolve().then(call),new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error("Discord işlemi zaman aşımına uğradı")),3000);})]);
    } finally { if (timer) clearTimeout(timer); }
}

export class CaptureCatalog {
    private account = "";
    private channel = "";
    private revision = 0;
    private epoch = 0;
    private entries: CaptureSource[] = [];
    private expires = 0;
    private pending = false;
    private opened = false;
    private error = "";

    clear() {
        this.epoch++; this.entries = []; this.pending = false; this.opened = false; this.error = ""; this.expires = 0;
    }

    state(provider: CaptureProvider) {
        if (this.account !== (provider.account ?? "") || this.channel !== (provider.channel ?? "")) {
            this.clear(); this.account = provider.account ?? ""; this.channel = provider.channel ?? "";
        }
        return {sources:this.entries,sourceRevision:this.revision,sourceChannel:this.channel,sourcePending:this.pending,sourceOpen:this.opened,sourceError:this.error};
    }

    async open(provider: CaptureProvider, now = Date.now()) {
        this.state(provider);
        if (this.pending) return;
        if (!provider.account || !provider.channel || !provider.listSources) throw new Error("Kaynak seçmek için Discord'da bir ses odasına bağlan");
        const epoch = ++this.epoch;
        const account = provider.account, channel = provider.channel;
        this.opened = true; this.pending = true; this.entries = []; this.error = "";
        try {
            const entries = sources(await bounded(provider.listSources));
            if (epoch !== this.epoch || account !== provider.account || channel !== provider.channel) return;
            this.entries = entries; this.expires = now + 30000;
            this.revision = this.revision % 2147483646 + 1;
        } catch (error) {
            if (epoch === this.epoch) this.error = error instanceof Error ? error.message : "Kaynak listesi okunamadı";
        } finally { if (epoch === this.epoch) this.pending = false; }
    }

    async resolve(provider: CaptureProvider, revision: unknown, id: unknown, now = Date.now()) {
        this.state(provider);
        const selected = this.entries.find(source=>source.id === id);
        if (!this.opened || this.pending || revision !== this.revision || !selected || now >= this.expires || !provider.listSources) throw new Error("Kaynak seçimi eskidi; listeyi yenile");
        const epoch = this.epoch, account = provider.account, channel = provider.channel;
        const fresh = sources(await bounded(provider.listSources)).find(source=>source.id === selected.id && source.name === selected.name);
        if (!fresh || epoch !== this.epoch || account !== provider.account || channel !== provider.channel) throw new Error("Kaynak veya ses odası değişti; listeyi yenile");
        return fresh;
    }
}
