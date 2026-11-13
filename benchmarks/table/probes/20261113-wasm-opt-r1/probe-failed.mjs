// A postprocessing experiment; original captures and default builds are untouched.
import assert from "node:assert/strict";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { cpus } from "node:os";
import { collect } from "./testing/memory.mjs";
import { hash, verifySnapshot } from "./table-profile-summary.mjs";

assert.equal(typeof global.gc, "function", "use node --expose-gc");
const [snapshotArg, outputArg, optimizerArg] = process.argv.slice(2);
assert.ok(snapshotArg && outputArg && optimizerArg, "usage: table-opt-probe.mjs snapshot new-output wasm-opt");
const snapshot = resolve(snapshotArg), output = resolve(outputArg), optimizer = resolve(optimizerArg);
const captured = await verifySnapshot(snapshot);
await mkdir(dirname(output), { recursive: true }); await mkdir(output); await mkdir(join(output, "optimized"));
const command = [join(snapshot, "kernel/core.wasm"), "--enable-gc", "--enable-reference-types", "--enable-exception-handling",
  "--enable-bulk-memory", "--enable-mutable-globals", "--enable-multivalue", "--enable-multimemory", "--enable-simd",
  "-O3", "-g", "-o", join(output, "optimized/core.wasm")];
const version = (await promisify(execFile)(optimizer, ["--version"])).stdout.trim();
assert.match(version, /\b133\b/); await promisify(execFile)(optimizer, command);
const original = await readFile(join(snapshot, "kernel/core.wasm")), optimized = await readFile(join(output, "optimized/core.wasm"));
assert.deepEqual(WebAssembly.Module.exports(await WebAssembly.compile(optimized)), WebAssembly.Module.exports(await WebAssembly.compile(original)));
const { instantiate } = await import(pathToFileURL(join(snapshot, "source/runtime/table/index.mjs")));
const variants = [];
for (const filter of ["simd", "packed-scalar"])
  for (const [name, directory] of [["original", join(snapshot, "kernel/")], ["O3", join(output, "optimized/")]])
    variants.push({ name: `${filter}.${name}`, api: await instantiate(pathToFileURL(directory), { filter }) });
// Check semantics beyond the timed input before starting samples.
const predicates = [d => d.x < 0, d => d.x >= 0, d => d.x === 0, d => d.x !== 0, d => d.x == 0,
  d => d.x != 0, d => !(d.x < 0), d => d.x >= -100 && d.x < 100 && (d.y < 32 || d.y >= 96), d => false && d.text - 1];
for (const { api } of variants) {
  for (const n of [0, 1, 2, 3, 31, 32, 33, 129]) {
    const values = [null, undefined, NaN, -0, 0, -1, 1, Infinity, -Infinity, Number.MIN_VALUE];
    const data = { x: Array.from({ length: n }, (_, i) => values[i % values.length]), y: Array.from({ length: n }, (_, i) => i % 127), text: Array(n).fill("x") };
    const rows = data.x.map((x, i) => ({ x, y: data.y[i], text: "x" })), t = api.table(data);
    for (const p of predicates) assert.deepEqual(t.filter(p).objects(), rows.filter(p));
    assert.deepEqual(t.orderby("y").slice(1).filter(d => d.x >= 0).objects(), rows.slice(1).filter(d => d.x >= 0));
  }
  assert.throws(() => api.table({ x: [] }).filter(d => false && d.missing > 0));
  assert.throws(() => api.table({ x: [1], text: ["x"] }).filter(d => d.x > 0 && d.text - 1), WebAssembly.RuntimeError);
}
const report = { schema: "walz-table-opt-probe-v1", created: new Date().toISOString(), node: process.version, v8: process.versions.v8,
  cpu: cpus()[0]?.model, snapshot, captureSha256: hash(await readFile(join(snapshot, "results.json"))), inputSha256: captured.inputs["262144-numeric"],
  optimizer: { version, sha256: hash(await readFile(optimizer)), command: [optimizer, ...command], closedWorld: false, fastMath: false },
  artifacts: { original: { bytes: original.length, sha256: hash(original) }, O3: { bytes: optimized.length, sha256: hash(optimized) } },
  method: "Separate from profiles. Four variants, rotating order, nine 20ms batches, at least 500ms/32 calls warmup; forced GC and complete row validation outside clocks. Same captured JS facade and inputs; kernel postprocessing only. Not a full language/API conformance suite.", cases: [] };
let sink;
function batch(v, n) {
  let result, checksum = 0; const start = performance.now();
  for (let i = 0; i < n; i++) { result = v.run(); checksum = (checksum + result.numRows()) | 0; }
  const ms = performance.now() - start; sink = result; assert.equal(checksum, Math.imul(v.expected, n)); return ms;
}
const input = JSON.parse(await readFile(join(snapshot, "inputs/262144-numeric.json")));
const rows = input.x.map((x, i) => ({ x, y: input.y[i], id: input.id[i] }));
for (const v of variants) v.table = v.api.table(input);
for (const [name, predicate] of [["simple", d => d.x < 20], ["compound", d => d.x >= -100 && d.x < 100 && (d.y < 32 || d.y >= 96)]]) {
  const expected = rows.filter(predicate), samples = {}, iterations = {};
  for (const v of variants) {
    v.run = () => v.table.filter(predicate); v.expected = expected.length;
    assert.deepEqual(v.run().objects(), expected);
    const end = performance.now() + 500; let calls = 0;
    do { batch(v, 1); calls++; } while (calls < 32 || performance.now() < end);
    let n = 1, ms;
    do { ms = batch(v, n); if (ms >= 10) break; n *= 2; } while (true);
    iterations[v.name] = Math.max(1, Math.floor(n * 20 / ms)); samples[v.name] = [];
  }
  for (let round = 0; round < 9; round++) for (let offset = 0; offset < variants.length; offset++) {
    const v = variants[(round + offset) % variants.length]; await collect();
    samples[v.name].push(batch(v, iterations[v.name]) * 1000 / iterations[v.name]); assert.deepEqual(sink.objects(), expected);
  }
  const medianUs = Object.fromEntries(Object.entries(samples).map(([name, values]) => [name, [...values].sort((a, b) => a - b)[4]]));
  report.cases.push({ name, iterations, samplesUs: samples, medianUs }); console.log(name, medianUs);
}
await verifySnapshot(snapshot);
await copyFile(fileURLToPath(import.meta.url), join(output, "probe.mjs"));
await writeFile(join(output, "probe.json"), JSON.stringify(report, null, 2) + "\n", { flag: "wx" });
