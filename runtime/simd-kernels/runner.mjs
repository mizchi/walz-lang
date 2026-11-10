import assert from "node:assert/strict";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
import { cpus } from "node:os";
import { createHash } from "node:crypto";
import { corpus } from "./workloads.mjs";

export const hash = bytes => createHash("sha256").update(bytes).digest("hex");
export const median = xs => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
let sink = 0;
function batch(fn, count) {
  let total = 0;
  const start = performance.now();
  for (let i = 0; i < count; i++) total += Number(fn());
  const elapsed = performance.now() - start;
  sink = total;
  return elapsed;
}
// Calibrate independently, then rotate order. All measured values are consumed.
export function measure(variants, expected, rounds = 9, targetMs = 8) {
  const entries = Object.entries(variants), results = {};
  for (const [name, fn] of entries) {
    assert.equal(Number(fn()), expected, `${name}: reference mismatch`);
    let calls = 1;
    while (calls < 1 << 18 && batch(fn, calls) < targetMs) calls *= 2;
    batch(fn, calls); // Warm the selected batch after calibration.
    results[name] = { callsPerSample: calls, samplesNs: [] };
  }
  for (let round = 0; round < rounds; round++)
    for (let j = 0; j < entries.length; j++) {
      const [name, fn] = entries[(j + round) % entries.length], result = results[name];
      assert.equal(Number(fn()), expected, name);
      result.samplesNs.push(batch(fn, result.callsPerSample) * 1e6 / result.callsPerSample);
    }
  for (const result of Object.values(results)) {
    result.medianNs = median(result.samplesNs);
    result.minNs = Math.min(...result.samplesNs);
    result.maxNs = Math.max(...result.samplesNs);
  }
  return results;
}
async function instantiate(path) {
  const { instance } = await WebAssembly.instantiate(await readFile(path));
  return instance.exports;
}
function load(api, kind, xs) {
  const k = kind === "bytes" ? "i32" : kind, out = api[`${k}-new`](xs.length);
  for (let i = 0; i < xs.length; i++) api[`${k}-set`](out, i, xs[i]);
  return out;
}
async function linearReference(directory, c) {
  const feature = { "i32-sum": "i32-array", "f32-dot": "f32-vector", "bitmap-intersection": "bitmap" }[c.name] ?? "bytes";
  const api = await instantiate(join(directory, "reference", feature, "kernels.wasm"));
  const padded = (c.a.length + 3) & ~3, width = c.kind === "bytes" ? 1 : 4;
  // The Zig modules reserve page zero for their stack. Scratch is reused.
  const left = 65536, right = left + Math.ceil(padded * width / 16) * 16;
  const required = right + padded * width;
  if (required > api.memory.buffer.byteLength) api.memory.grow(Math.ceil((required - api.memory.buffer.byteLength) / 65536));
  const ArrayType = c.kind === "f32" ? Float32Array : c.kind === "bytes" ? Uint8Array : Int32Array;
  const a = new ArrayType(api.memory.buffer, left, c.a.length), b = c.b && new ArrayType(api.memory.buffer, right, c.b.length);
  const copy = () => { a.set(c.a); if (b) b.set(c.b); };
  copy();
  const kernel = c.name === "i32-sum" ? () => api.sum(left, c.a.length)
    : c.name === "f32-dot" ? () => api.dot(left, right, c.a.length)
    : c.name === "bitmap-intersection" ? () => api.intersection_count(left, right, c.a.length)
    : () => api.find_byte(left, c.a.length, c.needle);
  return { resident: kernel, copyInclusive: () => { copy(); return kernel(); } };
}

