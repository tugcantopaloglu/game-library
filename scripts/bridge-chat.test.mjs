import assert from "node:assert/strict";
import test from "node:test";
import { CHAT_BYTE_LIMIT, ChatControls, chatMessages, chatMessageAdapters, directMessageAdapter, discordFocusAdapter, oneToOneChannel, validContent } from "../integrations/vencord/gameLibraryBridge.desktop/directMessage.ts";

const now = 1800000000000;
const account = "123456789012345678", friend = "223456789012345678", outsider = "323456789012345678", channelId = "423456789012345678";
const messageId = (offset = 0) => (((BigInt(now + offset) - 1420070400000n) << 22n) | 1n).toString();
const nonce = messageId(1);
function fixture(overrides = {}) {
    const channel = { id: channelId, type: 1, recipients: [friend] };
    const messages = { ready: true, _array: [] };
    return { account, friendIds: () => [friend], dmChannelId: () => channelId, channel: () => channel, messages: () => messages, nonce: () => nonce, selectedChannel: () => channelId, ...overrides };
}
const row = (content = "Hello", options = {}) => ({ id: messageId(), channel_id: channelId, author: { id: friend, username: "Friend" }, content, state: "SENT", ...options });

test("open adapter passes one recipient inside the current native options object", async () => {
    let received, receiver;
    const actions = { async openPrivateChannel(e) { const {recipientIds: ids, navigateToChannel: navigate, joinCall: call} = e; this._getRecipients(ids); received = e; receiver = this; return channelId; }, _getRecipients: ids => ids };
    const open = directMessageAdapter(actions);
    assert.equal(typeof open, "function");
    await open(friend);
    assert.equal(receiver, actions);
    assert.deepEqual(received, { recipientIds: [friend], joinCall: false, joinCallVideo: false, navigateToChannel: true });
    assert.equal(directMessageAdapter({openPrivateChannel: id => id}), undefined);
    assert.throws(() => open("invalid"), /geçersiz/);
});

test("message adapters use bounded fetch and four-argument native send with a nonce", async () => {
    let fetchArgs, sendArgs;
    const actions = {
        fetchMessages(e) { const {channelId: channel, limit: count} = e; const marker = "LOAD_MESSAGES_SUCCESS"; fetchArgs = e; return marker && channel && count; },
        sendMessage(channel, message, wait, options) { const {nonce} = options; const whenReady = null; const _sendMessage = null; sendArgs = [channel,message,wait,options]; return nonce && whenReady && _sendMessage; }
    };
    const adapters = chatMessageAdapters(actions);
    adapters.fetch(channelId, 30);
    assert.deepEqual(fetchArgs, { channelId, limit: 30, skipLocalFetch: true });
    adapters.send(channelId, "hello", nonce);
    assert.equal(sendArgs[0], channelId);
    assert.deepEqual(sendArgs[1], {content:"hello",invalidEmojis:[],validNonShortcutEmojis:[],tts:false});
    assert.equal(sendArgs[2], false);
    assert.deepEqual(sendArgs[3], {nonce,allowedMentions:{parse:[],replied_user:false}});
    assert.equal(chatMessageAdapters({}).send, undefined);
    assert.throws(() => adapters.fetch(channelId, 1000), /geçersiz/);
});

test("native focus wrapper preserves its verified window argument order", () => {
    const calls = [];
    const g = { window: { restore: id => calls.push(["restore",id]), focus: (force,id) => calls.push(["focus",force,id]) } };
    const native = { restore(e){g.window.restore(e)}, focus(e){let t=arguments.length>1&&void 0!==arguments[1]&&arguments[1];g.window.focus(t,e)} };
    discordFocusAdapter(native)();
    assert.deepEqual(calls, [["restore",undefined],["focus",false,undefined]]);
    assert.equal(discordFocusAdapter({focus(){}}), undefined);
});

test("only exact one-to-one DM channels are accepted", () => {
    assert.equal(oneToOneChannel({id:channelId,type:1,recipients:[friend]}, friend, account), channelId);
    for (const channel of [{id:channelId,type:3,recipients:[friend]}, {id:channelId,type:1,recipients:[friend,outsider]}, {id:channelId,type:1,recipients:[]}, {id:channelId,type:1,recipients:[outsider]}, {id:channelId,type:1,guild_id:outsider,recipients:[friend]}]) assert.equal(oneToOneChannel(channel, friend, account), null);
});

