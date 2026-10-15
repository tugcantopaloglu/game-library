import assert from "node:assert/strict";
import test from "node:test";
import { avatarUrl, favoriteIds, lastMessageAt, unreadCount, visibleFriends } from "../integrations/vencord/gameLibraryBridge.desktop/social.ts";

test("avatar URLs preserve custom images and both Discord default avatar schemes",()=>{
    const id="223456789012345678";
    assert.equal(avatarUrl(id,"a_abcdef","0"),`https://cdn.discordapp.com/avatars/${id}/a_abcdef.png?size=64`);
    assert.equal(avatarUrl(id,null,"1234"),"https://cdn.discordapp.com/embed/avatars/4.png");
    assert.equal(avatarUrl(id,null,"0"),`https://cdn.discordapp.com/embed/avatars/${Number((BigInt(id)>>22n)%6n)}.png`);
    assert.equal(avatarUrl("bad","abc","0"),"");
    assert.equal(new URL(avatarUrl(id,"../secret","0")).hostname,"cdn.discordapp.com");
});

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

test("unread badges reject unknown values and preserve counts through friend ordering",()=>{
    for(const value of [undefined,null,NaN,Infinity,-1,1.5,"3"]) assert.equal(unreadCount(value),0);
    assert.equal(unreadCount(1234),999);
    const rows=visibleFriends([{id:"223456789012345678",lastMessageAt:12,unread:unreadCount(3)}],[]);
    assert.equal(rows[0].unread,3);
});
