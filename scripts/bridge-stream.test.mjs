import assert from "node:assert/strict";
import test from "node:test";
import { StreamControls } from "../integrations/vencord/gameLibraryBridge.desktop/stream.ts";
import { CaptureCatalog, sources } from "../integrations/vencord/gameLibraryBridge.desktop/capture.ts";
import { mediaAdapter, startAdapter } from "../integrations/vencord/gameLibraryBridge.desktop/streamAdapter.ts";

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

test("capture metadata is bounded and closing the picker discards delayed results", async () => {
    const metadata = sources(Array.from({length:110},(_,i)=>({id:`window:${i}:0`,name:`Window ${i}`,url:"large-preview"})));
    assert.equal(metadata.length,100); assert.equal("url" in metadata[0],false);
    assert.deepEqual(sources([{id:"file:///secret",name:"bad"}]),[]);
    let finish;
    const provider = {account:"first",channel:"room",listSources:()=>new Promise(resolve=>{finish=resolve;})};
    const catalog = new CaptureCatalog(); const pending = catalog.open(provider,0);
    await Promise.resolve(); catalog.clear(); finish([{id:"window:1:0",name:"Game"}]); await pending;
    assert.equal(catalog.state(provider).sourceOpen,false); assert.equal(catalog.state(provider).sources.length,0);
});

test("start revalidates the chosen source and confirms the actual stream", async () => {
    let live = null, starts = 0;
    const provider = {account:"first",channel:"room",known:true,read:()=>live,listSources:async()=>[{id:"window:1:0",name:"Game"}],start:()=>{starts++;}};
    const control = new StreamControls(); await control.catalog.open(provider,0);
    const state = control.state(provider), data = {streamKey:"",channelId:"room",sourceRevision:state.sourceRevision,sourceId:"window:1:0"};
    await assert.rejects(control.execute(provider,"stream_start",{...data,channelId:"other"},1));
    await control.execute(provider,"stream_start",data,1); assert.equal(starts,1); assert.equal(control.observe(provider,2),false);
    live = fixture().live; assert.equal(control.observe(provider,3),true); assert.equal(control.state(provider).result,"Yayın başlatıldığı doğrulandı");
    await assert.rejects(control.execute(provider,"stream_start",data,4)); assert.equal(starts,1);
});

test("a disappeared source and a changed account never start a stream", async () => {
    let entries = [{id:"window:1:0",name:"Game"}], starts = 0;
    const provider = {account:"first",channel:"room",read:()=>null,listSources:async()=>entries,start:()=>{starts++;}};
    const control = new StreamControls(); await control.catalog.open(provider,0);
    const data = {streamKey:"",channelId:"room",sourceRevision:control.state(provider).sourceRevision,sourceId:"window:1:0"};
    entries = [{id:"window:1:0",name:"Different window"}]; await assert.rejects(control.execute(provider,"stream_start",data,1));
    provider.account = "second"; await assert.rejects(control.execute(provider,"stream_start",data,2)); assert.equal(starts,0);
});

test("current Discord action contracts receive settings and unknown contracts stay disabled", () => {
    const calls = [];
    const media = {setGoLiveSource(settings) { if (settings?.qualityOptions) calls.push(settings.desktopSettings); calls.push({type:"MEDIA_ENGINE_SET_GO_LIVE_SOURCE",settings:settings}); }};
    mediaAdapter(media)(fixture().live.source,"stream");
    assert.deepEqual(calls[0],{sourceId:"window:1:0",sound:false});
    assert.equal(calls[1].settings.context,"stream"); assert.equal(calls[1].settings.qualityOptions.resolution,720);
    assert.equal(mediaAdapter({setGoLiveSource(){}}),undefined); assert.equal(startAdapter(()=>{}),undefined);
    function start(guild,channel,options) { calls.push({type:"STREAM_START",streamType:guild ? "guild":"call",guildId:guild,channelId:channel,...options}); }
    startAdapter(start)(null,"room",{id:"screen:0:0",name:"Screen"});
    assert.equal(calls[2].sourceId,"screen:0:0"); assert.equal(calls[2].sound,false);
});

test("source changes preserve quality and confirm only the selected source", async () => {
    const data = fixture(), control = new StreamControls();
    Object.assign(data.provider,{account:"first",channel:"room",listSources:async()=>[{id:"screen:0:0",name:"Screen"}]});
    await control.catalog.open(data.provider,0);
    await control.execute(data.provider,"stream_source",{streamKey:"live",channelId:"room",sourceRevision:control.state(data.provider).sourceRevision,sourceId:"screen:0:0"},1);
    assert.deepEqual(data.live.source.quality,{resolution:720,frameRate:30}); assert.equal(control.observe(data.provider,2),true);
    assert.equal(data.live.source.desktopSource.id,"screen:0:0"); assert.equal(control.state(data.provider).sourceOpen,false);
});

test("replacing a stream during source validation leaves the replacement untouched", async () => {
    const data = fixture(), control = new StreamControls(); let finish;
    Object.assign(data.provider,{account:"first",channel:"room",listSources:async()=>[{id:"screen:0:0",name:"Screen"}]});
    await control.catalog.open(data.provider,0);
    const revision = control.state(data.provider).sourceRevision;
    data.provider.listSources = ()=>new Promise(resolve=>{finish=resolve;});
    const pending = control.execute(data.provider,"stream_source",{streamKey:"live",channelId:"room",sourceRevision:revision,sourceId:"screen:0:0"},1);
    await Promise.resolve(); data.live = {...data.live,key:"replacement"}; finish([{id:"screen:0:0",name:"Screen"}]);
    await assert.rejects(pending); assert.equal(data.writes.length,0);
});
