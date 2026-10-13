/* SPDX-License-Identifier: GPL-3.0-or-later */
import { validId } from "./social";

/** Matches the token-free accessors in Vencord's MediaEngineStore declarations. */
export interface BroadcastStoreReader {
    getInputDevices?(): unknown;
    getOutputDevices?(): unknown;
    getInputDeviceId?(): unknown;
    getOutputDeviceId?(): unknown;
}

export interface BroadcastSelector {
    setInputDevice(id: string): void | Promise<void>;
    setOutputDevice(id: string): void | Promise<void>;
}

export interface BroadcastProvider {
    account?: string;
    store?: BroadcastStoreReader;
    /** Only supply an adapter whose actual Discord dispatch contract was recognized. */
    selector?: BroadcastSelector;
}

export interface BroadcastDevice {
    id: string;
    name: string;
}

type RecordValue = Record<string, unknown>;

function object(value: unknown): RecordValue | undefined {
    return value !== null && typeof value === "object" && !Array.isArray(value) ? value as RecordValue : undefined;
}

function deviceId(value: unknown): value is string {
    return typeof value === "string" && value.length > 0 && value.length <= 512 && !/[\u0000-\u001f\u007f]/.test(value);
}

function broadcastDevices(value: unknown): BroadcastDevice[] | null {
    const catalog = object(value);
    if (!catalog) return null;
    const result: BroadcastDevice[] = [];
    // The store contract is a dictionary keyed by its actual device ID, not a guessed Windows ID.
    for (const [id, value] of Object.entries(catalog).slice(0, 2048)) {
        const device = object(value);
        if (!device || !deviceId(id) || id === "default" || device.id !== id || device.disabled !== false
            || !Number.isInteger(device.index) || (device.index as number) < 0
            || typeof device.name !== "string" || device.name.length > 512 || /[\u0000-\u001f\u007f]/.test(device.name)
            || !/\bnvidia broadcast\b/i.test(device.name)) continue;
        result.push({ id, name: device.name.slice(0, 192) });
    }
    return result;
}

function unique(devices: BroadcastDevice[], selectedId: string): BroadcastDevice | null {
    const selected = devices.find(device => device.id === selectedId);
    // Avoid choosing an arbitrary device when a broken/duplicate virtual installation is present.
    return selected ?? (devices.length === 1 ? devices[0] : null);
}

function read(provider: BroadcastProvider) {
    const store = provider.store;
    if (!validId(provider.account) || !store) return null;
    try {
        if (![store.getInputDevices, store.getOutputDevices, store.getInputDeviceId,
            store.getOutputDeviceId].every(value => typeof value === "function")) return null;
        const inputs = broadcastDevices(store.getInputDevices!());
        const outputs = broadcastDevices(store.getOutputDevices!());
        const inputId = store.getInputDeviceId!(), outputId = store.getOutputDeviceId!();
        if (!inputs || !outputs || !deviceId(inputId) || !deviceId(outputId)) return null;
        return { inputs, outputs, inputId, outputId, microphone: unique(inputs, inputId), speaker: unique(outputs, outputId) };
    } catch { return null; }
}

/**
 * Verified against Discord's own client action code: both functions accept a device ID
 * and dispatch AUDIO_SET_*_DEVICE with id/oldId. Unknown client implementations stay disabled.
 */
export function broadcastAdapter(value: unknown): BroadcastSelector | undefined {
    try {
        const media = object(value);
        if (!media) return undefined;
        const input = media.setInputDevice, output = media.setOutputDevice;
        const recognizes = (action: unknown, direction: "INPUT" | "OUTPUT") => {
            if (typeof action !== "function" || action.length !== 1) return false;
            const body = Function.prototype.toString.call(action);
            return new RegExp(`type\\s*:\\s*["']AUDIO_SET_${direction}_DEVICE["']`).test(body)
                && /\bid\s*:/.test(body) && /\boldId\s*:/.test(body)
                && body.includes(direction === "INPUT" ? "getInputDeviceId" : "getOutputDeviceId");
        };
        if (!recognizes(input, "INPUT") || !recognizes(output, "OUTPUT")) return undefined;
        return {
            setInputDevice: id => Reflect.apply(input as (...args: string[]) => void, media, [id]),
            setOutputDevice: id => Reflect.apply(output as (...args: string[]) => void, media, [id])
        };
    } catch { return undefined; }
}

