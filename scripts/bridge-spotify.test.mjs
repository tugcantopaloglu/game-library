import assert from "node:assert/strict";
import test from "node:test";
import { listenAlongAdapter, SpotifyControls, spotifyActivity, spotifyTrack, validTrackId } from "../integrations/vencord/gameLibraryBridge.desktop/spotify.ts";

const now = 1800000000000;
const account = "123456789012345678", friend = "223456789012345678", outsider = "323456789012345678";
const trackId = "4iV5W9uYEdYUVa79Axb7Rh", nextId = "1301WleyT98MSxVHPZCA6M";
const activity = (id = trackId) => ({ type: 2, name: "Spotify", sync_id: id, details: "Song", state: "Artist", timestamps: { start: now - 60000, end: now + 180000 }, assets: { large_text: "Album" }, metadata: { type: "track" } });
const track = () => ({ id: trackId, name: "Song", isLocal: false, type: "track", artists: [{ name: "Artist" }], album: { name: "Album" } });
function fixture(overrides = {}) {
    return { account, friendIds: () => [friend], activities: () => [activity()], store: { hasConnectedAccount: () => true, getTrack: track, getSyncingWith: () => undefined }, ...overrides };
}

test("only valid Spotify listening activities supply track links", () => {
    assert.equal(spotifyActivity(activity(), now).uri, `spotify:track:${trackId}`);
    for (const broken of [{ ...activity(), type: 0 }, { ...activity(), name: "Other" }, { ...activity(), sync_id: "../../bad" }, { ...activity(), metadata: { type: "episode" } }, { ...activity(), timestamps: { start: now - 60000, end: now - 1 } }, { ...activity(), timestamps: { start: now + 120000, end: now + 180000 } }]) {
        assert.equal(spotifyActivity(broken, now), null);
    }
    assert.equal(spotifyActivity(null, now), null);
});

test("track labels are bounded and local files never become public links", () => {
    const info = spotifyTrack({ ...track(), name: "a\n".repeat(1000), accessToken: "not-exported" });
    assert.equal(info.title.length, 192);
    assert.equal(info.title.includes("\n"), false);
    assert.equal(JSON.stringify(info).includes("not-exported"), false);
    assert.equal(spotifyTrack({ ...track(), isLocal: true }), null);
    assert.equal(spotifyTrack({ ...track(), type: "episode" }), null);
    assert.equal(validTrackId(trackId + "\n"), false);
});

test("token-bearing accessors are never touched or exported", () => {
    const provider = fixture();
    Object.defineProperty(provider.store, "getActiveSocketAndDevice", { get() { throw new Error("Token accessor must never be touched"); } });
    const state = new SpotifyControls().state(provider, now);
    assert.equal(state.connected, true);
    assert.equal(state.friends[0].track.id, trackId);
    assert.equal(Object.hasOwn(state, "accessToken"), false);
});

test("unknown accessors disable joins and disconnected accounts hide own track", () => {
    const controls = new SpotifyControls();
    assert.equal(controls.state(fixture({ store: {} }), now).connected, null);
    assert.equal(controls.state(fixture(), now).canListenAlong, false);
    const disconnected = controls.state(fixture({ store: { hasConnectedAccount: () => false, getTrack: track } }), now);
    assert.equal(disconnected.current, null);
    assert.equal(disconnected.canListenAlong, false);
});

test("friends cannot be substituted with an arbitrary user or stale presence", async () => {
    const controls = new SpotifyControls();
    let opens = 0;
    const provider = fixture({ openProfile: () => opens++ });
    await assert.rejects(() => controls.execute(provider, "spotify_profile", { userId: outsider }, now), /etkinliği/);
    await assert.rejects(() => controls.execute(provider, "spotify_profile", { userId: friend }, now + 180001), /etkinliği/);
    assert.equal(opens, 0);
    assert.deepEqual(controls.state(fixture({ friendIds: () => [account, outsider], activities: () => [] }), now).friends, []);
});

test("explicit profile action opens Discord and waits for the user's native Listen Along choice", async () => {
    let opened;
    const controls = new SpotifyControls();
    await controls.execute(fixture({ openProfile: id => opened = id }), "spotify_profile", { userId: friend }, now);
    assert.equal(opened, friend);
    assert.equal(controls.waiting, false);
    assert.match(controls.state(fixture(), now).result, /profilinde/);
});

test("track action is bound to the current selected user's track", async () => {
    let uri;
    const controls = new SpotifyControls();
    const provider = fixture({ openExternal: value => uri = value });
    await controls.execute(provider, "spotify_track", { userId: friend, trackId }, now);
    assert.equal(uri, `spotify:track:${trackId}`);
    await assert.rejects(() => controls.execute(provider, "spotify_track", { userId: friend, trackId: nextId }, now), /değişti/);
    await assert.rejects(() => controls.execute(provider, "spotify_track", { trackId: "file:///secret" }, now), /açılamıyor/);
});

test("arbitrary client exception strings are not copied into the snapshot", async () => {
    const controls = new SpotifyControls();
    await assert.rejects(() => controls.execute(fixture({ openExternal: () => { throw new Error("accessToken=secret"); } }), "spotify_track", { trackId }, now), /Spotify açılamadı/);
    assert.equal(JSON.stringify(controls.state(fixture(), now)).includes("secret"), false);
});

test("join is disabled when its action or authoritative readback is missing", async () => {
    const controls = new SpotifyControls();
    const provider = fixture({ join: () => assert.fail("must not be called"), store: { hasConnectedAccount: () => true, getTrack: track } });
    await assert.rejects(() => controls.execute(provider, "spotify_join", { userId: friend }, now), /doğrulanamıyor/);
    assert.equal(controls.state(provider, now).canListenAlong, false);
});

