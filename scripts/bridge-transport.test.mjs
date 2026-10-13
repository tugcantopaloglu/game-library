import assert from "node:assert/strict";
import test from "node:test";
import { configurationPaths, parseConfiguration } from "../integrations/vencord/gameLibraryBridge.desktop/transport.ts";

test("packaged and standalone profiles can reconnect without accepting invalid credentials", () => {
    const paths = configurationPaths("C:/Packaged/GameLibrary", "C:/Local");
    assert.equal(paths.length, 2);
    assert.ok(paths[0].endsWith("bridge.json"));
    assert.equal(configurationPaths("C:/Local/GameLibrary", "C:/Local").length, 1);
    assert.deepEqual(parseConfiguration(JSON.stringify({protocol:1,port:4455,token:"a".repeat(64)})),{port:4455,token:"a".repeat(64)});
    for (const value of [{protocol:2,port:4455,token:"a".repeat(64)},{protocol:1,port:65536,token:"a".repeat(64)},{protocol:1,port:4455,token:"bad"}]) {
        assert.throws(() => parseConfiguration(JSON.stringify(value)));
    }
    assert.throws(() => parseConfiguration(" ".repeat(4097)));
});
