// Paired stage comparison. All sources/artifacts are frozen before any samples.
import assert from "node:assert/strict";
import * as arquero from "arquero";
import { readFile, writeFile, mkdir, cp, copyFile, readdir } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { cpus } from "node:os";
import { hash, verifySnapshot } from "./table-profile-summary.mjs";
import { collect } from "./testing/memory.mjs";

assert.equal(typeof global.gc, "function", "use node --expose-gc");
const [beforeArg, stagesArg, outputArg] = process.argv.slice(2);
assert.ok(beforeArg && stagesArg && outputArg, "usage: table-tuning.mjs baseline-capture stages-directory new-output");
const before = resolve(beforeArg), stages = resolve(stagesArg), output = resolve(outputArg);
const original = await verifySnapshot(before);
await mkdir(dirname(output), { recursive: true }); await mkdir(output);
const report = { schema: "walz-table-tuning-v2", created: new Date().toISOString(), node: process.version, v8: process.versions.v8,
  cpu: cpus()[0]?.model, platform: process.platform, arch: process.arch, baseline: before,
  baselineSha256: hash(await readFile(join(before, "results.json"))), inputs: {}, stages: {}, cases: [],
  method: { samples: 9, warmupMs: 500, minimumWarmupCalls: 32, targetBatchMs: 20, order: "rotate variants every round",
    stages: "original; bulk numeric I/O; one aligned scan; block short-circuit; same-column vector fusion; exact row-ID allocation via masks",
    controls: "official Arquero 8.0.3 and final packed-scalar comparisons; default I/O per captured facade",
    verification: "full rows and scalar counts checked outside clocks; every iteration consumes its result; explicit GC outside clocks",
    caveat: "Different suite from historical v1: additional stages and 500ms/32-call warmup. Do not mix samples. No concurrent builds/tests/profiles." } };
const save = () => writeFile(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
async function inventory(directory, prefix = "") {
  const out = {};
  for (const entry of await readdir(join(directory, prefix), { withFileTypes: true })) {
    const name = join(prefix, entry.name);
    if (entry.isDirectory()) Object.assign(out, await inventory(directory, name));
    else out[name] = hash(await readFile(join(directory, name)));
  }
  return out;
}
await copyFile(fileURLToPath(import.meta.url), join(output, "tuning.mjs"));
const variants = [];
async function add(name, directory, options = {}) {
  const { instantiate } = await import(pathToFileURL(join(directory, "source/runtime/table/index.mjs")));
  const base = pathToFileURL(join(directory, name === "original" ? "kernel/" : "./"));
  variants.push({ name, api: await instantiate(base, options) });
}
await add("original", before);
for (let i = 1; i <= 5; i++) {
  const name = `stage${i}`, target = join(output, name);
  await cp(join(stages, name), target, { recursive: true }); report.stages[name] = await inventory(target);
  await add(name, target);
}
variants.push({ name: "arquero", api: arquero });
await add("stage5.packed-scalar", join(output, "stage5"), { filter: "packed-scalar" });
// An artifact-only capture for replaying the unchanged profiling workloads.
// This is explicitly not a suite-1 benchmark result.
const final = join(output, "final"); await mkdir(final); await mkdir(join(final, "kernel"));
await cp(join(output, "stage5/source"), join(final, "source"), { recursive: true });
await copyFile(join(output, "stage5/core.wasm"), join(final, "kernel/core.wasm"));
await cp(join(before, "inputs"), join(final, "inputs"), { recursive: true });
const finalSources = await inventory(join(final, "source"));
await writeFile(join(final, "results.json"), JSON.stringify({ schema: "walz-table-artifact-v2", version: 2,
  note: "Artifact-only capture for profiling; paired timings are in the parent tuning-v2 results.",
  artifacts: { core: hash(await readFile(join(final, "kernel/core.wasm"))) }, inputs: original.inputs, sources: finalSources }, null, 2) + "\n");
let sink;
function batch(v, n) {
  let value, checksum = 0; const start = performance.now();
  for (let i = 0; i < n; i++) { value = v.run(); checksum = (checksum + v.consume(value)) | 0; }
  const ms = performance.now() - start; sink = value;
  assert.equal(checksum, Math.imul(v.expected, n)); return ms;
}
const median = a => [...a].sort((a, b) => a - b)[a.length >>> 1];
for (const id of ["4096-numeric", "262144-numeric", "262144-nullable"]) {
  const bytes = await readFile(join(before, "inputs", id + ".json")); assert.equal(hash(bytes), original.inputs[id]);
  await mkdir(join(output, "inputs"), { recursive: true }); await writeFile(join(output, "inputs", id + ".json"), bytes);
  report.inputs[id] = hash(bytes);
  const data = JSON.parse(bytes); data.x = data.x.map(v => v?.tag === "undefined" ? undefined : v?.tag === "NaN" ? NaN : v);
  const rows = data.x.map((x, i) => ({ x, y: data.y[i], id: data.id[i] }));
  const simple = d => d.x < 20, compound = d => d.x >= -100 && d.x < 100 && (d.y < 32 || d.y >= 96);
  for (const v of variants) v.table = v.api.table(data);
  for (const [operation, predicate] of [["resident-simple", simple], ["resident-compound", compound],
    ["ingest-only", null], ["cold-ingest-filter", simple], ["filter-materialize", simple]]) {
    const expected = predicate ? rows.filter(predicate) : rows, samples = {}, iterations = {}, warmups = {};
    for (const v of variants) {
      v.expected = expected.length;
      v.run = operation === "ingest-only" ? () => v.api.table(data)
        : operation === "cold-ingest-filter" ? () => v.api.table(data).filter(predicate)
        : operation === "filter-materialize" ? () => v.table.filter(predicate).objects() : () => v.table.filter(predicate);
      v.consume = operation === "filter-materialize" ? a => a.length : t => t.numRows();
      v.verify = operation === "filter-materialize" ? a => assert.deepEqual(a, expected) : t => assert.deepEqual(t.objects(), expected);
      v.verify(v.run()); const end = performance.now() + 500; let calls = 0;
      do { batch(v, 1); calls++; } while (calls < 32 || performance.now() < end);
      warmups[v.name] = calls;
      let n = 1, ms;
      do { ms = batch(v, n); if (ms >= 10 || n >= 65536) break; n *= 2; } while (true);
      iterations[v.name] = Math.max(1, Math.min(65536, Math.floor(n * 20 / ms))); samples[v.name] = [];
    }
    for (let round = 0; round < 9; round++) for (let offset = 0; offset < variants.length; offset++) {
      const v = variants[(round + offset) % variants.length]; await collect();
      samples[v.name].push(batch(v, iterations[v.name]) * 1000 / iterations[v.name]); v.verify(sink);
    }
    const mediansUs = Object.fromEntries(Object.entries(samples).map(([name, values]) => [name, median(values)]));
    report.cases.push({ id, operation, mediansUs, samplesUs: samples, iterations, warmupCalls: warmups });
    await save(); console.log(id, operation, JSON.stringify(mediansUs));
  }
}
await verifySnapshot(before); await save();
