// Resident and copy-inclusive measurements are separate experiments. Each run
// saves inputs, sources, tested artifacts and samples in a fresh directory.
import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { gzipSync, brotliCompressSync } from "node:zlib";
import { cpus } from "node:os";
import { collect, instrument } from "./testing/memory.mjs";

if (!global.gc) throw new Error("use node --expose-gc");
const root = fileURLToPath(new URL("..", import.meta.url));
const output = resolve(process.argv[2] ?? "target/simd-collections-bench/current");
const sizes = (process.argv[3] ?? "128,4096,65536,262144").split(",").map(Number);
assert.ok(sizes.length && sizes.every(n => Number.isInteger(n) && n > 0 && n <= 1048576));
const run = (command, args) => promisify(callback)(command, args, { cwd: root, maxBuffer: 32 * 1024 * 1024 });
const sha = value => createHash("sha256").update(value).digest("hex");
await mkdir(dirname(output), { recursive: true });
await mkdir(output); // Refuse replacement of previous measurements.
const report = {
  version: 3, date: new Date().toISOString(), node: process.version, v8: process.versions.v8,
  platform: process.platform, arch: process.arch, cpu: cpus()[0].model,
  head: (await run("git", ["rev-parse", "HEAD"])).stdout.trim(),
  method: { samples: 9, warmupCalls: 1024, warmupMs: 120, targetBatchMs: 20, gcOutsideClock: true,
    resident: "pack once, then query opaque GC objects; JS uses the same typed arrays",
    portable: "actual typed-list ABI input copies, packing and result conversion on every call",
    jsControls: "both branching and branchless range/sum loops; bitmap uses SWAR popcount",
    wasmControl: "scalar checked GC-array loops over the same values; no SIMD",
    jsSums: "Number sums are exact for the bounded captured values; Wasm i64 results cross as BigInt" },
  sizes, cases: [], inputs: {}, sources: {}, artifacts: {}, setupMs: {},
};
const sourceFiles = ["walz/simd/bitmap.walz", "walz/simd/i32-column.walz",
  "examples/simd-collections/resident.walz", "examples/simd-collections/portable.walz",
  "runtime/simd-collections-bench.mjs", "runtime/simd-collections.test.mjs"];
for (const name of sourceFiles) {
  const bytes = await readFile(join(root, name)); report.sources[name] = sha(bytes);
  const path = join(output, "source", name); await mkdir(dirname(path), { recursive: true }); await writeFile(path, bytes);
}
const compilerSources = {};
for (const name of (await run("git", ["ls-files", "src", "build.rs", "Cargo.lock", "Cargo.toml"])).stdout.trim().split("\n"))
  compilerSources[name] = sha(await readFile(join(root, name)));
await writeFile(join(output, "compiler-sources.json"), JSON.stringify(compilerSources, null, 2) + "\n");
await run("cargo", ["build", "--locked", "--bin", "walz"]);
report.compiler = { binarySha256: sha(await readFile(join(root, "target/debug/walz"))),
  sourceManifestSha256: sha(JSON.stringify(compilerSources)) };
const path = join(output, "resident.wasm");
await run("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/simd-collections/resident.walz", path]);
const binary = await readFile(path);
report.artifacts.resident = { sha256: sha(binary), bytes: binary.length, gzip: gzipSync(binary, { level: 9 }).length,
  brotli: brotliCompressSync(binary).length };
