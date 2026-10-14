import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";
import vm from "node:vm";
const source = await readFile(new URL("../integrations/steam/owned-library.js",import.meta.url),"utf8");
function run({host="store.steampowered.com",loggedIn=true,response={game_count:1,games:[{appid:570,name:"Dota 2",playtime_forever:9000}]},status=200}={}) {
    const requests=[];
    const context={location:{protocol:"https:",hostname:host},document:{getElementById:()=>({getAttribute:name=>JSON.stringify(name==="data-userinfo"?{logged_in:loggedIn,steamid:"76561198000000001"}:{webapi_token:"private-session-secret"})})},URLSearchParams,AbortSignal,TextDecoder,fetch:async address=>{requests.push(address);return new Response(JSON.stringify({response}),{status});}};
    return {requests,promise:vm.runInNewContext(source,context)};
}
test("Steam login exports only owned catalog metadata, never browser credentials",async()=>{
    const fixture=run();const result=await fixture.promise;
    assert.equal(result.kind,"games");assert.equal(result.response.games[0].appid,570);
    assert.equal(JSON.stringify(result).includes("private-session-secret"),false);
    assert.equal(JSON.stringify(result).includes("playtime_forever"),false);
    assert.equal(new URL(fixture.requests[0]).hostname,"api.steampowered.com");
});
test("untrusted pages and logged-out browsers never request account data",async()=>{
    for(const [data,kind] of [[{host:"store.steampowered.com.evil.test"},"waiting"],[{loggedIn:false},"login_required"]]) {const fixture=run(data);assert.equal((await fixture.promise).kind,kind);assert.equal(fixture.requests.length,0);}
});
test("incomplete or denied responses cannot erase an existing account library",async()=>{
    for(const data of [{response:{}},{response:{game_count:2,games:[]}},{status:401}]) assert.equal((await run(data).promise).kind,"request_failed");
    assert.equal((await run({response:{game_count:0}}).promise).kind,"games");
});

test("missing store token moves to the authenticated community games catalog",async()=>{
    let destination;
    const context={location:{protocol:"https:",hostname:"store.steampowered.com",assign:url=>destination=url},document:{getElementById:()=>({getAttribute:name=>JSON.stringify(name==="data-userinfo"?{logged_in:true,steamid:"76561198000000001"}:{})})}};
    assert.equal((await vm.runInNewContext(source,context)).kind,"navigating");
    assert.equal(destination,"https://steamcommunity.com/profiles/76561198000000001/games/?tab=all");
});
test("community fallback exports the complete own catalog and rejects another account",async()=>{
    const context={location:{protocol:"https:",hostname:"steamcommunity.com",pathname:"/profiles/76561198000000001/games/"},g_steamID:"76561198000000001",rgGames:[{appid:"10",name:"Counter-Strike"},{appid:570,name:"Dota 2"}]};
    assert.equal((await vm.runInNewContext(source,context)).response.game_count,2);
    context.g_steamID="76561198000000002";
    assert.equal((await vm.runInNewContext(source,context)).kind,"login_required");
});
