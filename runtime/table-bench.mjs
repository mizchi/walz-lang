import assert from "node:assert/strict";
import * as reference from "arquero";
import { instantiate } from "./table/index.mjs";
import { build } from "./table/build.mjs";
import { collect } from "./testing/memory.mjs";
import { mkdir, readFile, writeFile, readdir } from "node:fs/promises";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { cpus } from "node:os";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";

if (!global.gc) throw new Error("Use node --expose-gc");
const root = fileURLToPath(new URL("../", import.meta.url));
const output = resolve(process.argv[2]);
const sizes = (process.argv[3] ?? "4096,65536,262144").split(",").map(Number);
assert.ok(sizes.length && sizes.every(n => Number.isInteger(n) && n > 0 && n <= 262144));
await mkdir(dirname(output), { recursive: true }); await mkdir(output);
const run = promisify(callback), sha = bytes => createHash("sha256").update(bytes).digest("hex");
const report = { version: 1, date: new Date().toISOString(), node: process.version, v8: process.versions.v8,
  platform: process.platform, arch: process.arch, cpu: cpus()[0].model,
  head: (await run("git", ["rev-parse", "HEAD"], { cwd: root })).stdout.trim(),
  reference: "arquero@8.0.3", sizes, sources: {}, inputs: {}, artifacts: {}, cases: [],
  method: { samples: 9, minimumWarmupCalls: 8, warmupMs: 120, targetBatchMs: 20,
    resident: "Already ingested tables; packing is warmed before sampling. Expression parsing and row selection allocation are included.",
    cold: "Fresh table ingestion, first filter/cache preparation and row count on every call.",
    controls: "Boxed scalar; packed scalar with identical cache and row handling; f64x2 SIMD; official Arquero; direct JS scan for count tasks only.",
    order: "Variant order rotates each round; GC runs outside clocks. No concurrent builds/tests during samples.",
    verification: "Full rows checked against independent JS filter and Arquero outside clocks; every call consumes row count. Cold input/packed cache allocation is not amortized.",
    summation: "No vector reductions or floating-point reassociation." } };
const sourceNames = ["runtime/table-bench.mjs", "examples/table/kernel.walz", "package.json", "pnpm-lock.yaml"];
for (const directory of ["walz/table", "runtime/table"])
  for (const name of await readdir(join(root, directory))) sourceNames.push(join(directory, name));
