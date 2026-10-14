/* SPDX-License-Identifier: GPL-3.0-or-later */
import { bounded, CaptureCatalog } from "./capture";
import type { CaptureProvider, CaptureSource } from "./capture";
export interface LiveStream {
    key: string;
    source: { desktopSource: {id: string; sourcePid: number | null; soundshareId: string | number | null; soundshareSession: string | null}; quality: {resolution: number; frameRate: number} } | null;
    context: "default" | "stream";
    sourceName: string;
}

export interface StreamProvider extends CaptureProvider {
    known?: boolean;
    read(): LiveStream | null;
    setSource?: (source: NonNullable<LiveStream["source"]>, context: LiveStream["context"], sound?: boolean) => unknown;
    stop?: (key: string) => unknown;
    start?: (channel: string, source: CaptureSource, sound: boolean) => unknown;
    qualityAllowed?: (resolution: number, fps: number) => boolean;
}

export class StreamControls {
    private pending: {key:string; account:string; channel:string; sourceId:string; sourceName?:string; action:string; value:number; expires:number; before:LiveStream | null} | undefined;
    private sourceLabel: {key:string; id:string; name:string} | undefined;
    readonly catalog = new CaptureCatalog();
    private result = "";
    private knownKey = "";
    private maxResolution = 720;
    private maxFps = 30;
    get waiting() { return !!this.pending; }
    fail(message: string) { this.result = message.slice(0,512); }

    state(provider: StreamProvider) {
        const live = provider.read();
        if (this.sourceLabel && (live?.key !== this.sourceLabel.key || live.source?.desktopSource.id !== this.sourceLabel.id)) this.sourceLabel = undefined;
        if (live?.key !== this.knownKey) {
            this.knownKey = live?.key ?? "";
            this.maxResolution = 720; this.maxFps = 30;
        }
        if (live?.source) {
            this.maxResolution = Math.max(this.maxResolution,live.source.quality.resolution);
            this.maxFps = Math.max(this.maxFps,live.source.quality.frameRate);
        }
        const resolutions = live?.source ? [...new Set([0,480,720,1080,1440,live.source.quality.resolution])].filter(value=>provider.qualityAllowed ? provider.qualityAllowed(value,live.source!.quality.frameRate) : value === live.source!.quality.resolution || value > 0 && value<=this.maxResolution).sort((a,b)=>a-b) : [];
        const fpsOptions = live?.source ? [...new Set([5,15,30,60,live.source.quality.frameRate])].filter(value=>provider.qualityAllowed ? provider.qualityAllowed(live.source!.quality.resolution,value) : value<=this.maxFps).sort((a,b)=>a-b) : [];
        const writable = provider.known !== false && !!live?.key && !!live.source && typeof provider.setSource === "function";
        return {
            known: provider.known !== false,
            active: !!live, key: live?.key ?? "", sourceName: this.sourceLabel?.name ?? live?.sourceName ?? "", sourceId: live?.source?.desktopSource.id ?? "",
            resolution: live?.source?.quality.resolution ?? null, fps: live?.source?.quality.frameRate ?? null,
            audio: live?.source ? live.source.desktopSource.soundshareId != null : null,
            resolutions, fpsOptions, pending: !!this.pending, result: this.result,
            canStop: !!live?.key && typeof provider.stop === "function", canQuality: writable && resolutions.length > 0, canFps: writable && fpsOptions.length > 0,
            canStart: provider.known !== false && !!provider.account && !!provider.channel && !live && !!provider.start && !!provider.listSources,
            canSources: !!provider.listSources && !!provider.account && !!provider.channel,
            canSource: writable && !!provider.listSources && !!provider.account && !!provider.channel,
            canAudio: writable && (live!.source!.desktopSource.soundshareId != null || provider.audioSupported?.(live!.source!.desktopSource.id) === true),
            ...this.catalog.state(provider)
        };
    }

