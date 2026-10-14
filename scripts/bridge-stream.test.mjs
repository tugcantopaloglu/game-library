import assert from "node:assert/strict";
import test from "node:test";
import { StreamControls } from "../integrations/vencord/gameLibraryBridge.desktop/stream.ts";
import { CaptureCatalog, nativeCaptureAdapter, sources } from "../integrations/vencord/gameLibraryBridge.desktop/capture.ts";
import { currentStreamKey, mediaAdapter, qualityAdapter, startAdapter } from "../integrations/vencord/gameLibraryBridge.desktop/streamAdapter.ts";
import { stopAdapter } from "../integrations/vencord/gameLibraryBridge.desktop/streamAdapter.ts";

test("active stream keys do not depend on an RTC snapshot and never target another account", () => {
    const owner="123456789012345678", channel="223456789012345678", guild="323456789012345678";
    assert.equal(currentStreamKey({ownerId:owner,channelId:channel,guildId:guild,streamType:"guild"},owner),`guild:${guild}:${channel}:${owner}`);
    assert.equal(currentStreamKey({ownerId:owner,channelId:channel,guildId:null,streamType:"call"},owner),`call:${channel}:${owner}`);
    assert.equal(currentStreamKey({ownerId:owner,channelId:channel,guildId:guild,streamType:"guild"},channel),"");
    assert.equal(currentStreamKey({ownerId:owner,channelId:channel,streamType:"unknown"},owner),"");
});

test("native catalog IDs survive enumeration without preview images", async () => {
    const calls=[];
    const capture=nativeCaptureAdapter({
        async getScreenPreviews(...args){calls.push(args);return [{id:"screen-handle:327932",name:"Screen 1",url:"private pixels"}];},
        async getWindowPreviews(...args){calls.push(args);return [{id:"window:919562",name:"Window",url:"private pixels"}];}
    });
    const raw=await capture();
    assert.deepEqual(calls,[[0,0],[0,0]]);
    assert.equal(JSON.stringify(raw).includes("private pixels"),false);
    const catalog=sources(raw);
    assert.equal(catalog.find(s=>s.kind==="screen").id,"screen-handle:327932");
    assert.equal(catalog.find(s=>s.kind==="window").id,"window:919562");
    assert.equal(sources([{id:"screen-handle:bad",name:"Invalid"},{id:"file:///private",name:"Invalid"}]).length,0);
});

test("quality options follow Discord's current permission check rather than the previous maximum", async () => {
    const data=fixture(), control=new StreamControls();
    data.provider.qualityAllowed=(resolution,fps)=>[480,720,1080,1440,0].includes(resolution)&&[5,15,30,60].includes(fps);
    assert.ok(control.state(data.provider).resolutions.includes(1080));
    await control.execute(data.provider,"stream_quality",{streamKey:"live",resolution:1080},0);
    assert.equal(control.observe(data.provider,1),true);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:60},2);
    assert.equal(control.observe(data.provider,3),true);
    data.provider.qualityAllowed=(resolution,fps)=>resolution<=720&&resolution>0&&fps<=30;
    await assert.rejects(control.execute(data.provider,"stream_quality",{streamKey:"live",resolution:1440},4));
});

test("quality permission adapter passes the same native tuple and fails closed", () => {
    let args;
    function check(...values){const canStreamWithSettings="canStreamWithSettings",guildPremiumTier="guildPremiumTier",canStreamQuality="canStreamQuality";args=values;return !!(canStreamWithSettings&&guildPremiumTier&&canStreamQuality);}
    assert.equal(qualityAdapter(check,3)(1080,60,"user",2,"channel"),true);
    assert.deepEqual(args,[3,1080,60,"user",2,"channel"]);
    assert.equal(qualityAdapter(()=>true,3),undefined);
});

test("FPS can return to a permitted value even when current FPS leaves no resolution choices", async () => {
    const data=fixture(),control=new StreamControls();
    data.live={...data.live,source:{...data.live.source,quality:{resolution:720,frameRate:120}}};
    data.provider.qualityAllowed=(resolution,fps)=>[480,720].includes(resolution)&&[15,30,60].includes(fps);
    assert.equal(control.state(data.provider).canQuality,false);
    assert.equal(control.state(data.provider).canFps,true);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:30},0);
    assert.equal(control.observe(data.provider,1),true);
    assert.equal(control.state(data.provider).canQuality,true);
});

test("native source IDs support the complete start, quality, FPS, source change and stop flow", async () => {
    const data=fixture(), control=new StreamControls();
    data.live=null;
    Object.assign(data.provider,{
        account:"owner",channel:"room",
        listSources:async()=>[{id:"screen-handle:327932",name:"Screen 1"},{id:"window:919562",name:"Test window"}],
        qualityAllowed:(resolution,fps)=>[0,480,720,1080].includes(resolution)&&[15,30,60].includes(fps),
        start:(_channel,source)=>{data.live={key:"live",context:"stream",sourceName:"Old metadata",source:{desktopSource:{id:source.id,sourcePid:null,soundshareId:null,soundshareSession:null},quality:{resolution:720,frameRate:30}}};}
    });
    await control.catalog.open(data.provider,0);
    await control.execute(data.provider,"stream_start",{streamKey:"",channelId:"room",sourceRevision:control.state(data.provider).sourceRevision,sourceId:"screen-handle:327932",audio:false},1);
    assert.equal(control.observe(data.provider,2),true);
    assert.equal(control.state(data.provider).sourceName,"Screen 1");
    await control.execute(data.provider,"stream_quality",{streamKey:"live",resolution:1080},3);
    assert.equal(control.observe(data.provider,4),true);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:60},5);
    assert.equal(control.observe(data.provider,6),true);
    await control.catalog.open(data.provider,7);
    await control.execute(data.provider,"stream_source",{streamKey:"live",channelId:"room",sourceRevision:control.state(data.provider).sourceRevision,sourceId:"window:919562"},8);
    assert.equal(control.observe(data.provider,9),true);
    assert.equal(control.state(data.provider).sourceName,"Test window");
    assert.deepEqual(data.live.source.quality,{resolution:1080,frameRate:60});
    await control.execute(data.provider,"stream_stop",{streamKey:"live"},10);
    assert.equal(control.observe(data.provider,11),true);
    assert.equal(control.state(data.provider).active,false);
});

