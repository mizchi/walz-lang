import { test } from "node:test";
import assert from "node:assert/strict";
import { summarizeCpu, wasmSections } from "./flatbuffers-inspect.mjs";
import { assertComparable, measureBatch } from "./flatbuffers-compare.mjs";

test("benchmark batches drain queued Task-like promise jobs before cleanup and the next batch", async () => {
  let pending = 0;
  const job = { fn() { pending++; Promise.resolve().then(() => pending--); return 42; },
    check(value) { assert.equal(value, 42); }, clean() { assert.equal(pending, 0); } };
  await measureBatch(job, 1000);
  assert.equal(pending, 0);
  await measureBatch(job, 1000);
});

test("paired comparisons reject different inputs or reference programs", () => {
  const before = { reference: { flatbuffers: "25.9.23" }, hashes: { "corpus.json": "same", "inputs/small.bin": "same", "reference.mjs": "same" } };
  assertComparable(before, structuredClone(before));
  for (const name of Object.keys(before.hashes)) {
    const after = structuredClone(before); after.hashes[name] = "changed";
    assert.throws(() => assertComparable(before, after), /comparison input changed/);
  }
});

test("FlatBuffers CPU summaries use sample durations and include callees separately", () => {
  const profile = { nodes: [
    { id: 1, callFrame: { functionName: "root", url: "" }, children: [2] },
    { id: 2, callFrame: { functionName: "encode", url: "wasm://guest" }, children: [3] },
    { id: 3, callFrame: { functionName: "copy", url: "wasm://guest" } },
  ], samples: [2, 3, 3], timeDeltas: [1000, 2000, 3000] };
  const result = summarizeCpu(profile);
  assert.equal(result.totalUs, 6000);
  assert.equal(result.functions.find(f => f.name === "encode").selfUs, 1000);
  assert.equal(result.functions.find(f => f.name === "encode").inclusiveUs, 6000);
  assert.equal(result.functions.find(f => f.name === "copy").selfUs, 5000);
});

test("Wasm section sizes include section overhead and reject truncated inputs", () => {
  const bytes = Uint8Array.of(0,97,115,109,1,0,0,0, 0,3,1,120,42);
  const sections = wasmSections(bytes);
  assert.deepEqual(sections, [{ id: 0, name: "x", bytes: 5, payloadBytes: 3 }]);
  assert.throws(() => wasmSections(bytes.slice(0,-1)), /truncated/);
});