    async execute(provider: StreamProvider, action: string, data: any, now = Date.now()) {
        if (this.pending) throw new Error("Önceki yayın değişikliğinin sonucu bekleniyor");
        const state = this.state(provider);
        const live = provider.read();
        if (action === "stream_start") {
            if (!state.canStart || data?.channelId !== provider.channel || data?.streamKey !== "") throw new Error("Ses odası veya yayın durumu değişti");
            const audio = data.audio ?? false;
            if (typeof audio !== "boolean") throw new Error("Yayın sesi seçimi geçersiz");
            const account = provider.account, channel = provider.channel;
            this.pending = {key:"",account:account!,channel:channel!,sourceId:String(data.sourceId),action,value:audio ? 1 : 0,expires:now+8000,before:null};
            this.result = "Kaynak doğrulanıyor…";
            try {
                const source = await this.catalog.resolve(provider,data.sourceRevision,data.sourceId,now);
                this.pending!.sourceName = source.name;
                if (provider.known === false || provider.account !== account || provider.channel !== channel || provider.read()) throw new Error("Ses odası veya yayın durumu değişti");
                if (audio && !source.audioAvailable) throw new Error("Bu kaynak yayın sesi paylaşımını desteklemiyor");
                await bounded(()=>provider.start!(channel!,source,audio));
                this.catalog.clear(); this.result = "Discord'dan sonuç bekleniyor…";
            } catch (error) { this.pending = undefined; this.result = "Yayın başlatılamadı"; throw error; }
            return;
        }
        if (!live || data?.streamKey !== live.key) throw new Error("Yayın değişti. Güncel yayın üzerinden yeniden dene.");
        if (action === "stream_source") {
            if (!state.canSource || data?.channelId !== provider.channel) throw new Error("Yayın kaynağı bu istemcide değiştirilemiyor");
            const before: LiveStream = {...live,source:{...live.source!,desktopSource:{...live.source!.desktopSource},quality:{...live.source!.quality}}};
            const account = provider.account, channel = provider.channel;
            this.pending = {key:live.key,account:account ?? "",channel:channel ?? "",sourceId:String(data.sourceId),action,value:0,expires:now+8000,before};
            this.result = "Kaynak doğrulanıyor…";
            try {
                const selected = await this.catalog.resolve(provider,data.sourceRevision,data.sourceId,now);
                this.pending!.sourceName = selected.name;
                const current = provider.read();
                if (provider.known === false || provider.account !== account || provider.channel !== channel || current?.key !== live.key || JSON.stringify(current.source) !== JSON.stringify(before.source)) throw new Error("Ses odası veya yayın kaynağı değişti");
                if (before.source!.desktopSource.soundshareId != null && !selected.audioAvailable) throw new Error("Bu kaynak ses paylaşımını desteklemiyor; önce yayın sesini kapat");
                await bounded(()=>provider.setSource!({...before.source!,desktopSource:{...before.source!.desktopSource,id:selected.id,sourcePid:null}},before.context));
                this.catalog.clear(); this.result = "Discord'dan sonuç bekleniyor…";
            } catch (error) { this.pending = undefined; this.result = "Kaynak değiştirilemedi"; throw error; }
            return;
        }
        let value = 0;
        if (action === "stream_stop") {
            if (!state.canStop) throw new Error("Yayın durdurma bu istemcide kullanılamıyor");
        } else if (action === "stream_audio") {
            if (!state.canAudio || typeof data.audio !== "boolean" || (data.audio && provider.audioSupported?.(live.source!.desktopSource.id) !== true)) throw new Error("Bu kaynak yayın sesini değiştirmeyi desteklemiyor");
            value = data.audio ? 1 : 0;
        } else if (action === "stream_quality" || action === "stream_fps") {
            value = action === "stream_quality" ? data.resolution : data.fps;
            const options = action === "stream_quality" ? state.resolutions : state.fpsOptions;
            if (!(action === "stream_quality" ? state.canQuality : state.canFps) || !Number.isInteger(value) || !options.includes(value)) throw new Error("Bu yayın ayarı kullanılamıyor");
        } else throw new Error("Yayın kontrolü bu istemcide desteklenmiyor");
        this.pending = {key:live.key,account:provider.account ?? "",channel:provider.channel ?? "",sourceId:"",action,value,expires:now+5000,before:{...live,source:live.source ? {...live.source,desktopSource:{...live.source.desktopSource},quality:{...live.source.quality}} : null}};
        this.result = "Discord'dan sonuç bekleniyor…";
        let deadline: ReturnType<typeof setTimeout> | undefined;
        try {
            const apply = () => {
                if (action === "stream_stop") return provider.stop!(live.key);
                const previous = live.source!;
                if (action === "stream_audio") return provider.setSource!({...previous,desktopSource:{...previous.desktopSource},quality:{...previous.quality}},live.context,!!value);
                return provider.setSource!({...previous,desktopSource:{...previous.desktopSource},quality:{...previous.quality,
                    ...(action === "stream_quality" ? {resolution:value} : {frameRate:value})}},live.context);
            };
            await Promise.race([Promise.resolve(apply()),new Promise((_,reject)=>{deadline=setTimeout(()=>reject(new Error("Discord yayın işlemi zaman aşımına uğradı")),3000);})]);
        } catch (error) {
            this.pending = undefined;
            this.result = "Yayın işlemi uygulanamadı";
            throw error;
        } finally { if (deadline) clearTimeout(deadline); }
    }