export async function verify(directory) {
  const manifest = JSON.parse(await readFile(join(directory, "capture.json")));
  for (const [name, expected] of Object.entries(manifest.hashes))
    assert.equal(hash(await readFile(join(directory, name))), expected, `snapshot changed: ${name}`);
  return manifest;
}
function table(report, group, variants) {
  const lines = [`| workload | n | ${variants.join(" | ")} |`, `| --- | ---: | ${variants.map(() => "---:").join(" | ")} |`];
  for (const c of report.cases)
    lines.push(`| ${c.name} | ${c.length} | ${variants.map(v => (c[group][v].medianNs / 1000).toFixed(3)).join(" | ")} |`);
  return lines.join("\n");
}
export async function run(directory, { rounds = 9, sizes = [16, 1024, 4096, 16384, 262144], targetMs = 8 } = {}) {
  assert.ok(Number.isInteger(rounds) && rounds >= 5 && rounds <= 31);
  assert.ok(sizes.length && sizes.every(n => Number.isInteger(n) && n > 0 && n <= 1048576));
  assert.ok(targetMs >= 1 && targetMs <= 100);
  const manifest = await verify(directory);
  const api = await instantiate(join(directory, "resident.wasm"));
  const adapters = {};
  for (const backend of ["core", "component"]) {
    const { instantiate } = await import(pathToFileURL(join(directory, backend, "index.js")));
    let arena;
    const instance = await instantiate({ instantiateCore: async (m, i) => {
      const instance = await WebAssembly.instantiate(m, i);
      if (instance.exports.live) arena = instance.exports;
      return instance;
    } });
    adapters[backend] = { api: instance.api, arena: () => arena };
  }
  const report = {
    schema: "walz-simd-kernels-v1", created: new Date().toISOString(),
    node: process.version, v8: process.versions.v8, cpu: cpus()[0]?.model,
    platform: process.platform, arch: process.arch, execArgv: process.execArgv,
    rounds, targetMs, sizes, capture: hash(await readFile(join(directory, "capture.json"))),
    reference: manifest.reference, sizesBytes: manifest.sizesBytes, cases: [],
  };
  for (const length of sizes) for (const c of corpus(length)) {
    const a = load(api, c.kind, c.a), b = c.b && load(api, c.kind, c.b);
    const pack = xs => api[c.kind === "f32" ? "pack-f32" : c.kind === "bytes" ? "pack-bytes" : "pack-i32"](xs);
    const pa = pack(a), pb = b && pack(b);
    const scalar = api[c.scalar], simd = api[c.simd];
    const scalarRun = b ? () => scalar(a, b) : c.kind === "bytes" ? () => scalar(a, c.needle) : () => scalar(a);
    const simdRun = pb ? () => simd(pa, pb) : c.kind === "bytes" ? () => simd(pa, c.needle) : () => simd(pa);
    const packRun = b ? () => simd(pack(a), pack(b)) : c.kind === "bytes" ? () => simd(pack(a), c.needle) : () => simd(pack(a));
    const linear = await linearReference(directory, c);
    const resident = measure({
      js: c.js,
      walzScalar: scalarRun,
      walzSimd: simdRun,
      jsimdKernel: linear.resident,
      walzPackAndSimd: packRun,
    }, c.expected, rounds, targetMs);
    const boundary = { jsimdScratchCopy: linear.copyInclusive };
    for (const [backend, { api }] of Object.entries(adapters)) for (const kind of ["Scalar", "Simd"]) {
      const fn = api[`${c.portable}${kind}`];
      boundary[`${backend}${kind}`] = c.b ? () => fn(c.a, c.b) : c.kind === "bytes" ? () => fn(c.a, c.needle) : () => fn(c.a);
    }
    const copyInclusive = measure(boundary, c.expected, rounds, targetMs);
    for (const adapter of Object.values(adapters)) assert.equal(adapter.arena().live(), 0, "ABI buffer leak");
    const inputHash = hash(Buffer.concat([c.a, c.b].filter(Boolean).map(a => Buffer.from(a.buffer, a.byteOffset, a.byteLength))));
    report.cases.push({ name: c.name, length, inputHash, expected: c.expected, resident, copyInclusive });
    console.log(`${c.name} n=${length}: SIMD/JS ${(resident.js.medianNs / resident.walzSimd.medianNs).toFixed(2)}x; SIMD/scalar ${(resident.walzScalar.medianNs / resident.walzSimd.medianNs).toFixed(2)}x`);
    global.gc?.();
  }
  assert.ok(Number.isFinite(sink));
  const results = join(directory, "results", report.created.replaceAll(/[:.]/g, "-"));
  await mkdir(results, { recursive: true });
  await writeFile(join(results, "report.json"), JSON.stringify(report, null, 2) + "\n", { flag: "wx" });
  const text = `# SIMD kernel measurements\n\n${report.node} / ${report.cpu}; ${rounds} rounds. Median microseconds/call.\n\nResident excludes construction and packing; walzPackAndSimd includes packing from existing scalar GC arrays. jsimdKernel is the captured raw linear-memory kernel, without its public JS wrapper.\n\n${table(report, "resident", ["js", "walzScalar", "walzSimd", "jsimdKernel", "walzPackAndSimd"])}\n\nCopy-inclusive includes input copies and the kernel. Walz uses its production value adapters, GC conversion and packing; jsimdScratchCopy reuses preallocated linear scratch and excludes its public allocator/wrapper. These are representation/boundary costs, not equal API implementations.\n\n${table(report, "copyInclusive", ["jsimdScratchCopy", "coreScalar", "coreSimd", "componentScalar", "componentSimd"])}\n`;
  await writeFile(join(results, "report.md"), text, { flag: "wx" });
  console.log(results);
  return { report, results };
}