test("cached DM opens inside Game Library without another native channel action", async () => {
    const controls = new ChatControls(), provider = fixture({openDm: () => assert.fail("cached DM must be reused")});
    await controls.execute(provider, "chat_open", {userId:friend}, now);
    const state = controls.state(provider, now);
    assert.equal(state.userId, friend);
    assert.equal(state.channelId, channelId);
    assert.equal(state.loading, false);
    assert.equal(state.pending, false);
    assert.equal(state.sentNonce, null);
});

test("slow native DM opening is nonblocking and rejects a returned group channel", async () => {
    let finish;
    const controls = new ChatControls(), provider = fixture({dmChannelId:()=>undefined,openDm:()=>new Promise(resolve=>finish=resolve)});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    assert.equal(controls.state(provider,now).loading,true);
    provider.channel=()=>({id:channelId,type:3,recipients:[friend]});
    finish(channelId); await Promise.resolve();
    assert.equal(controls.state(provider,now).userId,"");
    assert.match(controls.state(provider,now).status,/doğrulamadı/);
});

test("closed or account-switched pending opens cannot populate the new conversation", async () => {
    for (const close of [true,false]) {
        let finish;
        const controls = new ChatControls(), provider = fixture({dmChannelId:()=>undefined,openDm:()=>new Promise(resolve=>finish=resolve)});
        await controls.execute(provider,"chat_open",{userId:friend},now);
        if(close) await controls.execute(provider,"chat_close",{},now); else provider.account=outsider;
        finish(channelId); await Promise.resolve();
        assert.equal(controls.state(provider,now).userId,"");
        assert.deepEqual(controls.state(provider,now).messages,[]);
    }
});

test("selected chat exposes only its final messages and bounded text", async () => {
    const controls = new ChatControls(), provider = fixture();
    const entries = [];
    for(let i=0;i<100;i++) entries.push(row("😀".repeat(4000),{id:messageId(i)}));
    entries.push(row("other channel",{channel_id:outsider}));
    entries.push(row("optimistic",{state:"SENDING"}));
    provider.messages=()=>({ready:true,_array:entries,accessToken:"do-not-export"});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    const messages=controls.state(provider,now).messages;
    assert.ok(messages.length<=30);
    assert.ok(new TextEncoder().encode(JSON.stringify(messages)).byteLength<=CHAT_BYTE_LIMIT);
    assert.equal(messages.some(message=>message.content==="other channel"||message.content==="optimistic"),false);
    assert.equal(JSON.stringify(controls.state(provider,now)).includes("do-not-export"),false);
    assert.deepEqual(chatMessages(provider,channelId,friend,outsider,now),[]);
});

test("send requires the exact selected friend, channel, account and revision", async () => {
    const controls = new ChatControls(), provider = fixture({send:()=>assert.fail("invalid send must not run")});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    for(const bad of [{userId:outsider,channelId,content:"hi"},{userId:friend,channelId:outsider,content:"hi"},{userId:friend,channelId,content:"hi",accountId:outsider},{userId:friend,channelId,content:"hi",revision:1000}]) await assert.rejects(()=>controls.execute(provider,"chat_send",bad,now),/sohbet|hedef/);
    assert.equal(controls.state(provider,now).pending,false);
});

test("optimistic and wrong nonce messages never confirm sending", async () => {
    let args;
    const controls = new ChatControls(), provider = fixture({send:(...values)=>args=values});
    const entries=[]; provider.messages=()=>({ready:true,_array:entries});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    await controls.execute(provider,"chat_send",{userId:friend,channelId,content:"hello"},now);
    assert.deepEqual(args,[channelId,"hello",nonce]);
    assert.equal(controls.state(provider,now).pending,true);
    assert.equal(controls.state(provider,now).sentNonce,null);
    entries.push(row("hello",{author:{id:account,username:"Me"},nonce,state:"SENDING"}));
    controls.observe(provider,now+1);
    assert.equal(controls.state(provider,now).pending,true);
    entries[0].state="SENT"; entries[0].nonce=messageId(2);
    controls.observe(provider,now+2);
    assert.equal(controls.state(provider,now).pending,true);
    entries[0].nonce=nonce;
    controls.observe(provider,now+3);
    assert.equal(controls.state(provider,now).pending,false);
    assert.equal(controls.state(provider,now).sentNonce,nonce);
    assert.equal(controls.state(provider,now).messages[0].own,true);
});