test("join waits for the exact target's Discord readback", async () => {
    let synced, joined;
    const controls = new SpotifyControls();
    const provider = fixture({ join: id => joined = id });
    provider.store.getSyncingWith = () => synced;
    await controls.execute(provider, "spotify_join", { userId: friend }, now);
    assert.equal(joined, friend);
    assert.equal(controls.waiting, true);
    synced = { userId: outsider, accessToken: "not-exported" };
    assert.equal(controls.observe(provider, now + 1000), false);
    synced = { userId: friend };
    assert.equal(controls.observe(provider, now + 2000), true);
    assert.equal(controls.waiting, false);
    assert.match(controls.state(provider, now).result, /doğrulandı/);
});

test("pending joins time out or fail after account switch", async () => {
    const controls = new SpotifyControls();
    const provider = fixture({ join: () => {} });
    await controls.execute(provider, "spotify_join", { userId: friend }, now);
    await assert.rejects(() => controls.execute(provider, "spotify_join", { userId: friend }, now + 1), /bekleniyor/);
    assert.equal(controls.observe(provider, now + 45000), true);
    assert.match(controls.state(provider, now).result, /doğrulamadı/);
    await controls.execute(provider, "spotify_join", { userId: friend }, now);
    provider.account = outsider;
    assert.equal(controls.observe(provider, now + 1000), true);
    assert.match(controls.state(provider, now).result, /oturumu değişti/);
});

test("a failing join clears waiting state without leaking client errors", async () => {
    const controls = new SpotifyControls();
    const provider = fixture({ join: () => { throw new Error("bearer secret"); } });
    await assert.rejects(() => controls.execute(provider, "spotify_join", { userId: friend }, now), /kabul etmedi/);
    assert.equal(controls.waiting, false);
    assert.equal(JSON.stringify(controls.state(provider, now)).includes("secret"), false);
});

test("oversized friend lists are bounded and duplicate rows collapse", () => {
    const controls = new SpotifyControls();
    const provider = fixture({ friendIds: () => Array(5000).fill(friend) });
    assert.equal(controls.state(provider, now).friends.length, 1);
    assert.deepEqual(controls.state(fixture({ account: undefined }), now).friends, []);
});

test("Listen Along rejects unknown wrappers and lower-level dispatch functions", () => {
    assert.equal(listenAlongAdapter(undefined), undefined);
    assert.equal(listenAlongAdapter((activity, userId) => ({type:"ACTIVITY_SYNC",activity,userId})), undefined);
    assert.equal(listenAlongAdapter(async function fake(e,t,d) { return e.hasSpotifyAccount && t && d; }), undefined);
});

test("recognized native wrapper keeps launch/Premium checks and uses the native button contract", async () => {
    const calls = [];
    let premium = true;
    const o = { A: connected => connected };
    const s = { dM: async () => calls.push("launch"), G8: async () => { calls.push("premium"); if (!premium) throw new Error("premium required"); } };
    const r = { openModalLazy: () => calls.push("premium modal") };
    const a = { OH: (value, userId) => calls.push({ activity: value, userId }) };
    const l = { A: (type, user, value, context) => calls.push({ type, user, context }) };
    // Native structure is intentional: changing its contract must disable detection.
    async function d(e,t,d){let{hasSpotifyAccount:c,activity:u,user:_}=e;(0,o.A)(c)&&null!=u&&null!=u.sync_id&&(await (0,s.dM)(),await (0,s.G8)().catch(e=>((0,r.openModalLazy)(async()=>{}),Promise.reject(e))),a.OH(u,_.id),(0,l.A)(t,_,u,d))}
    const join = listenAlongAdapter(d);
    assert.equal(typeof join, "function");
    const live = { ...activity(), timestamps: { start: Date.now() - 1000, end: Date.now() + 100000 }, party: { id: `spotify:${friend}` } };
    await join(friend, live);
    assert.deepEqual(calls.slice(0, 2), ["launch", "premium"]);
    assert.equal(calls[2].userId, friend);
    assert.equal(calls[3].type, "user_activity_sync");
    assert.deepEqual(calls[3].user, { id: friend });
    assert.equal(calls[3].context, undefined);
    calls.length = 0;
    premium = false;
    await assert.rejects(() => join(friend, live), /premium/);
    assert.deepEqual(calls, ["launch", "premium", "premium modal"]);
    assert.throws(() => join(outsider, { ...live, party: { id: "bad" } }), /geçersiz/);
});

test("slow native joins do not block bridge polling and async errors stay sanitized", async () => {
    let reject;
    const operation = new Promise((_resolve, failure) => reject = failure);
    const controls = new SpotifyControls();
    const provider = fixture({ join: () => operation });
    await controls.execute(provider, "spotify_join", { userId: friend }, now);
    assert.equal(controls.waiting, true);
    reject(new Error("accessToken=private"));
    await Promise.resolve();
    assert.equal(controls.waiting, false);
    assert.match(controls.state(provider, now).result, /kabul etmedi/);
    assert.equal(JSON.stringify(controls.state(provider, now)).includes("private"), false);
    assert.equal(controls.observe(provider, now), true);
    assert.equal(controls.observe(provider, now), false);
});

test("late native rejection cannot overwrite a cleared/reconnected session", async () => {
    let reject;
    const controls = new SpotifyControls();
    const provider = fixture({ join: () => new Promise((_resolve, failure) => reject = failure) });
    await controls.execute(provider, "spotify_join", { userId: friend }, now);
    controls.clear();
    reject(new Error("late"));
    await Promise.resolve();
    assert.equal(controls.state(provider, now).result, "");
});
