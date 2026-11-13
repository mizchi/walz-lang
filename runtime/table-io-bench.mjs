// Compare captured artifacts through the complete column-input / row-output API.
// Build and capture separately; no compilation/profiling is allowed during timing.
import assert from "node:assert/strict";
import * as arquero from "arquero";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { cpus } from "node:os";
import { hash, verifySnapshot } from "./table-profile-summary.mjs";
import { collect } from "./testing/memory.mjs";

assert.equal(typeof global.gc, "function", "use node --expose-gc");
const referenceVersion = JSON.parse(await readFile(new URL("../node_modules/arquero/package.json", import.meta.url))).version;
assert.equal(referenceVersion, "8.0.3", "the recorded reference version must match the installed library");
const [baselineArg, candidateArg, outputArg] = process.argv.slice(2);
assert.ok(baselineArg && candidateArg && outputArg, "usage: table-io-bench.mjs baseline-capture candidate-capture new-output");
const baseline = resolve(baselineArg), candidate = resolve(candidateArg), output = resolve(outputArg);
const captures = await Promise.all([verifySnapshot(baseline), verifySnapshot(candidate)]);
await mkdir(dirname(output), { recursive: true }); await mkdir(output);
await copyFile(fileURLToPath(import.meta.url), join(output, "io-bench.mjs"));
const report = { schema: "walz-table-io-v1", created: new Date().toISOString(), node: process.version, v8: process.versions.v8,
  cpu: cpus()[0]?.model, platform: process.platform, arch: process.arch,
  baseline, candidate, captures: await Promise.all([baseline, candidate].map(async dir => hash(await readFile(join(dir, "results.json"))))),
  inputs: {}, cases: [], method: { samples: 9, warmupMs: 500, minimumWarmupCalls: 32, targetBatchMs: 20,
    order: "rotate variants every round; GC outside clocks; results consumed and every sampled result fully verified outside clocks",
    reference: "official Arquero 8.0.3", input: "existing JS column arrays (same object for every variant)", output: "ordinary JS row objects",
    endToEnd: "table(data).filter(predicate).objects(); includes copying, GC, query construction, scan and output; excludes loading/instantiation and JSON/network I/O",
    caveat: "New workload suite; all baseline and candidate timings are rerun together. No concurrent builds/tests/profiles." } };
const variants = [];
for (const [name, directory] of [["before", baseline], ["after", candidate]]) {
  const { instantiate } = await import(pathToFileURL(join(directory, "source/runtime/table/index.mjs")));
  variants.push({ name, api: await instantiate(pathToFileURL(join(directory, "kernel/"))) });
}
variants.push({ name: "arquero", api: arquero });
const simple = d => d.x < 20, compound = d => d.x >= -100 && d.x < 100 && (d.y < 32 || d.y >= 96);
const median = a => [...a].sort((a, b) => a - b)[a.length >>> 1];
let sink;
function batch(v, n) {
  let value, checksum = 0; const start = performance.now();
  for (let i = 0; i < n; i++) { value = v.run(); checksum = (checksum + v.consume(value)) | 0; }
  const ms = performance.now() - start; sink = value;
  assert.equal(checksum, Math.imul(v.expected, n)); return ms;
}
for (const size of [4096, 65536, 262144]) for (const type of ["numeric", "nullable"]) {
  const id = `${size}-${type}`, bytes = await readFile(join(candidate, "inputs", `${id}.json`));
  assert.equal(hash(bytes), captures[1].inputs[id]);
  // The baseline may contain only the historical profiling subset. It still
  // receives these exact candidate-captured bytes, never regenerated inputs.
  if (captures[0].inputs[id]) assert.equal(hash(bytes), captures[0].inputs[id]);
  report.inputs[id] = hash(bytes);
  const data = JSON.parse(bytes); data.x = data.x.map(v => v?.tag === "undefined" ? undefined : v?.tag === "NaN" ? NaN : v);
  const rows = data.x.map((x, i) => ({ x, y: data.y[i], id: data.id[i] }));
  for (const v of variants) { v.table = v.api.table(data); v.filtered = v.table.filter(simple); }
  for (const [operation, predicate] of [["ingest-only", null], ["resident-compound", compound],
    ["materialize-only", simple], ["filter-materialize", simple], ["end-to-end-simple", simple], ["end-to-end-compound", compound]]) {
    const expected = predicate ? rows.filter(predicate) : rows, samples = {}, iterations = {}, warmups = {};
    const objects = operation.includes("materialize") || operation.startsWith("end-to-end");
    for (const v of variants) {
      v.expected = expected.length;
      v.run = operation === "ingest-only" ? () => v.api.table(data)
        : operation === "resident-compound" ? () => v.table.filter(predicate)
        : operation === "materialize-only" ? () => v.filtered.objects()
        : operation === "filter-materialize" ? () => v.table.filter(predicate).objects()
        : () => v.api.table(data).filter(predicate).objects();
      v.consume = objects ? a => a.length : t => t.numRows();
      v.verify = objects ? a => assert.deepEqual(a, expected) : t => assert.deepEqual(t.objects(), expected);
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
    await writeFile(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n"); console.log(id, operation, JSON.stringify(mediansUs));
  }
}
await Promise.all([verifySnapshot(baseline), verifySnapshot(candidate)]);
