import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { instrument } from "./testing/memory.mjs";
for (const backend of ["core", "component"]) test(`${backend}: packed GC copies preserve snapshots, order, truncation, overlap and bounds`, { timeout: 60000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-packed-"));
  try {
    const { build } = await import(backend === "core" ? "./build-core.mjs" : "./build.mjs");
    await build("tests/component-fixtures/packed-arrays.walz", directory); await instrument(directory, backend);
    const { instantiate } = await import(pathToFileURL(join(directory, "index.js")));
    for (const [v, signed, unsigned] of [[255,-1,255],[256,0,0],[-129,127,127]]) {
      const { api } = await instantiate(); assert.equal(api.signed8(v), signed); assert.equal(api.unsigned8(v), unsigned);
    }
    const { api, __memory } = await instantiate();
    assert.equal(api.signed16(65535), -1); assert.equal(api.signed16(65536), 0);
    for (const value of [0,128,255]) {
      assert.deepEqual(api.wideRange(value),Uint8Array.of(value));
      assert.deepEqual(api.numericRange(value),Uint8Array.of(value));
    }
    assert.deepEqual(api.wideRange(65536),Uint8Array.of(0),"i16 stores truncate before conversion");
    assert.deepEqual(api.overlap(), Uint8Array.of(0,0,1,2,3,4)); assert.equal(api.order(), 12345);
    assert.deepEqual(api.copy(2,1,3), Uint8Array.of(9,9,2,3,4,9,9,9));
    for (const n of [0,1,15,16,17,65537]) {
      const bytes = Uint8Array.from({ length: n }, (_, i) => (i * 31 + 7) & 255);
      for (const start of [0,n >> 1,n]) assert.deepEqual(api.snapshot(bytes,start,n-start), bytes.slice(start));
    }
    assert.ok(Object.values(__memory()).every(n => n === 0));
    for (const name of ["wideRange","numericRange"]) for (const value of [-1,256]) {
      const {api}=await instantiate();assert.throws(()=>api[name](value),WebAssembly.RuntimeError);
    }
    for (const args of [[-1,0,1],[8,0,1],[0,4,1],[0,0,-1]]) {
      const { api } = await instantiate(); assert.throws(() => api.copy(...args), WebAssembly.RuntimeError);
    }
    for (const [start,n] of [[-1,1],[1,-1],[3,0],[2,1]]) {
      const { api } = await instantiate(); assert.throws(() => api.snapshot(Uint8Array.of(1,2),start,n), WebAssembly.RuntimeError);
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});
