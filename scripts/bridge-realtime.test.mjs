import assert from "node:assert/strict";
import test from "node:test";
import {createServer} from "node:net";
import {mkdtemp,mkdir,writeFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import * as native from "../integrations/vencord/gameLibraryBridge.desktop/native.ts";

test("commands wake the renderer without waiting for the polling timer",async()=>{
    const directory=await mkdtemp(join(tmpdir(),"game-library-realtime-"));
    const server=createServer();let connection;
    const accepted=new Promise(resolve=>server.once("connection",socket=>{connection=socket;socket.once("data",()=>{socket.write('{"type":"welcome","protocol":1}\n');resolve();});}));
    await new Promise(resolve=>server.listen(0,"127.0.0.1",resolve));
    await mkdir(join(directory,"integrations/vencord"),{recursive:true});
    await writeFile(join(directory,"integrations/vencord/bridge.json"),JSON.stringify({protocol:1,port:server.address().port,token:"a".repeat(64)}));
    try {
        native.start(null,directory);await accepted;
        const waiting=native.waitCommands();
        const start=performance.now();
        connection.write('{"type":"command","action":"mute","data":{}}\n');
        const packet=await Promise.race([waiting,new Promise((_,reject)=>setTimeout(()=>reject(new Error("command waited for a timer")),500))]);
        assert.equal(packet.commands[0].action,"mute");assert.ok(performance.now()-start<500);
        const closing=native.waitCommands();native.stop();assert.equal((await closing).connected,false);
    } finally {native.stop();connection?.destroy();await new Promise(resolve=>server.close(resolve));await rm(directory,{recursive:true,force:true});}
});
