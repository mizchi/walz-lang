import assert from "node:assert/strict";
import { Session } from "node:inspector";
import { promisify } from "node:util";
import { performance } from "node:perf_hooks";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { cpus } from "node:os";
import { verify } from "./flatbuffers-bench/runner.mjs";
import { hash, materialize } from "./flatbuffers-bench/workloads.mjs";
import { createReference } from "./flatbuffers-bench/reference.mjs";

export function summarizeCpu(profile) {
  const parents = new Map(), nodes = new Map(profile.nodes.map(n => [n.id, n]));
  for (const n of profile.nodes) for (const child of n.children ?? []) parents.set(child, n.id);
  const rows = new Map(profile.nodes.map(n => [n.id, { name: n.callFrame.functionName || "(anonymous)", url: n.callFrame.url, selfUs: 0, inclusiveUs: 0 }]));
  let totalUs = 0;
  for (const [i, id] of (profile.samples ?? []).entries()) {
    const us = profile.timeDeltas?.[i] ?? 1000;
    totalUs += us;
    if (!nodes.has(id)) continue;
    rows.get(id).selfUs += us;
    let current = id;
    while (current !== undefined) { rows.get(current).inclusiveUs += us; current = parents.get(current); }
  }
  return { totalUs, samples: profile.samples?.length ?? 0, functions: [...rows.values()].filter(r => r.inclusiveUs)
    .map(r => ({ ...r, selfPercent: r.selfUs / totalUs * 100, inclusivePercent: r.inclusiveUs / totalUs * 100 }))
    .sort((a, b) => b.selfUs - a.selfUs) };
}
export function wasmSections(bytes) {
  assert.ok(bytes.length >= 8 && bytes[0] === 0 && bytes[1] === 97 && bytes[2] === 115 && bytes[3] === 109, "invalid Wasm");
  let offset = 8;
  const uleb = () => {
    let n = 0, scale = 1;
    for (let i = 0; i < 5; i++) {
      assert.ok(offset < bytes.length, "truncated Wasm section");
      const b = bytes[offset++]; n += (b & 127) * scale;
      if (!(b & 128)) return n;
      scale *= 128;
    }
    throw new Error("invalid Wasm section length");
  };
  const out = [];
  while (offset < bytes.length) {
    const start = offset, id = bytes[offset++], size = uleb(), payload = offset, end = offset + size;
    assert.ok(end <= bytes.length, "truncated Wasm section");
    let name;
    if (id === 0) { const n = uleb(); assert.ok(offset + n <= end, "truncated custom section name"); name = new TextDecoder().decode(bytes.subarray(offset, offset + n)); }
    out.push({ id, ...(name === undefined ? {} : { name }), bytes: end - start, payloadBytes: end - payload });
    offset = end;
  }
  return out;
}
function allocationSummary(node, rows = []) {
  if (node.selfSize) rows.push({ name: node.callFrame.functionName || "(anonymous)", url: node.callFrame.url, sampledBytes: node.selfSize });
  for (const child of node.children ?? []) allocationSummary(child, rows);
  return rows.sort((a, b) => b.sampledBytes - a.sampledBytes);
}
const save = (path, value) => writeFile(path, JSON.stringify(value, null, 2) + "\n", { flag: "wx" });
let sink;
export async function createJobs(directory, cases = null) {
  const corpus = JSON.parse(await readFile(join(directory, "corpus.json"), "utf8"));
  const reference = createReference(await import(pathToFileURL(join(directory, "reference-package/js/flatbuffers.js"))));
  const { instance } = await WebAssembly.instantiate(await readFile(join(directory, "resident.wasm")));
  const raw = instance.exports, adapters = {};
  for (const backend of ["core", "component"]) {
    const { instantiate } = await import(pathToFileURL(join(directory, backend, "index.js")));
    let arena;
    const program = await instantiate({ instantiateCore: async (m, i) => {
      const instance = await WebAssembly.instantiate(m, i); if (instance.exports.live) arena = instance.exports; return instance;
    } });
    adapters[backend] = { api: program.api, clean() { assert.equal(arena.live(), 0); assert.ok(Object.values(program.__memory()).every(n => n === 0)); } };
  }
  const bytesOut = b => Uint8Array.from({ length: raw["byte-length"](b) }, (_, i) => raw["byte-get"](b, i));
  const out = [];
  for (const c of corpus.filter(c => !cases || cases.includes(c.name))) {
    const value = materialize(c.value), bytes = new Uint8Array(await readFile(join(directory, "inputs", c.name + ".bin")));
    const a = raw["input-new"](bytes.length);
    for (let i = 0; i < bytes.length; i++) raw["input-set"](a, i, bytes[i]);
    const input = raw["input-bytes"](a), record = raw.decode(input);
    const checkBytes = b => assert.deepEqual(reference.decode(b), value);
    out.push({ name: `${c.name}.encode.resident`, fn: () => raw.encode(record), check: b => checkBytes(bytesOut(b)) });
    out.push({ name: `${c.name}.decode.resident`, fn: () => raw.decode(input), check: m => checkBytes(bytesOut(raw.encode(m))) });
    out.push({ name: `${c.name}.encode.js`, fn: () => reference.encode(value), check: checkBytes });
    out.push({ name: `${c.name}.decode.js`, fn: () => reference.decode(bytes), check: m => assert.deepEqual(m, value) });
    for (const [backend, { api, clean }] of Object.entries(adapters)) {
      out.push({ name: `${c.name}.encode.${backend}`, fn: () => api.encode(value), check: checkBytes, clean });
      out.push({ name: `${c.name}.decode.${backend}`, fn: () => api.decode(bytes), check: m => assert.deepEqual(m, { tag: "ok", val: value }), clean });
    }
  }
  return out;
}
export async function profile(directory, output, { durationMs = 1000, filter = "" } = {}) {
  assert.equal(typeof global.gc, "function", "run with node --expose-gc");
  assert.ok(Number.isInteger(durationMs) && durationMs >= 100 && durationMs <= 10000);
  await verify(directory);
  const selected = (await createJobs(directory, ["small", "bytes-64k", "values-16k"])).filter(job => job.name.includes(filter));
  assert.ok(selected.length, "no profile cases matched");
  await mkdir(output, { recursive: false });
  await copyFile(fileURLToPath(import.meta.url), join(output, "tool.mjs"));
  const report = { schema: "walz-flatbuffers-profile-v1", created: new Date().toISOString(), node: process.version, v8: process.versions.v8,
    cpu: cpus()[0]?.model, captureHash: hash(await readFile(join(directory, "capture.json"))), durationMs,
    method: "CPU and sampled JS-heap allocation passes are separate; warmup/setup/validation excluded. Wasm GC allocations may not appear in heap sampling. Profiled throughput is not a speed benchmark.", cases: [] };
  for (const job of selected) {
    job.check(job.fn()); job.clean?.();
    let end = performance.now() + 150;
    while (performance.now() < end) for (let i = 0; i < 64; i++) sink = job.fn();
    global.gc();
    const session = new Session(); session.connect();
    const post = promisify(session.post.bind(session));
    let cpu, heap;
    try {
      await post("Profiler.enable"); await post("Profiler.setSamplingInterval", { interval: 1000 });
      await post("Profiler.start");
      end = performance.now() + durationMs;
      while (performance.now() < end) for (let i = 0; i < 64; i++) sink = job.fn();
      cpu = (await post("Profiler.stop")).profile;
      job.check(sink); job.clean?.(); global.gc();
      await post("HeapProfiler.startSampling", { samplingInterval: 32768, includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true });
      for (let i = 0; i < (job.name.startsWith("small.") ? 8192 : 256); i++) sink = job.fn();
      heap = (await post("HeapProfiler.stopSampling")).profile;
      job.check(sink); job.clean?.();
    } finally { session.disconnect(); }
    await save(join(output, job.name + ".cpuprofile"), cpu);
    await save(join(output, job.name + ".heapprofile"), heap);
    const summary = { name: job.name, cpu: summarizeCpu(cpu), allocations: allocationSummary(heap.head) };
    report.cases.push(summary);
    console.log(job.name + ": " + summary.cpu.functions.slice(0, 4).map(f => `${f.name} ${f.selfPercent.toFixed(1)}%`).join(", "));
  }
  await save(join(output, "profile.json"), report);
  return report;
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [mode, snapshot, output, filter] = process.argv.slice(2);
  if (mode !== "profile" || !snapshot || !output) throw new Error("usage: flatbuffers-inspect.mjs profile capture-directory output-directory [filter]");
  await profile(resolve(snapshot), resolve(output), { filter });
}
