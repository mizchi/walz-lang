// Profiles archived executable artifacts. Never build current sources here.
import assert from "node:assert/strict";
import * as reference from "arquero";
import { Session } from "node:inspector";
import { promisify } from "node:util";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { join, resolve, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { cpus } from "node:os";
import { collect } from "./testing/memory.mjs";
import { hash, summarizeCpu, summarizeHeap, verifySnapshot } from "./table-profile-summary.mjs";

const simple = d => d.x < 20;
const compound = d => d.x >= -100 && d.x < 100 && (d.y < 32 || d.y >= 96);
const median = values => [...values].sort((a, b) => a - b)[values.length >>> 1];
const save = (path, value) => writeFile(path, JSON.stringify(value, null, 2) + "\n");
let sink;

async function namespace(instantiate, base, mode, counted) {
  if (!counted) return { api: await instantiate(base, { filter: mode }) };
  // Diagnostic pass only. The real functions and Wasm bytes are unchanged.
  // Restore the global immediately; only this facade instance keeps wrappers.
  const original = WebAssembly.instantiate, calls = new Map();
  try {
    WebAssembly.instantiate = async (...args) => {
      const value = await original(...args);
      assert.ok(value.instance, "the captured facade must instantiate Wasm bytes");
      const exports = Object.fromEntries(Object.entries(value.instance.exports).map(([name, fn]) => [name, typeof fn !== "function" ? fn : (...values) => {
        calls.set(name, (calls.get(name) ?? 0) + 1); return fn(...values);
      }]));
      return { module: value.module, instance: { exports } };
    };
    return { api: await instantiate(base, { filter: mode }), calls };
  } finally { WebAssembly.instantiate = original; }
}

async function jobs(snapshot, counted = false) {
  const { instantiate } = await import(pathToFileURL(join(snapshot, "source/runtime/table/index.mjs")));
  const base = pathToFileURL(join(snapshot, "kernel/")), out = [];
  async function dataset(id) {
    const input = JSON.parse(await readFile(join(snapshot, "inputs", id + ".json")));
    input.x = input.x.map(value => value?.tag === "undefined" ? undefined : value?.tag === "NaN" ? NaN : value);
    const rows = input.id.map((id, i) => ({ x: input.x[i], y: input.y[i], id }));
    return { input, rows };
  }
  function add(name, fn, expected, consume, decode, calls) {
    out.push({ name, fn, consume, calls, verify: value => assert.deepEqual(decode(value), expected), expectedCount: expected.length });
  }
  const large = await dataset("262144-numeric");
  for (const mode of ["scalar", "packed-scalar", "simd"]) {
    const { api, calls } = await namespace(instantiate, base, mode, counted), t = api.table(large.input);
    add(`${mode}.compound`, () => t.filter(compound), large.rows.filter(compound), t => t.numRows(), t => t.objects(), calls);
    if (mode !== "simd") continue;
    const expected = large.rows.filter(simple), filtered = t.filter(simple);
    add("simd.simple", () => t.filter(simple), expected, t => t.numRows(), t => t.objects(), calls);
    add("simd.ingest-only", () => api.table(large.input), large.rows, t => t.numRows(), t => t.objects(), calls);
    add("simd.cold-ingest-filter", () => api.table(large.input).filter(simple), expected, t => t.numRows(), t => t.objects(), calls);
    add("simd.materialize-only", () => filtered.objects(), expected, a => a.length, a => a, calls);
    add("simd.filter-materialize", () => t.filter(simple).objects(), expected, a => a.length, a => a, calls);
  }
  const official = reference.table(large.input);
  add("arquero.compound", () => official.filter(compound), large.rows.filter(compound), t => t.numRows(), t => t.objects());
  add("arquero.filter-materialize", () => official.filter(simple).objects(), large.rows.filter(simple), a => a.length, a => a);
  for (const [id, label] of [["4096-numeric", "small"], ["262144-nullable", "nullable"]]) {
    const data = await dataset(id), { api, calls } = await namespace(instantiate, base, "simd", counted), t = api.table(data.input);
    add(`simd.${label}-compound`, () => t.filter(compound), data.rows.filter(compound), t => t.numRows(), t => t.objects(), calls);
  }
  return out;
}

function batch(job, iterations) {
  let value, checksum = 0;
  const start = performance.now();
  for (let i = 0; i < iterations; i++) { value = job.fn(); checksum = (checksum + job.consume(value)) | 0; }
  sink = value;
  assert.equal(checksum, Math.imul(job.expectedCount, iterations));
  return { ms: performance.now() - start, value };
}

async function warm(job) {
  const start = performance.now(); let calls = 0;
  do { batch(job, 1); calls++; } while (calls < 32 || performance.now() - start < 500);
  return calls;
}

export async function profile(snapshot, output, { durationMs = 1500, filter = "" } = {}) {
  assert.equal(typeof global.gc, "function", "use node --expose-gc");
  assert.ok(Number.isInteger(durationMs) && durationMs >= 500 && durationMs <= 10000);
  snapshot = resolve(snapshot); output = resolve(output);
  const captured = await verifySnapshot(snapshot);
  const versions = {};
  for (const name of ["arquero", "acorn"])
    versions[name] = JSON.parse(await readFile(new URL(`../node_modules/${name}/package.json`, import.meta.url))).version;
  assert.equal(versions.arquero, "8.0.3"); assert.equal(versions.acorn, "8.15.0");
  const selected = (await jobs(snapshot)).filter(job => job.name.includes(filter));
  assert.ok(selected.length, "no profile cases matched");
  await mkdir(dirname(output), { recursive: true }); await mkdir(output);
  for (const name of ["table-profile.mjs", "table-profile-summary.mjs", "table-profile.test.mjs"])
    await copyFile(new URL(name, import.meta.url), join(output, name));
  const report = { schema: "walz-table-profile-v1", created: new Date().toISOString(), snapshot,
    captureSha256: hash(await readFile(join(snapshot, "results.json"))), wasmSha256: captured.artifacts.core,
    node: process.version, v8: process.versions.v8, cpu: cpus()[0]?.model, platform: process.platform, arch: process.arch,
    versions, durationMs, predicates: { simple: String(simple), compound: String(compound) },
    method: { timing: "Separate unprofiled pass: at least 500ms/32 calls warmup, 9 samples, 20ms target batches; full rows verified outside clocks.",
      cpu: "V8 inspector, 500us interval; setup/warmup/validation excluded. Recursive inclusive time is counted once per function per stack.",
      heap: "Separate sampled JS heap pass, 32768-byte sampling, including objects collected by minor/major GC. Wasm GC allocations may be absent; not a total-allocation or leak measurement.",
      calls: "Separate untimed facade instance wraps native exports to count JS-to-Wasm calls. No wrappers in timing/CPU/heap passes.",
      caveat: "Profiled throughput is not speed evidence. Warmup is longer than benchmark suite v1 and may change JIT tiering. No concurrent builds/tests/profilers." }, cases: [] };
  // Complete all unprofiled measurements before enabling either profiler.
  for (const job of selected) {
    job.verify(job.fn()); const warmupCalls = await warm(job);
    let iterations = 1, trial;
    do { trial = batch(job, iterations); if (trial.ms >= 10 || iterations >= 65536) break; iterations *= 2; } while (true);
    iterations = Math.max(1, Math.floor(iterations * 20 / trial.ms));
    const samplesUs = [];
    for (let i = 0; i < 9; i++) {
      await collect(); const value = batch(job, iterations); samplesUs.push(value.ms * 1000 / iterations); job.verify(value.value);
    }
    report.cases.push({ name: job.name, warmupCalls, timing: { iterations, samplesUs, medianUs: median(samplesUs) } });
    console.log(`${job.name}: unprofiled ${median(samplesUs).toFixed(1)} us`);
  }
  await save(join(output, "profile.json"), report);
  for (const job of selected) {
    await warm(job); await collect();
    const session = new Session(); session.connect(); const post = promisify(session.post.bind(session));
    let cpu, heap, cpuCalls = 0;
    const heapCalls = /ingest|materialize/.test(job.name) ? 8 : 64;
    try {
      await post("Profiler.enable"); await post("Profiler.setSamplingInterval", { interval: 500 }); await post("Profiler.start");
      const end = performance.now() + durationMs;
      do { batch(job, 1); cpuCalls++; } while (performance.now() < end);
      cpu = (await post("Profiler.stop")).profile; job.verify(sink); await collect();
      await post("HeapProfiler.startSampling", { samplingInterval: 32768,
        includeObjectsCollectedByMajorGC: true, includeObjectsCollectedByMinorGC: true });
      batch(job, heapCalls); heap = (await post("HeapProfiler.stopSampling")).profile; job.verify(sink);
    } finally { session.disconnect(); }
    await save(join(output, `${job.name}.cpuprofile`), cpu); await save(join(output, `${job.name}.heapprofile`), heap);
    const entry = report.cases.find(c => c.name === job.name);
    Object.assign(entry, { cpuCalls, cpu: summarizeCpu(cpu), heap: summarizeHeap(heap.head, heapCalls) });
    await save(join(output, "profile.json"), report);
    console.log(`${job.name}: ${entry.cpu.functions.slice(0, 4).map(f => `${f.name} ${f.selfPercent.toFixed(1)}%`).join(", ")}`);
  }
  for (const job of (await jobs(snapshot, true)).filter(job => job.name.includes(filter))) {
    if (!job.calls) continue;
    job.calls.clear(); const value = job.fn(); job.consume(value);
    const exportedCalls = Object.fromEntries([...job.calls].sort(([a], [b]) => a.localeCompare(b)));
    job.verify(value);
    const entry = report.cases.find(c => c.name === job.name);
    entry.exportedCalls = exportedCalls; entry.totalExportedCalls = Object.values(exportedCalls).reduce((a, b) => a + b, 0);
  }
  await verifySnapshot(snapshot); await save(join(output, "profile.json"), report);
  return report;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [snapshot, output, filter = ""] = process.argv.slice(2);
  assert.ok(snapshot && output, "usage: table-profile.mjs snapshot-directory new-output-directory [filter]");
  await profile(snapshot, output, { filter });
}
