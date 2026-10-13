import assert from "node:assert/strict";
import test from "node:test";
import { SnapshotCache, StoreListeners, encodeSnapshot } from "../integrations/vencord/gameLibraryBridge.desktop/state.ts";

test("idle confirmation reuses cached data and changes coalesce per section", () => {
    const calls = {friends:0,rooms:0,controls:0};
    const cache = new SnapshotCache();
    const read = section => {calls[section]++;return {[section]:calls[section]};};
    const first = cache.packet(1,read);
    assert.equal(first.type,"snapshot");
    for(let i=0;i<100;i++) assert.equal(cache.packet(1,read).type,"snapshot_unchanged");
    assert.deepEqual(calls,{friends:1,rooms:1,controls:1});
    cache.invalidate("friends"); cache.invalidate("friends");
    assert.equal(cache.packet(1,read).revision,2);
    assert.deepEqual(calls,{friends:2,rooms:1,controls:1});
    assert.equal(cache.packet(2,read).type,"snapshot");
    assert.deepEqual(calls,{friends:3,rooms:2,controls:2});
});

test("store listeners detach and failed reads require a full refresh", () => {
    const attached = new Set();
    const store = {addChangeListener:cb=>attached.add(cb),removeChangeListener:cb=>attached.delete(cb)};
    const listeners = new StoreListeners();
    assert.equal(listeners.bind(store,()=>{}),true);
    listeners.bind(store,()=>{});
    assert.equal(attached.size,1);
    listeners.clear(); assert.equal(attached.size,0);
    assert.equal(listeners.bind({},()=>{}),false);
    const cache = new SnapshotCache();
    assert.throws(()=>cache.packet(1,()=>{throw new Error("not ready");}));
    const read = [];
    assert.equal(cache.packet(1,section=>{read.push(section);return {};} ).type,"snapshot");
    assert.deepEqual(read,["friends","rooms","controls"]);
    cache.invalidate("controls");
    assert.equal(cache.packet(1,()=>({})).type,"snapshot_unchanged");
});

test("a failed section cannot swallow changes already read from another section", () => {
    const cache = new SnapshotCache();
    const values = {friends:["old"],rooms:[],controls:{}};
    cache.packet(1,section=>({[section]:values[section]}));
    values.friends = ["new"];
    cache.invalidate("friends","rooms");
    assert.throws(()=>cache.packet(1,section=>{if(section==="rooms") throw new Error("not ready");return {[section]:values[section]};}));
    const packet = cache.packet(1,section=>({[section]:values[section]}));
    assert.equal(packet.type,"snapshot");
    assert.deepEqual(packet.friends,["new"]);
    assert.equal(packet.revision,2);
});

test("a rejected delivery is resent as full state rather than confirming an unseen revision", () => {
    const cache = new SnapshotCache();
    const read = section=>({[section]:[]});
    cache.packet(1,read);
    cache.invalidate();
    const retry = cache.packet(1,read);
    assert.equal(retry.type,"snapshot");
    assert.equal(retry.revision,2);
    assert.equal(cache.packet(1,read).type,"snapshot_unchanged");
});

test("UTF-8 wire size remains bounded while favorite rows and valid Unicode survive", () => {
    const friends = Array.from({length:200},(_,index)=>({id:String(index),name:"界".repeat(64),activity:"界".repeat(128)}));
    const rooms = Array.from({length:100},(_,index)=>({id:String(index),name:"界".repeat(128),guild:"界".repeat(128),members:1}));
    const encoded = encodeSnapshot({type:"snapshot",friends,rooms,roomCatalog:rooms,favoriteIds:friends.map(row=>row.id),favoriteRoomIds:rooms.slice(0,50).map(row=>row.id),result:"\ud800"},text=>Buffer.byteLength(text));
    assert.ok(encoded);
    assert.ok(Buffer.byteLength(encoded)+1<=256*1024);
    const value = JSON.parse(encoded);
    assert.equal(value.friends.length,200);
    assert.ok(value.rooms.length>=50);
    assert.equal(value.result,"\ufffd");
    assert.equal(rooms.length,100);
});

test("chat and music cannot exceed the wire budget or mutate cached conversations", () => {
    const messages=Array.from({length:30},(_,id)=>({id:String(id),content:"界".repeat(4000)}));
    const friends=Array.from({length:200},(_,id)=>({id:String(id),title:"界".repeat(1000)}));
    const encoded=encodeSnapshot({type:"snapshot",spotify:{friends},chat:{messages,sentNonce:"123456789012345678"}},text=>Buffer.byteLength(text));
    assert.ok(encoded);
    assert.ok(Buffer.byteLength(encoded)+1<=256*1024);
    const packet=JSON.parse(encoded);
    assert.equal(packet.chat.messages.at(-1).id,"29");
    assert.equal(packet.chat.sentNonce,"123456789012345678");
    assert.equal(messages.length,30);assert.equal(friends.length,200);
});