const api = (await WebAssembly.instantiate(binary)).instance.exports;
function input(values) { const x = api.input(values.length); for (let i = 0; i < values.length; i++) api.set(x, i, values[i]); return x; }
function popcount(x) {
  x -= (x >>> 1) & 0x55555555;
  x = (x & 0x33333333) + ((x >>> 2) & 0x33333333);
  return Math.imul((x + (x >>> 4)) & 0x0f0f0f0f, 0x01010101) >>> 24;
}
let sink = 0;
function timed(fn, count) {
  let checksum = 0;
  const start = performance.now();
  for (let i = 0; i < count; i++) checksum = (checksum + Number(fn())) | 0;
  const ms = performance.now() - start; sink ^= checksum;
  return { ms, checksum };
}
const median = values => [...values].sort((a, b) => a - b)[values.length >>> 1];
async function measure(mode, operation, n, variants) {
  for (const variant of variants) {
    assert.equal(variant.fn(), variant.expected);
    const start = performance.now(); let calls = 0;
    do { timed(variant.fn, 16); calls += 16; }
    while (calls < report.method.warmupCalls || performance.now() - start < report.method.warmupMs);
  }
  const perCall = Math.max(...variants.map(v => timed(v.fn, 32).ms / 32));
  const iterations = Math.max(1, Math.min(1000000, Math.floor(report.method.targetBatchMs / perCall)));
  const samples = Object.fromEntries(variants.map(v => [v.name, []]));
  for (let round = 0; round < report.method.samples; round++) {
    for (let offset = 0; offset < variants.length; offset++) {
      const v = variants[(round + offset) % variants.length];
      await collect();
      const value = timed(v.fn, iterations);
      assert.equal(value.checksum, Math.imul(Number(v.expected) | 0, iterations));
      assert.equal(v.fn(), v.expected); v.clean?.();
      samples[v.name].push(value.ms * 1000 / iterations);
    }
  }
  const mediansUs = Object.fromEntries(Object.entries(samples).map(([name, values]) => [name, median(values)]));
  report.cases.push({ mode, operation, n, iterations, mediansUs, samplesUs: samples,
    expected: String(variants[0].expected) });
  console.log(JSON.stringify({ mode, operation, n, mediansUs }));
}
const data = [];
for (const n of sizes) {
  let seed = (42 ^ n) | 0;
  const next = () => seed = (Math.imul(seed, 1664525) + 1013904223) | 0;
  const values = Int32Array.from({ length: n }, () => (next() >>> 8) % 8192 - 4096);
  const a = Int32Array.from({ length: Math.ceil(n / 32) }, next), b = Int32Array.from(a, next);
  if (n % 32) { a[a.length - 1] &= 2 ** (n % 32) - 1; b[b.length - 1] &= 2 ** (n % 32) - 1; }
  for (const [name, array] of [["values", values], ["a", a], ["b", b]]) {
    const bytes = Buffer.alloc(array.length * 4); array.forEach((v, i) => bytes.writeInt32LE(v, i * 4));
    const file = `${n}-${name}.bin`; await writeFile(join(output, file), bytes);
    report.inputs[file] = { sha256: sha(bytes), elements: array.length, format: "i32 little-endian" };
  }
  const start = performance.now();
  const source = input(values), sourceA = input(a), sourceB = input(b);
  const column = api.column(source), ba = api.bitmap(n, sourceA), bb = api.bitmap(n, sourceB);
  report.setupMs[n] = performance.now() - start;
  const jsIntersection = () => { let sum = 0; for (let i = 0; i < a.length; i++) sum += popcount(a[i] & b[i]); return sum; };
  const jsRange = () => { let count = 0; for (let i = 0; i < n; i++) count += values[i] >= -500 && values[i] < 500; return count; };
  const jsRangeBranchless = () => { let count = 0; for (let i = 0; i < n; i++) count += (values[i] >= -500) & (values[i] < 500); return count; };
  const jsSum = () => { let sum = 0; for (let i = 0; i < n; i++) if ((a[i >>> 5] >>> (i & 31)) & 1) sum += values[i]; return sum; };
  const jsSumBranchless = () => { let sum = 0; for (let i = 0; i < n; i++) sum += values[i] * ((a[i >>> 5] >>> (i & 31)) & 1); return sum; };
  await measure("resident", "intersection-count", n, [
    { name: "js", fn: jsIntersection, expected: jsIntersection() },
    { name: "walzScalar", fn: () => api["scalar-intersection-count"](sourceA, sourceB), expected: jsIntersection() },
    { name: "walz", fn: () => api["bitmap-intersection-count"](ba, bb), expected: jsIntersection() },
  ]);
  await measure("resident", "count-between", n, [
    { name: "js", fn: jsRange, expected: jsRange() },
    { name: "jsBranchless", fn: jsRangeBranchless, expected: jsRange() },
    { name: "walzScalar", fn: () => api["scalar-count-between"](source, -500, 500), expected: jsRange() },
    { name: "walz", fn: () => api["column-count-between"](column, -500, 500), expected: jsRange() },
  ]);
  await measure("resident", "sum-where", n, [
    { name: "js", fn: jsSum, expected: jsSum() },
    { name: "jsBranchless", fn: jsSumBranchless, expected: jsSum() },
    { name: "walzScalar", fn: () => api["scalar-sum-where"](source, sourceA), expected: BigInt(jsSum()) },
    { name: "walz", fn: () => api["column-sum-where"](column, ba), expected: BigInt(jsSum()) },
  ]);
  data.push({ n, values, jsRange, jsRangeBranchless });
}
const portable = [];
for (const backend of ["core", "component"]) {
  const directory = join(output, backend);
  await run(process.execPath, [join(root, "runtime", backend === "core" ? "build-core.mjs" : "build.mjs"),
    "examples/simd-collections/portable.walz", directory]);
  await instrument(directory, backend);
  report.artifacts[backend] = {};
  for (const name of (await readdir(directory)).filter(n => /\.(?:m?js|wasm|json)$/.test(n)))
    report.artifacts[backend][name] = sha(await readFile(join(directory, name)));
  let arena;
  const runtime = await import(pathToFileURL(join(directory, "index.js")));
  const instance = await runtime.instantiate({ instantiateCore: async (m, imports) => {
    const value = await WebAssembly.instantiate(m, imports); if (value.exports.live) arena = value.exports; return value;
  } });
  portable.push({ backend, api: instance.api, clean() {
    assert.equal(arena.live(), 0); assert.ok(Object.values(instance.__memory()).every(n => n === 0));
  } });
}
for (const { n, values, jsRange, jsRangeBranchless } of data) {
  const expected = jsRange();
  await measure("one-shot", "count-between", n, [
    { name: "js", fn: jsRange, expected },
    { name: "jsBranchless", fn: jsRangeBranchless, expected },
    ...portable.map(v => ({ name: v.backend, fn: () => v.api.countBetween(values, -500, 500), expected, clean: v.clean })),
  ]);
}
report.sink = sink;
await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2) + "\n");
console.log(`report: ${join(output, "report.json")}`);