test("double-click, failed sends and timeouts never trigger automatic resending", async () => {
    let sends=0;
    const controls=new ChatControls(),provider=fixture({send:()=>sends++});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    const request={userId:friend,channelId,content:"hello"};
    await controls.execute(provider,"chat_send",request,now);
    await assert.rejects(()=>controls.execute(provider,"chat_send",request,now+1),/bekleniyor/);
    controls.observe(provider,now+30001);
    assert.equal(sends,1);
    assert.equal(controls.state(provider,now).sentNonce,null);
    assert.match(controls.state(provider,now).status,/tekrar gönderilmedi/);
});

test("late rejected sends cannot change a closed chat or leak error contents", async () => {
    let reject;
    const controls=new ChatControls(),provider=fixture({send:()=>new Promise((_resolve,failure)=>reject=failure)});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    await controls.execute(provider,"chat_send",{userId:friend,channelId,content:"hello"},now);
    await controls.execute(provider,"chat_close",{},now);
    reject(new Error("private token")); await Promise.resolve();
    assert.equal(controls.state(provider,now).pending,false);
    assert.equal(controls.state(provider,now).sentNonce,null);
    assert.equal(JSON.stringify(controls.state(provider,now)).includes("private token"),false);
});

test("account switch and friend removal clear messages and disable sending", async () => {
    for(const change of [provider=>provider.account=outsider,provider=>provider.friendIds=()=>[]]) {
        const controls=new ChatControls(),provider=fixture({messages:()=>({ready:true,_array:[row("secret")]}),send:()=>{}});
        await controls.execute(provider,"chat_open",{userId:friend},now);
        assert.equal(controls.state(provider,now).messages.length,1);
        change(provider); controls.observe(provider,now+1);
        assert.deepEqual(controls.state(provider,now).messages,[]);
        assert.equal(controls.state(provider,now).canSend,false);
    }
});

test("refresh is bounded, nonblocking and late completion cannot replace another chat", async () => {
    let finish,count=0;
    const controls=new ChatControls(),provider=fixture({fetch:(id,limit)=>{assert.equal(id,channelId);assert.equal(limit,30);count++;return new Promise(resolve=>finish=resolve)}});
    await controls.execute(provider,"chat_open",{userId:friend},now);
    await controls.execute(provider,"chat_refresh",{},now);
    await controls.execute(provider,"chat_refresh",{},now+1);
    assert.equal(count,1);
    assert.equal(controls.state(provider,now).loading,true);
    await controls.execute(provider,"chat_close",{},now);
    finish(true);await Promise.resolve();
    assert.equal(controls.state(provider,now).loading,false);
    assert.equal(controls.state(provider,now).userId,"");
});

test("loading chat can close with exact scope and queued stale opens are rejected", async () => {
    const controls=new ChatControls(),provider=fixture({dmChannelId:()=>undefined,openDm:()=>new Promise(()=>{})});
    const before=controls.state(provider,now);
    assert.ok(before.revision>0);
    assert.equal(before.accountId,account);
    await controls.execute(provider,"chat_open",{userId:friend,accountId:account,revision:before.revision},now);
    const current=controls.state(provider,now);
    await assert.rejects(()=>controls.execute(provider,"chat_open",{userId:friend,revision:before.revision},now),/değişti/);
    await controls.execute(provider,"chat_close",{userId:friend,channelId:"",accountId:account,revision:current.revision},now);
    assert.equal(controls.state(provider,now).loading,false);
});

test("empty, oversized, ill-formed and control-filled sends are rejected", () => {
    assert.equal(validContent("Hello\nworld"),true);
    for(const invalid of ["", " ", "x".repeat(2001),"hello\u0000", "\ud800"]) assert.equal(validContent(invalid),false);
    assert.equal(validContent("hello\tworld\r\n"),true);
    assert.equal(validContent("🦊".repeat(2000)),true);
    assert.equal(validContent("🦊".repeat(2001)),false);
});
