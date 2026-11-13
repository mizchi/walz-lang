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
  assert.equal(report.version, 1, "unsupported table benchmark suite");
  const files = { "kernel/core.wasm": report.artifacts.core };
  for (const [name, digest] of Object.entries(report.sources)) files[`source/${name}`] = digest;
  for (const [name, digest] of Object.entries(report.inputs)) files[`inputs/${name}.json`] = digest;
  for (const [file, digest] of Object.entries(files))
    assert.equal(hash(await readFile(join(directory, file))), digest, `snapshot changed: ${file}`);
  return report;
}