for (const name of sourceNames) {
  const bytes = await readFile(join(root, name)), target = join(output, "source", name);
  report.sources[name] = sha(bytes); await mkdir(dirname(target), { recursive: true }); await writeFile(target, bytes);
}
const manifest = {};
const tracked = (await run("git", ["ls-files", "-co", "--exclude-standard", "src", "runtime", "walz", "build.rs", "Cargo.toml", "Cargo.lock", "pnpm-lock.yaml"], { cwd: root })).stdout.trim().split("\n");
for (const name of [...new Set(tracked)].sort()) manifest[name] = sha(await readFile(join(root, name)));
await writeFile(join(output, "source-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
await build(join(output, "kernel"));
report.artifacts.core = sha(await readFile(join(output, "kernel/core.wasm")));
report.artifacts.compiler = sha(await readFile(join(root, "target/debug/walz")));
const engines = [{ name: "arquero", api: reference }];
for (const filter of ["scalar", "packed-scalar", "simd"])
  engines.push({ name: filter, api: await instantiate(pathToFileURL(join(output, "kernel/")), { filter }) });

let sink;
const median = values => [...values].sort((a, b) => a - b)[values.length >>> 1];
function timed(v, n) {
  let checksum = 0, result;
  const start = performance.now();
  for (let i = 0; i < n; i++) { result = v.run(); checksum = (checksum + v.consume(result)) | 0; }
  const ms = performance.now() - start; sink = result;
  return { result, checksum, ms };
}
async function measure(id, operation, variants) {
  const samples = Object.fromEntries(variants.map(v => [v.name, []])), iterations = {}, warmups = {};
  for (const v of variants) {
    const value = v.run(); v.verify(value); v.checksum = v.consume(value) | 0;
    const start = performance.now(); let calls = 0;
    do { timed(v, 1); calls++; } while (calls < report.method.minimumWarmupCalls || performance.now() - start < report.method.warmupMs);
    warmups[v.name] = calls;
    let n = 1, trial;
    do { trial = timed(v, n); if (trial.ms >= 8 || n >= 65536) break; n *= 2; } while (true);
    iterations[v.name] = Math.max(1, Math.min(65536, Math.floor(n * report.method.targetBatchMs / trial.ms)));
  }
  for (let round = 0; round < report.method.samples; round++) {
    for (let offset = 0; offset < variants.length; offset++) {
      const v = variants[(round + offset) % variants.length], n = iterations[v.name];
      await collect(); const value = timed(v, n);
      assert.equal(value.checksum, Math.imul(v.checksum, n)); v.verify(value.result);
      samples[v.name].push(value.ms * 1000 / n);
    }
  }
  const mediansUs = Object.fromEntries(Object.entries(samples).map(([name, values]) => [name, median(values)]));
  report.cases.push({ id, operation, mediansUs, samplesUs: samples, iterations, warmupCalls: warmups });
  await writeFile(join(output, "results.json"), JSON.stringify(report, null, 2) + "\n");
  console.log(JSON.stringify({ id, operation, mediansUs }));
}
for (const n of sizes) for (const nullable of [false, true]) {
  const id = `${n}-${nullable ? "nullable" : "numeric"}`;
  let state = 0x12345678;
  const data = { x: [], y: [], id: [] };
  for (let i = 0; i < n; i++) {
    state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
    data.x.push(nullable && i % 29 === 0 ? undefined : nullable && i % 31 === 0 ? null : nullable && i % 37 === 0 ? NaN : (state >>> 0) % 1000 - 500);
    data.y.push(i % 127); data.id.push(i);
  }
  const input = Buffer.from(JSON.stringify({ x: data.x.map(v => v === undefined ? { tag: "undefined" } : Number.isNaN(v) ? { tag: "NaN" } : v), y: data.y, id: data.id }) + "\n");
  await mkdir(join(output, "inputs"), { recursive: true }); await writeFile(join(output, "inputs", `${id}.json`), input);
  report.inputs[id] = sha(input);
  const rows = data.id.map((id, i) => ({ x: data.x[i], y: data.y[i], id }));
  const resident = engines.map(({ name, api }) => ({ name, api, table: api.table(data) }));
  for (const [operation, predicate] of [["resident-lt-count", d => d.x < 20],
    ["resident-range-count", d => d.x >= -100 && d.x < 100 && (d.y < 32 || d.y >= 96)]]) {
    const expected = rows.filter(predicate);
    const variants = resident.map(({ name, table }) => ({ name, run: () => table.filter(predicate),
      consume: value => value.numRows(), verify: value => assert.deepEqual(value.objects(), expected) }));
    variants.push({ name: "js-scan", run: operation === "resident-lt-count"
      ? () => { let count = 0; for (let i = 0; i < n; i++) if (data.x[i] < 20) count++; return count; }
      : () => { let count = 0; for (let i = 0; i < n; i++) if (data.x[i] >= -100 && data.x[i] < 100 && (data.y[i] < 32 || data.y[i] >= 96)) count++; return count; },
      consume: value => value, verify: value => assert.equal(value, expected.length) });
    await measure(id, operation, variants);
  }
  const predicate = d => d.x < 20, expected = rows.filter(predicate);
  await measure(id, "cold-ingest-filter-count", engines.map(({ name, api }) => ({ name,
    run: () => api.table(data).filter(predicate), consume: value => value.numRows(), verify: value => assert.deepEqual(value.objects(), expected) })));
  await measure(id, "resident-filter-materialize", resident.map(({ name, table }) => ({ name,
    run: () => table.filter(predicate).objects(), consume: value => value.length, verify: value => assert.deepEqual(value, expected) })));
}
void sink;
