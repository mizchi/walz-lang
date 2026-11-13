import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";

export const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const frameKey = f => JSON.stringify([f.url, f.functionName, f.lineNumber ?? -1, f.columnNumber ?? -1]);
const frameRow = f => ({ name: f.functionName || "(anonymous)", url: f.url,
  lineNumber: f.lineNumber ?? -1, columnNumber: f.columnNumber ?? -1 });

export function summarizeCpu(profile) {
  const nodes = new Map(profile.nodes.map(n => [n.id, n])), parents = new Map(), functions = new Map();
  for (const node of profile.nodes) {
    for (const child of node.children ?? []) parents.set(child, node.id);
    const key = frameKey(node.callFrame);
    if (!functions.has(key)) functions.set(key, { ...frameRow(node.callFrame), selfUs: 0, inclusiveUs: 0 });
  }
  let totalUs = 0;
  for (const [i, id] of (profile.samples ?? []).entries()) {
    const weight = profile.timeDeltas?.[i] ?? 1000; totalUs += weight;
    if (!nodes.has(id)) continue;
    functions.get(frameKey(nodes.get(id).callFrame)).selfUs += weight;
    const seen = new Set();
    for (let at = id; at !== undefined; at = parents.get(at)) {
      const key = frameKey(nodes.get(at).callFrame);
      if (!seen.has(key)) functions.get(key).inclusiveUs += weight;
      seen.add(key);
    }
  }
  return { totalUs, samples: profile.samples?.length ?? 0, functions: [...functions.values()]
    .filter(f => f.inclusiveUs).map(f => ({ ...f, selfPercent: totalUs ? 100 * f.selfUs / totalUs : 0,
      inclusivePercent: totalUs ? 100 * f.inclusiveUs / totalUs : 0 })).sort((a, b) => b.selfUs - a.selfUs) };
}

export function summarizeHeap(head, calls) {
  assert.ok(Number.isInteger(calls) && calls > 0);
  const functions = new Map(), pending = [head]; let sampledBytes = 0;
  while (pending.length) {
    const node = pending.pop(); pending.push(...node.children ?? []);
    if (!node.selfSize) continue;
    const key = frameKey(node.callFrame);
    if (!functions.has(key)) functions.set(key, { ...frameRow(node.callFrame), sampledBytes: 0 });
    functions.get(key).sampledBytes += node.selfSize; sampledBytes += node.selfSize;
  }
  return { calls, sampledBytes, sampledBytesPerCall: sampledBytes / calls,
    functions: [...functions.values()].sort((a, b) => b.sampledBytes - a.sampledBytes) };
}

export async function verifySnapshot(directory) {
  const report = JSON.parse(await readFile(join(directory, "results.json")));
  assert.ok(report.version === 1 || (report.version === 2 && report.schema === "walz-table-artifact-v2"), "unsupported table capture");
  const files = { "kernel/core.wasm": report.artifacts.core };
  for (const [name, digest] of Object.entries(report.sources)) files[`source/${name}`] = digest;
  for (const [name, digest] of Object.entries(report.inputs)) files[`inputs/${name}.json`] = digest;
  for (const [file, digest] of Object.entries(files))
    assert.equal(hash(await readFile(join(directory, file))), digest, `snapshot changed: ${file}`);
  return report;
}

// Source-level execution model, not a timing estimate. The existing numeric
// plan has already proved every leaf total, so pair-mask short-circuiting is safe.
export function modelCompoundBlocks({ x, y }) {
  assert.equal(x.length, y.length);
  let blocks = 0, eager = 0, short = 0, zeroRange = 0, zeroLower = 0, fullLeftOrReached = 0, selected = 0;
  for (let i = 0; i < x.length; i += 2) {
    let lower = 0, upper = 0, left = 0, right = 0;
    for (let lane = 0; lane < 2 && i + lane < x.length; lane++) {
      const bit = 1 << lane;
      if (x[i + lane] >= -100) lower |= bit;
      if (x[i + lane] < 100) upper |= bit;
      if (y[i + lane] < 32) left |= bit;
      if (y[i + lane] >= 96) right |= bit;
    }
    const range = lower & upper, eagerMask = range & (left | right);
    let shortMask = 0; blocks++; eager += 4; short++;
    if (lower === 0) zeroLower++; else short++;
    if (range === 0) zeroRange++;
    else {
      short++;
      if (left === 3) { fullLeftOrReached++; shortMask = range; }
      else { short++; shortMask = range & (left | right); }
    }
    assert.equal(shortMask, eagerMask);
    selected += (eagerMask & 1) + ((eagerMask >>> 1) & 1);
  }
  return { physicalRows: x.length, blocks, passes: 2, selected,
    eagerLeafComparisons: eager * 2, shortCircuitLeafComparisons: short * 2,
    leafReduction: eager ? 1 - short / eager : 0, zeroRangeBlocks: zeroRange, zeroLowerBlocks: zeroLower,
    fullLeftOrBlocksReached: fullLeftOrReached, zeroRangePercent: blocks ? 100 * zeroRange / blocks : 0,
    caveat: "Counterfactual model of the original unfused two-scan plan on captured input, not executed-call counts or a timing estimate for the selected artifact." };
}
