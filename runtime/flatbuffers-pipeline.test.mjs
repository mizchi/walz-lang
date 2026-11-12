import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";
import * as flatbuffers from "flatbuffers";
import { createReference } from "./flatbuffers-bench/reference.mjs";
import { summarize, query, sumI32 } from "./flatbuffers-bench/pipeline.mjs";
const reference = createReference(flatbuffers);
test("pipeline reference sums retain exact i64 results across Number accumulation chunks", () => {
  const n=4194305, values=new Int32Array(n);values.fill(2147483647);
  assert.equal(sumI32(values),BigInt(n)*2147483647n);
  values.fill(-2147483648);assert.equal(sumI32(values),BigInt(n)*-2147483648n);
});
for (const backend of ["core", "component"]) test(`${backend}: FlatBuffers queries and batches avoid materialized ABI records`, { timeout: 120000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-fb-pipeline-"));
  try {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build("examples/flatbuffers/pipeline.walz", directory); await instrument(directory, backend);
    const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
    let arena;
    const { api, __memory } = await instantiate({ instantiateCore: async (module, imports) => {
      const instance = await WebAssembly.instantiate(module, imports);
      if (instance.exports.live) arena = instance.exports; return instance;
    } });
    const clean = () => { assert.equal(arena.live(), 0); assert.ok(Object.values(__memory()).every(n => n === 0)); };
    const messages = [0,1,17,16384].map(n => ({ id: -9007199254740993n, name: "日本語🦀",
      values: Int32Array.from({length:n}, (_,i) => i % 2 ? 2147483647 : -2147483648),
      payload: Uint8Array.from({length:n}, (_,i) => i), enabled: false, weight: -1.25,
      child: n ? {count:42,label:"valid🦀"} : undefined }));
    const inputs = messages.map(m => reference.encode(m));
    for (let i=0;i<inputs.length;i++) {
      assert.deepEqual(query(flatbuffers.ByteBuffer, inputs[i]), summarize(messages[i]));
      const unaligned=new Uint8Array(inputs[i].length+1);unaligned.set(inputs[i],1);
      assert.deepEqual(query(flatbuffers.ByteBuffer,unaligned.subarray(1)),summarize(messages[i]));
      assert.deepEqual(api.inspect(inputs[i]), {tag:"ok",val:summarize(messages[i])});
      const result = api.transform(inputs[i], 1); assert.equal(result.tag,"ok");
      const expected = {...messages[i], values: messages[i].values.map(v => (v + 1) | 0)};
      assert.deepEqual(reference.decode(result.val), expected); clean();
    }
    const bad = new Uint8Array();
    const mixed = [inputs[1],bad,inputs[0],inputs[3]];
    assert.deepEqual(api.inspectBatch(mixed), [1,-1,0,3].map(i => i < 0 ? {tag:"err",val:1} : {tag:"ok",val:summarize(messages[i])}));
    const transformed = api.transformBatch(mixed,-1);
    assert.equal(transformed[1].tag,"err"); assert.equal(transformed[1].val,1);
    for (const [at,i] of [[0,1],[2,0],[3,3]]) assert.deepEqual(reference.decode(transformed[at].val),
      {...messages[i],values:messages[i].values.map(v => (v - 1) | 0)});
    assert.deepEqual(api.inspectBatch([]),[]); assert.deepEqual(api.transformBatch([],1),[]);
    // Queries must validate every known string, even a child label not returned.
    const invalid = inputs[1].slice(), bb = new flatbuffers.ByteBuffer(invalid), root = bb.readUint32(0);
    const childSlot = root + bb.__offset(root,16), child = bb.__indirect(childSlot);
    const label = bb.__indirect(child + bb.__offset(child,6)); invalid[label + 4] = 255;
    assert.deepEqual(api.inspect(invalid),{tag:"err",val:4});
    assert.deepEqual(api.transform(invalid,1),{tag:"err",val:4});
    for (let i=0;i<100;i++) { api.inspectBatch([inputs[1],bad]); api.transformBatch([bad,inputs[1]],i); clean(); }
    assert.deepEqual(inputs.map(b => reference.decode(b)),messages,"inputs remain immutable"); clean();
  } finally { await rm(directory,{recursive:true,force:true}); }
});