export class BroadcastControls {
    private pending: { account: string; microphone: BroadcastDevice; speaker: BroadcastDevice; deadline: number } | undefined;
    private result = "";
    private resultAccount: string | undefined;

    get waiting() { return !!this.pending; }

    clear() { this.pending = undefined; this.result = ""; this.resultAccount = undefined; }

    state(provider: BroadcastProvider) {
        const observed = read(provider);
        const microphone = observed?.microphone ?? null, speaker = observed?.speaker ?? null;
        const supported = !!observed && typeof provider.selector?.setInputDevice === "function"
            && typeof provider.selector?.setOutputDevice === "function";
        const selected = !!microphone && !!speaker && observed?.inputId === microphone.id && observed?.outputId === speaker.id;
        return {
            supported, microphone, speaker, selected, waiting: this.waiting,
            canSelect: supported && !!microphone && !!speaker && !this.waiting,
            result: this.resultAccount === provider.account ? this.result : "",
            status: !validId(provider.account) ? "Discord oturumu bekleniyor."
                : !supported ? "Bu Discord sürümünde Broadcast ses cihazları güvenilir biçimde seçilemiyor."
                    : !microphone || !speaker ? "Discord'da kullanılabilir NVIDIA Broadcast mikrofonu ve hoparlörü bulunamadı; Broadcast'u açıp listeyi yenile."
                        : selected ? "NVIDIA Broadcast mikrofonu ve hoparlörü Discord'da seçili."
                            : "Broadcast cihazları bulundu. Discord ses cihazlarını seçmek için düğmeyi kullan."
        };
    }

    async execute(provider: BroadcastProvider, action: string, data: unknown, now = Date.now()) {
        const payload = object(data);
        if (action !== "broadcast_audio" || !payload || Object.keys(payload).length !== 0) throw new Error("Broadcast cihaz işlemi geçersiz.");
        if (this.waiting) throw new Error("Discord cihaz seçimi sonucu bekleniyor.");
        const state = this.state(provider);
        if (!state.canSelect || !state.microphone || !state.speaker || !provider.selector || !validId(provider.account)) {
            throw new Error("NVIDIA Broadcast ses cihazları bu Discord sürümünde seçilemiyor; Ses ve Görüntü ayarlarını kullan.");
        }
        if (state.selected) {
            this.resultAccount = provider.account;
            this.result = "NVIDIA Broadcast mikrofonu ve hoparlörü Discord'da zaten seçili.";
            return;
        }
        const pending = { account: provider.account, microphone: state.microphone, speaker: state.speaker, deadline: now + 10000 };
        this.pending = pending;
        this.resultAccount = provider.account;
        this.result = "Broadcast cihaz seçimi iletildi; Discord doğrulaması bekleniyor.";
        try {
            await provider.selector.setInputDevice(state.microphone.id);
            const current = read(provider);
            if (this.pending !== pending || provider.account !== pending.account || !current
                || !current.inputs.some(device => device.id === state.microphone!.id)
                || !current.outputs.some(device => device.id === state.speaker!.id)) throw new Error("Changed");
            await provider.selector.setOutputDevice(state.speaker.id);
        } catch {
            this.pending = undefined;
            this.result = "Discord cihaz değişimini tamamlamadı; Ses ve Görüntü ayarlarını kontrol et.";
            throw new Error(this.result);
        }
        this.observe(provider, now);
    }

    observe(provider: BroadcastProvider, now = Date.now()): boolean {
        const pending = this.pending;
        if (!pending) return false;
        const observed = read(provider);
        if (provider.account !== pending.account) {
            this.pending = undefined;
            this.resultAccount = provider.account;
            this.result = "Discord oturumu değişti; Broadcast cihaz seçimi doğrulanmadı.";
            return true;
        }
        if (observed && (!observed.inputs.some(device => device.id === pending.microphone.id)
            || !observed.outputs.some(device => device.id === pending.speaker.id))) {
            this.pending = undefined;
            this.result = "Broadcast cihaz listesi değişti; ses ayarlarını kontrol et.";
            return true;
        }
        if (observed?.inputId === pending.microphone.id && observed?.outputId === pending.speaker.id) {
            this.pending = undefined;
            this.result = "NVIDIA Broadcast mikrofonu ve hoparlörü Discord'da doğrulandı.";
            return true;
        }
        if (now >= pending.deadline) {
            this.pending = undefined;
            this.result = "Discord cihaz seçimini doğrulamadı; Ses ve Görüntü ayarlarını kontrol et.";
            return true;
        }
        return false;
    }
}
