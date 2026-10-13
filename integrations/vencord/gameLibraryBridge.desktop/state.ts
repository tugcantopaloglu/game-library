/* SPDX-License-Identifier: GPL-3.0-or-later */
export type Section = "friends" | "rooms" | "controls";
const sections: Section[] = ["friends", "rooms", "controls"];

export class SnapshotCache {
    private dirty = new Set<Section>(sections);
    private values: Partial<Record<Section, Record<string, unknown>>> = {};
    private generation = -1;
    private revision = 0;
    private full = true;
    private signatures: Partial<Record<Section,string>> = {};

    invalidate(...changed: Section[]) {
        if (!changed.length) this.full = true;
        for (const section of changed.length ? changed : sections) this.dirty.add(section);
    }

    packet(generation: number, collect: (section: Section) => Record<string, unknown>) {
        if (generation !== this.generation) {
            this.generation = generation;
            this.values = {};
            this.signatures = {};
            this.invalidate();
        }
        if (!this.dirty.size) return { type: "snapshot_unchanged", protocol: 1, version: 1, revision: this.revision };
        const changed = [...this.dirty];
        this.dirty.clear();
        const values = {...this.values};
        const signatures = {...this.signatures};
        let different = false;
        try {
            for (const section of changed) {
                const value = collect(section);
                const signature = JSON.stringify(value);
                if (signatures[section] !== signature) different = true;
                signatures[section] = signature;
                values[section] = value;
            }
        } catch (error) {
            this.invalidate();
            throw error;
        }
        this.values = values;
        this.signatures = signatures;
        if (!different && !this.full) return {type:"snapshot_unchanged",protocol:1,version:1,revision:this.revision};
        this.full = false;
        return { type: "snapshot", protocol: 1, version: 1, revision: ++this.revision, ...this.values.friends, ...this.values.rooms, ...this.values.controls };
    }
}

export function encodeSnapshot(payload: any, byteLength: (text:string) => number): string | null {
    const value = payload && typeof payload === "object" ? {...payload} : payload;
    const encode = () => JSON.stringify(value, (_key,item)=>typeof item === "string" ? item.toWellFormed() : item);
    let text = encode();
    if (typeof text !== "string") return null;
    if (value?.type === "snapshot") {
        for (const [key,favorites] of [["rooms","favoriteRoomIds"],["roomCatalog",""],["friends","favoriteIds"]]) {
            if (!Array.isArray(value[key])) continue;
            const minimum = Math.min(value[key].length,Array.isArray(value[favorites]) ? value[favorites].length : 0);
            while (byteLength(text) + 1 > 256 * 1024 && value[key].length > minimum) {
                value[key] = value[key].slice(0,Math.max(minimum,Math.floor(value[key].length / 2)));
                value[`${key}Truncated`] = true;
                if (key === "roomCatalog") value.roomCatalogMore = true;
                text = encode()!;
            }
        }
    }
    return byteLength(text) + 1 <= 256 * 1024 ? text : null;
}

export class StoreListeners {
    private subscribed = new Map<object, () => void>();

    bind(store: any, changed: () => void): boolean {
        if (!store || typeof store.addChangeListener !== "function" || typeof store.removeChangeListener !== "function") return false;
        if (!this.subscribed.has(store)) {
            store.addChangeListener(changed);
            this.subscribed.set(store, () => store.removeChangeListener(changed));
        }
        return true;
    }

    clear() {
        for (const remove of this.subscribed.values()) { try { remove(); } catch {} }
        this.subscribed.clear();
    }
}
