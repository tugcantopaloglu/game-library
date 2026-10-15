import { test } from "node:test";
import assert from "node:assert/strict";
import {generateKeyPairSync,verify,createHash} from "node:crypto";
import {signedManifest} from "./sign-update.mjs";
const {privateKey,publicKey}=generateKeyPairSync("ed25519");
const pem=privateKey.export({type:"pkcs8",format:"pem"});
const trusted=Buffer.from(publicKey.export({format:"jwk"}).x,"base64url").toString("base64");

test("release signature covers the exact Rust canonical payload",()=>{
    const bytes=Buffer.from("MZ fixture executable");
    const m=signedManifest(bytes,"0.2.0",pem,trusted);
    const payload=`${m.version}\n${m.url}\n${m.sha256}\n${m.size}\n${m.minimum_bridge_protocol}`;
    assert.equal(verify(null,Buffer.from(payload),publicKey,Buffer.from(m.signature,"base64")),true);
    assert.equal(m.sha256,createHash("sha256").update(bytes).digest("hex"));
    assert.equal(verify(null,Buffer.from(payload+"tamper"),publicKey,Buffer.from(m.signature,"base64")),false);
});
test("wrong signing keys, non-Windows files and unstable versions never publish",()=>{
    const bytes=Buffer.from("MZ fixture");
    assert.throws(()=>signedManifest(bytes,"0.2.0",pem,Buffer.alloc(32).toString("base64")),/does not match/);
    for(const version of ["v0.2.0","0.2.0-rc.1","0.2.0+foo","01.2.0","../../bad"]){assert.throws(()=>signedManifest(bytes,version,pem,trusted));}
    assert.throws(()=>signedManifest(Buffer.from("<!doctype html>"),"0.2.0",pem,trusted),/Invalid release executable/);
});
