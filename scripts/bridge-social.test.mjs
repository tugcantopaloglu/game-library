import assert from "node:assert/strict";
import test from "node:test";
import { favoriteIds, lastMessageAt, visibleFriends } from "../integrations/vencord/gameLibraryBridge.desktop/social.ts";

test("conversation times come from valid message snowflakes", () => {
    const now = 1800000000000;
    const id = ((BigInt(now - 60000) - 1420070400000n) << 22n).toString();
    assert.equal(lastMessageAt(id, now), now - 60000);
    assert.equal(lastMessageAt("bad", now), null);
    assert.equal(lastMessageAt("9999999999999999999999", now), null);
    assert.equal(lastMessageAt(((BigInt(now + 600000) - 1420070400000n) << 22n).toString(), now), null);
});

test("favorite inclusion precedes the limit and remaining friends are recent first", () => {
    const rows = [{id:"123456789012345678",lastMessageAt:null}, {id:"223456789012345678",lastMessageAt:20}, {id:"323456789012345678",lastMessageAt:10}];
    assert.deepEqual(favoriteIds("bad,123456789012345678,123456789012345678"), ["123456789012345678"]);
    assert.deepEqual(visibleFriends(rows,["123456789012345678"],2).map(row=>row.id),["123456789012345678","223456789012345678"]);
});
