import test from "node:test";
import assert from "node:assert/strict";
import { summarizeCpu, summarizeHeap, verifySnapshot, modelCompoundBlocks } from "./table-profile-summary.mjs";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";

test("CPU summaries merge call sites without double-counting recursive inclusive time", () => {
  const node = (id, name, children = [], url = "wasm://kernel") => ({ id, callFrame: { functionName: name, url }, children });
  const summary = summarizeCpu({ nodes: [node(1, "root", [2, 4]), node(2, "mask", [3]), node(3, "mask"), node(4, "mask")],
    samples: [3, 2, 4, 1], timeDeltas: [10, 20, 30, 40] });
  assert.equal(summary.totalUs, 100);
  const mask = summary.functions.find(f => f.name === "mask");
  assert.equal(mask.selfUs, 60); assert.equal(mask.inclusiveUs, 60);
  assert.equal(mask.selfPercent, 60); assert.equal(mask.inclusivePercent, 60);
  assert.equal(summary.functions.find(f => f.name === "root").inclusiveUs, 100);
  assert.equal(summary.functions.filter(f => f.name === "mask").length, 1);
});

test("sampled allocation totals merge sites and normalize by actual calls", () => {
  const frame = { functionName: "objects", url: "file:///table.mjs" };
  const summary = summarizeHeap({ callFrame: frame, selfSize: 1024, children: [{ callFrame: frame, selfSize: 3072, children: [] }] }, 2);
  assert.equal(summary.sampledBytes, 4096); assert.equal(summary.sampledBytesPerCall, 2048);
  assert.equal(summary.functions.length, 1); assert.equal(summary.functions[0].sampledBytes, 4096);
  assert.throws(() => summarizeHeap({ callFrame: frame, children: [] }, 0));
});

test("the block model proves short-circuit masks and counts skipped comparisons", () => {
  const result = modelCompoundBlocks({ x: [-200, -199, 0, 1, 200, -20], y: [0, 1, 0, 1, 100, 100] });
  assert.equal(result.selected, 3); assert.equal(result.eagerLeafComparisons, 24);
  assert.equal(result.shortCircuitLeafComparisons, 16); assert.equal(result.zeroRangeBlocks, 1);
  for (const n of [0, 1, 2, 3, 5, 31, 32, 33]) {
    const x = Array.from({ length: n }, (_, i) => [null, undefined, NaN, -100, 100, -0, Infinity, -Infinity][i % 8]);
    const y = Array.from({ length: n }, (_, i) => i % 127);
    assert.equal(modelCompoundBlocks({ x, y }).selected, x.filter((v, i) => v >= -100 && v < 100 && (y[i] < 32 || y[i] >= 96)).length);
  }
  assert.throws(() => modelCompoundBlocks({ x: [0], y: [] }));
});

test("profiling rejects modified archived Wasm, inputs and facade sources", async () => {
  const directory = await mkdtemp(join(tmpdir(), "walz-table-profile-"));
  try {
    const hash = bytes => createHash("sha256").update(bytes).digest("hex");
    const entries = { "kernel/core.wasm": "wasm", "inputs/1-numeric.json": "input", "source/runtime/table/index.mjs": "facade" };
    for (const [file, value] of Object.entries(entries)) {
      await mkdir(join(directory, file, ".."), { recursive: true }); await writeFile(join(directory, file), value);
    }
    await writeFile(join(directory, "results.json"), JSON.stringify({ version: 1,
      artifacts: { core: hash("wasm") }, inputs: { "1-numeric": hash("input") }, sources: { "runtime/table/index.mjs": hash("facade") } }));
    await verifySnapshot(directory);
    for (const [file, value] of Object.entries(entries)) {
      await writeFile(join(directory, file), "changed"); await assert.rejects(verifySnapshot(directory), /snapshot changed/);
      await writeFile(join(directory, file), value);
    }
  } finally { await rm(directory, { recursive: true, force: true }); }
});
