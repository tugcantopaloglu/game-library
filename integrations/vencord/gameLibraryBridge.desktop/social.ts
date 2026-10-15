/* SPDX-License-Identifier: GPL-3.0-or-later */
export function validId(value: unknown): value is string {
    return typeof value === "string" && /^\d{15,22}$/.test(value) && BigInt(value) > 0n && BigInt(value) <= 18446744073709551615n;
}

export function favoriteIds(value: string): string[] {
    return [...new Set(value.split(",").map(id => id.trim()).filter(validId))].slice(0, 200).sort();
}

export function avatarUrl(id: string, avatar: unknown, discriminator: unknown): string {
    if (!validId(id)) return "";
    if (typeof avatar === "string" && /^[a-zA-Z0-9_]{1,128}$/.test(avatar)) return `https://cdn.discordapp.com/avatars/${id}/${avatar}.png?size=64`;
    const legacy = typeof discriminator === "string" && /^\d{1,4}$/.test(discriminator) ? Number(discriminator) : 0;
    const index = legacy > 0 ? legacy % 5 : Number((BigInt(id) >> 22n) % 6n);
    return `https://cdn.discordapp.com/embed/avatars/${index}.png`;
}

export function lastMessageAt(value: unknown, now = Date.now()): number | null {
    if (!validId(value)) return null;
    const timestamp = Number((BigInt(value) >> 22n) + 1420070400000n);
    return timestamp <= now + 300000 ? timestamp : null;
}

export function visibleFriends<T extends { id: string; lastMessageAt: number | null }>(rows: T[], favorites: string[], limit = 200): T[] {
    const favorite = new Set(favorites);
    return rows.sort((first, second) => Number(favorite.has(second.id)) - Number(favorite.has(first.id)) || (second.lastMessageAt ?? 0) - (first.lastMessageAt ?? 0) || first.id.localeCompare(second.id)).slice(0, limit);
}

export function unreadCount(value: unknown): number {
    return typeof value === "number" && Number.isSafeInteger(value) && value > 0 ? Math.min(value,999) : 0;
}