test("a timed out update never overwrites a quality change made in Discord", async () => {
    const data=fixture(),control=new StreamControls();
    data.provider.setSource=source=>data.writes.push(source);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:15},0);
    data.live={...data.live,source:{...data.live.source,quality:{resolution:1080,frameRate:60}}};
    assert.equal(control.observe(data.provider,5000),true);
    assert.equal(data.writes.length,1);
    assert.deepEqual(data.live.source.quality,{resolution:1080,frameRate:60});
});

test("Discord action recognition tolerates formatting and default parameters", () => {
    let received;
    function start(guildId,channelId,options = {}) { const event={type:'STREAM_START',streamType: 'guild',guildId,channelId};received=[event,options]; }
    startAdapter(start)("guild","room",{id:"screen:0:0",name:"Screen"});
    assert.equal(received[1].sourceId,"screen:0:0");
    const media={setGoLiveSource(options) {const event={type:'MEDIA_ENGINE_SET_GO_LIVE_SOURCE',qualityOptions: options.qualityOptions};received=options;return event;}};
    mediaAdapter(media,3)({desktopSource:{id:"screen:0:0",soundshareId:null},quality:{resolution:720,frameRate:30}},"stream");
    assert.equal(received.desktopSettings.sourceId,"screen:0:0");
    stopAdapter(function stop(streamKey) {received={type:'STREAM_STOP', streamKey};})("live");
    assert.equal(received.streamKey,"live");
});
import { encodeSnapshot } from "../integrations/vencord/gameLibraryBridge.desktop/state.ts";

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
    assert.equal(data.writes.length,1);
    await control.execute(data.provider,"stream_fps",{streamKey:"live",fps:15},6000);
    data.live = {...data.live,key:"replacement"};
    assert.equal(control.observe(data.provider,11000),true);
    assert.equal(data.writes.length,2);
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
    mediaAdapter(media,3)(fixture().live.source,"stream");
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
    assert.equal(control.state(data.provider).sourceName,"Screen");
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

test("stream audio preserves the picture and accepts a zero soundshare identifier", async () => {
    const data = fixture(), control = new StreamControls();
    data.provider.audioSupported = ()=>true;
    data.provider.setSource = (source,context,audio)=>{data.writes.push(source);data.live = {...data.live,source:{...source,desktopSource:{...source.desktopSource,soundshareId:audio ? 0:null}}};};
    await control.execute(data.provider,"stream_audio",{streamKey:"live",audio:true},0);
    assert.equal(control.observe(data.provider,1),true); assert.equal(control.state(data.provider).audio,true);
    assert.equal(data.live.source.desktopSource.id,"window:1:0"); assert.deepEqual(data.live.source.quality,{resolution:720,frameRate:30});
    await control.execute(data.provider,"stream_audio",{streamKey:"live",audio:false},2);
    assert.equal(control.observe(data.provider,3),true); assert.equal(control.state(data.provider).audio,false);
});

test("unsupported audio and unconfirmed starts cannot report success", async () => {
    const data = fixture(), control = new StreamControls();
    data.provider.audioSupported = ()=>false;
    await assert.rejects(control.execute(data.provider,"stream_audio",{streamKey:"live",audio:true},0)); assert.equal(data.writes.length,0);
    data.live = null;
    Object.assign(data.provider,{account:"first",channel:"room",listSources:async()=>[{id:"screen:0:0",name:"Screen"}],start:()=>{}});
    await control.catalog.open(data.provider,0); const revision = control.state(data.provider).sourceRevision;
    await assert.rejects(control.execute(data.provider,"stream_start",{streamKey:"",channelId:"room",sourceId:"screen:0:0",sourceRevision:revision,audio:true},1));
    await control.execute(data.provider,"stream_start",{streamKey:"",channelId:"room",sourceId:"screen:0:0",sourceRevision:revision,audio:false},2);
    assert.equal(control.observe(data.provider,8002),true); assert.equal(control.state(data.provider).result,"Discord yayının başladığını doğrulamadı");
});

test("large source catalogs keep the IPC budget and favorite data intact", () => {
    const payload = {type:"snapshot",friends:[{id:"favorite",activity:"x".repeat(230000)}],favoriteIds:["favorite"],streamControl:{sourceRevision:2,sources:Array.from({length:100},(_,i)=>({id:`window:${i}:0`,name:"😀".repeat(128)}))}};
    const encoded = encodeSnapshot(payload,text=>Buffer.byteLength(text,"utf8"));
    assert.ok(encoded); assert.ok(Buffer.byteLength(encoded,"utf8") + 1 <= 256 * 1024);
    const packet = JSON.parse(encoded); assert.equal(packet.friends[0].activity.length,230000); assert.equal(packet.streamControl.sourceRevision,2);
    assert.ok(packet.streamControl.sources.length < 100); assert.equal(payload.streamControl.sources.length,100);
});
