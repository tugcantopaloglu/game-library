import assert from "node:assert/strict";
import test from "node:test";
import { BroadcastControls, broadcastAdapter } from "../integrations/vencord/gameLibraryBridge.desktop/broadcast.ts";

const account = "123456789012345678";
const device = (id, name, index = 1) => ({ id, name, index, disabled: false });
function fixture() {
    const state = { input: "physical-mic", output: "physical-speaker", inputs: { "nv-mic": device("nv-mic", "Mikrofon (NVIDIA Broadcast)") }, outputs: { "nv-speaker": device("nv-speaker", "Speakers (NVIDIA Broadcast)") }, writes: [] };
    const provider = { account, store: { getInputDevices: () => state.inputs, getOutputDevices: () => state.outputs, getInputDeviceId: () => state.input, getOutputDeviceId: () => state.output }, selector: { setInputDevice: id => state.writes.push(["input", id]), setOutputDevice: id => state.writes.push(["output", id]) } };
    return { state, provider, controls: new BroadcastControls() };
}

test("only a known Discord ID dispatch contract enables the action adapter", () => {
    const media = {
        calls: [], getInputDeviceId: () => "before-input", getOutputDeviceId: () => "before-output",
        dispatch(data) { this.calls.push(data); },
        setInputDevice(id) { this.dispatch({ type: "AUDIO_SET_INPUT_DEVICE", id: id, oldId: this.getInputDeviceId() }); },
        setOutputDevice(id) { this.dispatch({ type: "AUDIO_SET_OUTPUT_DEVICE", id: id, oldId: this.getOutputDeviceId() }); }
    };
    const adapter = broadcastAdapter(media);
    assert.ok(adapter);
    adapter.setInputDevice("nv-mic"); adapter.setOutputDevice("nv-speaker");
    assert.deepEqual(media.calls.map(call => call.id), ["nv-mic", "nv-speaker"]);
    assert.equal(broadcastAdapter({ setInputDevice(id) {}, setOutputDevice(id) {} }), undefined);
    assert.equal(broadcastAdapter({ ...media, setInputDevice(id, context) {} }), undefined);
});

test("native defaults and unrelated NVIDIA devices never become Broadcast targets", () => {
    const { state, provider, controls } = fixture();
    state.inputs = { default: device("default", "Default (NVIDIA Broadcast)", -1), physical: device("physical", "NVIDIA High Definition Audio") };
    assert.equal(controls.state(provider).canSelect, false);
    state.inputs = { "nv-mic": { ...device("nv-mic", "NVIDIA Broadcast"), disabled: true } };
    assert.equal(controls.state(provider).microphone, null);
    state.inputs = { "nv-mic": { ...device("some-other-id", "NVIDIA Broadcast") } };
    assert.equal(controls.state(provider).canSelect, false);
});

test("missing readback or actions disable selection before any mutation", async () => {
    const { provider, state, controls } = fixture();
    delete provider.store.getOutputDeviceId;
    await assert.rejects(() => controls.execute(provider, "broadcast_audio", {}), /seçilemiyor/);
    assert.deepEqual(state.writes, []);
    const second = fixture(); delete second.provider.selector.setOutputDevice;
    assert.equal(second.controls.state(second.provider).supported, false);
});

test("duplicate Broadcast devices require an already selected unambiguous target", () => {
    const { state, provider, controls } = fixture();
    state.inputs["nv-mic-duplicate"] = device("nv-mic-duplicate", "NVIDIA Broadcast");
    assert.equal(controls.state(provider).canSelect, false);
    state.input = "nv-mic";
    assert.equal(controls.state(provider).microphone.id, "nv-mic");
});

test("selection accepts no arbitrary UI-supplied device IDs", async () => {
    const { controls, provider, state } = fixture();
    await assert.rejects(() => controls.execute(provider, "broadcast_audio", { inputId: "other" }), /geçersiz/);
    await assert.rejects(() => controls.execute(provider, "other_action", {}), /geçersiz/);
    assert.deepEqual(state.writes, []);
});

test("both exact Discord getters must confirm the targets before reporting success", async () => {
    const { controls, provider, state } = fixture();
    await controls.execute(provider, "broadcast_audio", {}, 100);
    assert.deepEqual(state.writes, [["input", "nv-mic"], ["output", "nv-speaker"]]);
    assert.equal(controls.waiting, true);
    assert.equal(controls.state(provider).selected, false);
    state.input = "nv-mic";
    assert.equal(controls.observe(provider, 200), false);
    state.output = "nv-speaker";
    assert.equal(controls.observe(provider, 300), true);
    assert.equal(controls.waiting, false);
    assert.match(controls.state(provider).result, /doğrulandı/);
});

test("already selected Broadcast devices require no redundant audio changes", async () => {
    const { controls, provider, state } = fixture();
    state.input = "nv-mic"; state.output = "nv-speaker";
    await controls.execute(provider, "broadcast_audio", {}, 100);
    assert.deepEqual(state.writes, []);
    assert.equal(controls.waiting, false);
    assert.match(controls.state(provider).result, /zaten seçili/);
});

test("pending selection rejects repetition and expires without invented success", async () => {
    const { controls, provider } = fixture();
    await controls.execute(provider, "broadcast_audio", {}, 100);
    await assert.rejects(() => controls.execute(provider, "broadcast_audio", {}, 101), /bekleniyor/);
    assert.equal(controls.observe(provider, 10100), true);
    assert.match(controls.state(provider).result, /doğrulamadı/);
});

test("account changes and device removal invalidate the pending selection", async () => {
    const first = fixture();
    await first.controls.execute(first.provider, "broadcast_audio", {}, 100);
    first.provider.account = "223456789012345678";
    assert.equal(first.controls.observe(first.provider, 200), true);
    assert.match(first.controls.state(first.provider).result, /oturumu değişti/);
    const second = fixture();
    await second.controls.execute(second.provider, "broadcast_audio", {}, 100);
    second.state.outputs = {};
    assert.equal(second.controls.observe(second.provider, 200), true);
    assert.match(second.controls.state(second.provider).result, /listesi değişti/);
});

test("a lost device between the two writes stops before output selection", async () => {
    const { provider, controls, state } = fixture();
    provider.selector.setInputDevice = id => { state.writes.push(["input", id]); state.outputs = {}; };
    await assert.rejects(() => controls.execute(provider, "broadcast_audio", {}), /tamamlamadı/);
    assert.deepEqual(state.writes, [["input", "nv-mic"]]);
});

test("exceptions cannot expose arbitrary client strings in the bridge snapshot", async () => {
    const { provider, controls } = fixture();
    provider.selector.setOutputDevice = () => { throw new Error("private-token=secret"); };
    await assert.rejects(() => controls.execute(provider, "broadcast_audio", {}), /tamamlamadı/);
    assert.equal(controls.waiting, false);
    assert.equal(JSON.stringify(controls.state(provider)).includes("secret"), false);
    provider.store.getInputDevices = () => { throw new Error("secret"); };
    assert.equal(controls.state(provider).supported, false);
    Object.defineProperty(provider.store, "getInputDevices", { get() { throw new Error("secret"); } });
    assert.equal(controls.state(provider).supported, false);
});

test("clearing the bridge between the two writes prevents a later stale output write", async () => {
    const { provider, controls, state } = fixture();
    provider.selector.setInputDevice = id => { state.writes.push(["input", id]); controls.clear(); };
    await assert.rejects(() => controls.execute(provider, "broadcast_audio", {}), /tamamlamadı/);
    assert.deepEqual(state.writes, [["input", "nv-mic"]]);
});
