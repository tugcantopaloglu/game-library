import assert from "node:assert/strict";
import test from "node:test";
import { StreamControls } from "../integrations/vencord/gameLibraryBridge.desktop/stream.ts";

function fixture() {
    let live = {key:"live",context:"stream",sourceName:"Game",source:{desktopSource:{id:"window:1:0",sourcePid:1,soundshareId:null,soundshareSession:null},quality:{resolution:720,frameRate:30}}};
    const writes = [];
    return {get live(){return live;},set live(value){live=value;},writes,provider:{read:()=>live,setSource:source=>{writes.push(source);live={...live,source};},stop:()=>{live=null;}}};
}

test("stream updates require an observed option and confirm only on readback", async () => {
    const fixtureData = fixture(); const control = new StreamControls();
    await assert.rejects(control.execute(fixtureData.provider,"stream_quality",{streamKey:"live",resolution:1080},0));
    await assert.rejects(control.execute(fixtureData.provider,"stream_fps",{streamKey:"other",fps:15},0));
    assert.equal(fixtureData.writes.length,0);
    await control.execute(fixtureData.provider,"stream_quality",{streamKey:"live",resolution:480},0);
    assert.equal(control.state(fixtureData.provider).pending,true);
    assert.equal(control.observe(fixtureData.provider,1),true);
    assert.equal(control.state(fixtureData.provider).pending,false);
    assert.equal(fixtureData.live.source.quality.resolution,480);
});

test("an ignored update times out without reporting success; a replacement stream is untouched", async () => {
    const data = fixture(); const control = new StreamControls();
    data.provider.setSource = source=>data.writes.push(source);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:15},0);
    assert.equal(control.observe(data.provider,4999),false);
    assert.equal(control.observe(data.provider,5000),true);
    assert.equal(control.state(data.provider).result,"Discord değişikliği doğrulamadı");
    assert.equal(data.writes.length,2);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:15},6000);
    data.live = {...data.live,key:"replacement"};
    assert.equal(control.observe(data.provider,11000),true);
    assert.equal(data.writes.length,3);
});

test("stop confirms the disappearance of the actual stream and unsupported setters remain disabled", async () => {
    const data = fixture(); const control = new StreamControls();
    assert.equal(control.state({read:data.provider.read}).canQuality,false);
    await control.execute(data.provider,"stream_stop",{streamKey:"live"},0);
    assert.equal(control.observe(data.provider,1),true);
    assert.equal(control.state(data.provider).active,false);
});

test("an account change cannot confirm a stop in the previous session", async () => {
    const data = fixture(); const control = new StreamControls();
    data.provider.account = "first";
    data.provider.stop = ()=>{};
    await control.execute(data.provider,"stream_stop",{streamKey:"live"},0);
    data.provider.account = "second"; data.live = null;
    assert.equal(control.observe(data.provider,1),true);
    assert.equal(control.state(data.provider).result,"Discord oturumu değişti; önceki işlem iptal edildi");
});
