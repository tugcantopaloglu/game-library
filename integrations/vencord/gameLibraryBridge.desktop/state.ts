/* SPDX-License-Identifier: GPL-3.0-or-later */
export type Section = "friends" | "rooms" | "controls";
const sections: Section[] = ["friends", "rooms", "controls"];

export class SnapshotCache {
    private dirty = new Set<Section>(sections);
    private values: Partial<Record<Section, Record<string, unknown>>> = {};
    private generation = -1;
    private revision = 0;

    invalidate(...changed: Section[]) {
        for (const section of changed.length ? changed : sections) this.dirty.add(section);
    }

    packet(generation: number, collect: (section: Section) => Record<string, unknown>) {
        if (generation !== this.generation) {
            this.generation = generation;
            this.values = {};
            this.invalidate();
        }
        if (!this.dirty.size) return { type: "snapshot_unchanged", protocol: 1, version: 1, revision: this.revision };
        const changed = [...this.dirty];
        this.dirty.clear();
        try {
            for (const section of changed) this.values[section] = collect(section);
        } catch (error) {
            this.invalidate();
            throw error;
        }
        return { type: "snapshot", protocol: 1, version: 1, revision: ++this.revision, ...this.values.friends, ...this.values.rooms, ...this.values.controls };
    }
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
