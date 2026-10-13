import assert from "node:assert/strict";
import test from "node:test";
import { SnapshotCache, StoreListeners } from "../integrations/vencord/gameLibraryBridge.desktop/state.ts";

test("idle confirmation reuses cached data and changes coalesce per section", () => {
    const calls = {friends:0,rooms:0,controls:0};
    const cache = new SnapshotCache();
    const read = section => {calls[section]++;return {[section]:[]};};
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
});