    observe(provider: StreamProvider, now = Date.now()): boolean {
        const pending = this.pending;
        if (!pending) return false;
        if (provider.known === false || pending.account !== (provider.account ?? "") || pending.channel !== (provider.channel ?? "")) {
            this.pending = undefined; this.result = "Discord oturumu değişti; önceki işlem iptal edildi"; return true;
        }
        const live = provider.read();
        if (pending.action === "stream_start") {
            if (live?.key && live.source?.desktopSource.id === pending.sourceId && Number(live.source.desktopSource.soundshareId != null) === pending.value) { this.sourceLabel = {key:live.key,id:pending.sourceId,name:pending.sourceName ?? live.sourceName}; this.pending = undefined; this.result = "Yayın başlatıldığı doğrulandı"; return true; }
            if (live?.key && live.source && live.source.desktopSource.id !== pending.sourceId) { this.pending = undefined; this.result = "Başka bir yayın başladı; önceki işlem iptal edildi"; return true; }
            if (now >= pending.expires) { this.pending = undefined; this.result = "Discord yayının başladığını doğrulamadı"; return true; }
            return false;
        }
        if (live?.key === pending.key && pending.before?.source && live.source && (pending.action === "stream_source" ? ![pending.before.source.desktopSource.id,pending.sourceId].includes(live.source.desktopSource.id) : pending.action !== "stream_stop" && live.source.desktopSource.id !== pending.before.source.desktopSource.id)) {
            this.pending = undefined; this.result = "Yayın kaynağı değişti; önceki işlem iptal edildi"; return true;
        }
        const confirmed = pending.action === "stream_stop" ? !live : live?.key === pending.key &&
            (pending.action === "stream_source" ? live.source?.desktopSource.id === pending.sourceId : pending.action === "stream_audio" ? !!live.source && Number(live.source.desktopSource.soundshareId != null) === pending.value :
            (pending.action === "stream_quality" ? live.source?.quality.resolution : live.source?.quality.frameRate) === pending.value);
        if (confirmed) { if (pending.action === "stream_source" && live) this.sourceLabel = {key:live.key,id:pending.sourceId,name:pending.sourceName ?? live.sourceName}; this.pending = undefined; this.result = "Yayın değişikliği doğrulandı"; return true; }
        if (live?.key !== pending.key) { this.pending = undefined; this.result = "Yayın değişti; önceki işlem iptal edildi"; return true; }
        if (now >= pending.expires) {
            this.pending = undefined;
            this.result = "Discord değişikliği doğrulamadı";
            // An unconfirmed command must not undo a later change made in Discord.
            return true;
        }
        return false;
    }
}
