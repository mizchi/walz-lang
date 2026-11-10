import {test} from "node:test";
import assert from "node:assert/strict";
import {mkdtemp,writeFile,rm} from "node:fs/promises";
import {tmpdir} from "node:os";
import {join} from "node:path";
import {corpus,hash,checksum} from "./json-bench/workloads.mjs";
import {verify,measure,equivalentOutput} from "./json-bench/runner.mjs";
import {checkCompatibility} from "./json-bench/compare.mjs";

test("JSON benchmark freezes diverse inputs in the shared value domain",()=>{
  assert.deepEqual(corpus(),corpus());
  assert.ok(corpus().every(c=>c.bytes<=65536 && c.bytes===Buffer.byteLength(c.source)));
  assert.ok(corpus().some(c=>c.bytes>48000));
  for(const c of corpus()) {
    assert.equal(c.inputHash,hash(c.source));
    assert.deepEqual(JSON.parse(c.source),JSON.parse(c.normalized));
    assert.equal(c.normalizedHash,checksum(c.normalized));
  }
});
test("JSON benchmark refuses changed artifacts and rejects incorrect runners",async()=>{
  const directory=await mkdtemp(join(tmpdir(),"walz-json-bench-"));
  try {
    await writeFile(join(directory,"input.json"),"null");
    await writeFile(join(directory,"capture.json"),JSON.stringify({hashes:{"input.json":hash("null")}}));
    await verify(directory);
    await writeFile(join(directory,"input.json"),"true");
    await assert.rejects(verify(directory),/snapshot changed/);
  }finally{await rm(directory,{recursive:true,force:true});}
  await assert.rejects(measure({broken:{fn:()=>41,check:value=>assert.equal(value,42)}},{rounds:5,targetMs:1}),/41/);
});

test("JSON benchmark accepts equivalent escape spellings and rejects different values",()=>{
  assert.equal(equivalentOutput({tag:"ok",val:'"\\u000a"'},'"\\n"'),'"\\u000a"');
  assert.throws(()=>equivalentOutput({tag:"ok",val:'{"n":2}'},'{"n":1}'),/value differs/);
  assert.throws(()=>equivalentOutput({tag:"err",val:0},'null'));
});

test("JSON comparison requires identical inputs and benchmark programs",()=>{
  const manifest={reference:{node:process.version,v8:process.versions.v8},hashes:{
    "corpus.json":"inputs", "source/examples/json-parser/resident.walz":"resident",
    "source/examples/json-parser/benchmark.walz":"api",
  }};
  checkCompatibility(manifest,structuredClone(manifest));
  for(const key of Object.keys(manifest.hashes)) {
    const changed=structuredClone(manifest);changed.hashes[key]="different";
    assert.throws(()=>checkCompatibility(manifest,changed),/comparison mismatch/);
  }
  const changed=structuredClone(manifest);changed.reference.v8="other";
  assert.throws(()=>checkCompatibility(manifest,changed),/comparison mismatch/);
  assert.throws(()=>checkCompatibility({hashes:{},reference:manifest.reference},manifest),/missing/);
});
