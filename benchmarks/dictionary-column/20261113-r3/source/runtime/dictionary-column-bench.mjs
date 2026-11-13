import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { cpus } from "node:os";
import { collect, instrument } from "./testing/memory.mjs";
import { dataset, encode, validate, histogram, postings, countsRaw, countsEncoded, countsHistogram,
  countsValidated, queryRaw, queryEncoded, queryPostings } from "./dictionary-bench/workloads.mjs";

if (!global.gc) throw new Error("use node --expose-gc");
const root = fileURLToPath(new URL("..", import.meta.url));
const output = resolve(process.argv[2] ?? "target/dictionary-bench/current");
const sizes = (process.argv[3] ?? "4096,65536,262144").split(",").map(Number);
const cardinalities = (process.argv[4] ?? "4,64,1024").split(",").map(Number);
assert.ok(sizes.length && sizes.every(n => Number.isInteger(n) && n > 0 && n <= 262144));
assert.ok(cardinalities.length && cardinalities.every(k => Number.isInteger(k) && k > 0 && k <= Math.min(...sizes)));
const configs = sizes.flatMap(n => cardinalities.map(k => ({ n, k, unicode: false })));
if (sizes.includes(65536) && cardinalities.includes(64)) configs.push({ n: 65536, k: 64, unicode: true });
await mkdir(dirname(output), { recursive: true }); await mkdir(output);
const run = (command, args) => promisify(callback)(command, args, { cwd: root, maxBuffer: 32 * 1024 * 1024 });
const sha = value => createHash("sha256").update(value).digest("hex");
const report = {
  version: 2, date: new Date().toISOString(), node: process.version, v8: process.versions.v8,
  platform: process.platform, arch: process.arch, cpu: cpus()[0].model,
  head: (await run("git", ["rev-parse", "HEAD"])).stdout.trim(),
  method: { samples: 9, warmupMs: 120, minimumWarmupCalls: 8, targetBatchMs: 20,
    variantSpecificIterations: true, queriesPerCountBatch: 32, gcOutsideClock: true,
    resident: "strings/lists and amounts are already in each engine; packing/construction is measured separately",
    portable: "actual Core/Component APIs, copies, validation, construction, queries, and output conversion",
    stringSharing: "both resident engines reuse vocabulary strings; portable calls pay actual string ABI costs",
    controls: "raw strings, linear/Map code lookup, scalar Wasm, histogram counts, and typed-array postings",
    queryImplementations: "resident walz = composed bitmap path, walzFused = fused query; portable core/component = fused, coreComposed/componentComposed = bitmap path; same inputs/outputs and checksum readers",
    outputChecks: "every code/count/returned row is checked outside clocks; per-call checksums consume each result",
    sums: "bounded captured integers make JS Number exact; Wasm sums return i64/BigInt" },
  configs, cases: [], setup: {}, inputs: {}, sources: {}, artifacts: {},
};
for (const name of ["walz/simd/bitmap.walz", "walz/simd/i32-column.walz", "walz/simd/dictionary-column.walz",
  "examples/dictionary-column/bench-resident.walz", "examples/dictionary-column/portable.walz",
  "examples/dictionary-column/fused.walz",
  "runtime/dictionary-column-bench.mjs", "runtime/dictionary-bench/workloads.mjs", "runtime/dictionary-bench.test.mjs",
  "tests/dictionary-column.test.walz", "tests/dictionary_column.rs", "runtime/dictionary-column.test.mjs",
  "runtime/e2e/dictionary-column.spec.mjs"]) {
  const bytes = await readFile(join(root, name)), target = join(output, "source", name);
  report.sources[name] = sha(bytes); await mkdir(dirname(target), { recursive: true }); await writeFile(target, bytes);
}
const manifest = {};
const files = (await run("git", ["ls-files", "-co", "--exclude-standard", "src", "runtime", "walz", "build.rs", "Cargo.toml", "Cargo.lock", "pnpm-lock.yaml"])).stdout.trim().split("\n");
for (const name of [...new Set(files)].sort()) manifest[name] = sha(await readFile(join(root, name)));
await writeFile(join(output, "source-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
report.sourceManifestSha256 = sha(JSON.stringify(manifest));
await run("cargo", ["build", "--locked", "--bin", "walz"]);
report.compilerBinarySha256 = sha(await readFile(join(root, "target/debug/walz")));
const wasmFile = join(output, "resident.wasm");
await run("cargo", ["run", "--locked", "--quiet", "--", "build", "examples/dictionary-column/bench-resident.walz", wasmFile]);
const wasm = await readFile(wasmFile); report.artifacts.resident = sha(wasm);
const api = (await WebAssembly.instantiate(wasm)).instance.exports;
const portable = [];
for (const backend of ["core", "component"]) {
  const directory = join(output, backend);
  await run(process.execPath, [join(root, "runtime", backend === "core" ? "build-core.mjs" : "build.mjs"),
    "examples/dictionary-column/portable.walz", directory]);
  await instrument(directory, backend);
  report.artifacts[backend] = {};
  for (const name of (await readdir(directory)).filter(name => /\.(?:m?js|wasm|json)$/.test(name)))
    report.artifacts[backend][name] = sha(await readFile(join(directory, name)));
  let arena;
  const runtime = await import(pathToFileURL(join(directory, "index.js")));
  const program = await runtime.instantiate({ instantiateCore: async (m, imports) => {
    const value = await WebAssembly.instantiate(m, imports); if (value.exports.live) arena = value.exports; return value;
  } });
  portable.push({ name: backend, api: program.api, clean() {
    assert.equal(arena.live(), 0); assert.ok(Object.values(program.__memory()).every(n => n === 0));
  } });
}

let sink;
function timed(v, iterations) {
  let result, checksum = 0;
  const start = performance.now();
  for (let i = 0; i < iterations; i++) { result = v.run(); checksum = (checksum + v.consume(result)) | 0; }
  const ms = performance.now() - start;
  sink = result; return { result, checksum, ms };
}
const median = values => [...values].sort((a, b) => a - b)[values.length >>> 1];
async function measure(id, mode, operation, variants) {
  const iterations = {}, warmups = {}, samples = Object.fromEntries(variants.map(v => [v.name, []]));
  for (const v of variants) {
    const first = v.run(); v.verify(first); v.clean?.(); v.expectedChecksum = v.consume(first) | 0;
    const start = performance.now(); let calls = 0;
    do { timed(v, 1); calls++; } while (calls < report.method.minimumWarmupCalls || performance.now() - start < report.method.warmupMs);
    warmups[v.name] = calls;
    let n = 1, trial;
    do { trial = timed(v, n); if (trial.ms >= 8 || n >= 1048576) break; n *= 2; } while (true);
    iterations[v.name] = Math.max(1, Math.min(1048576, Math.floor(n * report.method.targetBatchMs / trial.ms)));
  }
  for (let round = 0; round < report.method.samples; round++) {
    for (let offset = 0; offset < variants.length; offset++) {
      const v = variants[(round + offset) % variants.length], n = iterations[v.name];
      await collect(); const measured = timed(v, n);
      assert.equal(measured.checksum, Math.imul(v.expectedChecksum, n));
      v.verify(measured.result); v.clean?.();
      samples[v.name].push(measured.ms * 1000 / n);
    }
  }
  const mediansUs = Object.fromEntries(Object.entries(samples).map(([name, values]) => [name, median(values)]));
  report.cases.push({ id, mode, operation, iterations, warmupCalls: warmups, mediansUs, samplesUs: samples });
  console.log(JSON.stringify({ id, mode, operation, mediansUs }));
}
const normQuery = result => ({ count: result.count, sum: Number(result.sum), indices: Int32Array.from(result.indices) });
const queryConsume = result => (result.count + Number(result.sum) + (result.indices[0] ?? 0)) | 0;
function inputNumbers(values) { const value = api["numbers-new"](values.length); values.forEach((x, i) => api["number-set"](value, i, x)); return value; }
function readCounts(value) { return Int32Array.from({ length: api["counts-len"](value) }, (_, i) => api["counts-get"](value, i)); }
function readQuery(value) { return { count: api["answer-count"](value), sum: Number(api["answer-sum"](value)), indices: readCounts(api["answer-indices"](value)) }; }
const gcCountsConsume = value => api["counts-get"](value, 0) + api["counts-get"](value, 31);
const jsCountsConsume = value => value[0] + value[31];
const gcQueryConsume = value => (api["answer-count"](value) + Number(api["answer-sum"](value))) | 0;
function elapsed(fn) { const start = performance.now(), value = fn(); return { value, ms: performance.now() - start }; }
async function saveInputs(id, d) {
  const directory = join(output, "inputs", id); await mkdir(directory, { recursive: true });
  for (const [name, values] of [["ids", d.ids], ["amounts", d.amounts], ["queries", d.queryIds]]) {
    const bytes = Buffer.alloc(values.length * 4); values.forEach((value, i) => bytes.writeInt32LE(value, i * 4));
    await writeFile(join(directory, `${name}.bin`), bytes); report.inputs[`${id}/${name}.bin`] = sha(bytes);
  }
  const metadata = Buffer.from(JSON.stringify({ n: d.n, cardinality: d.cardinality, unicode: d.unicode,
    vocabulary: d.vocabulary, lo: d.lo, hi: d.hi, values: "vocabulary[ids[i]]", positiveControl: "amounts[0] = 42" }, null, 2) + "\n");
  await writeFile(join(directory, "data.json"), metadata); report.inputs[`${id}/data.json`] = sha(metadata);
}

for (const { n, k, unicode } of configs) {
  const id = `${n}-${k}-${unicode ? "unicode" : "ascii"}`, d = dataset(n, k, unicode);
  await saveInputs(id, d);
  const encoded = encode(d.values), expectedCounts = countsRaw(d.values, d.queries);
  const make = elapsed(() => {
    const vocabulary = api["strings-new"](k + 1);
    for (let i = 0; i <= k; i++) api["label-set"](vocabulary, i, i, Number(unicode));
    const rows = api["strings-new"](n), queries = api["strings-new"](32), one = api["strings-new"](1), keys = api["strings-new"](k);
    d.ids.forEach((value, i) => api["label-copy"](rows, i, vocabulary, value));
    d.queryIds.forEach((value, i) => api["label-copy"](queries, i, vocabulary, value));
    for (let i = 0; i < k; i++) api["label-copy"](keys, i, vocabulary, i);
    api["label-copy"](one, 0, vocabulary, 0);
    const codes = inputNumbers(d.ids), amounts = inputNumbers(d.amounts);
    return { vocabulary, codes, rowList: api["strings-freeze"](rows), keyList: api["strings-freeze"](keys),
      queries: api["strings-freeze"](queries), one: api["strings-freeze"](one),
      codeList: api["numbers-freeze"](codes), amountColumn: api.amounts(api["numbers-freeze"](amounts)) };
  });
  const state = make.value, built = elapsed(() => api.encode(state.rowList)), map = elapsed(() => histogram(d.values)), index = elapsed(() => postings(encoded));
  report.setup[id] = { walzInputTransferAndFreezeMs: make.ms, walzEncodeObservationMs: built.ms,
    jsHistogramObservationMs: map.ms, jsPostingsObservationMs: index.ms, histogram: "separate counts-only index", postings: "separate sorted row-ID index" };
  const column = built.value, ids = new Map(encoded.dictionary.map((key, i) => [key, i]));
  const verifyColumn = value => {
    assert.equal(api["dictionary-len"](value), k);
    for (let i = 0; i < k; i++) assert.equal(api["dictionary-key-equal"](value, i, state.vocabulary, i), 1);
    for (let i = 0; i < n; i++) assert.equal(api["column-code"](value, i), d.ids[i]);
  };
  const consumeColumn = value => api["dictionary-len"](value) ^ api["column-code"](value, n - 1);
  const verifyEncoded = value => assert.deepEqual({ dictionary: value.dictionary, codes: Int32Array.from(value.codes) }, encoded);
  const consumeEncoded = value => value.dictionary.length ^ value.codes[n - 1];
  await measure(id, "resident-build", "encode", [
    { name: "jsMap", run: () => encode(d.values), consume: consumeEncoded, verify: verifyEncoded },
    { name: "walz", run: () => api.encode(state.rowList), consume: consumeColumn, verify: verifyColumn },
  ]);
  await measure(id, "resident-build", "from-codes", [
    { name: "jsValidated", run: () => validate(encoded), consume: consumeEncoded, verify: verifyEncoded },
    { name: "walz", run: () => api["from-codes"](state.keyList, state.codeList), consume: consumeColumn, verify: verifyColumn },
  ]);
  const checkCounts = result => assert.deepEqual(Int32Array.from(result), expectedCounts);
  await measure(id, "resident", "counts-32", [
    { name: "jsStrings", run: () => countsRaw(d.values, d.queries), consume: jsCountsConsume, verify: checkCounts },
    { name: "jsCodesLinear", run: () => countsEncoded(encoded, d.queries), consume: jsCountsConsume, verify: checkCounts },
    { name: "jsCodesMap", run: () => countsEncoded(encoded, d.queries, ids), consume: jsCountsConsume, verify: checkCounts },
    { name: "jsHistogram", run: () => countsHistogram(map.value, d.queries), consume: jsCountsConsume, verify: checkCounts },
    { name: "walzScalar", run: () => api["scalar-count-many"](column, state.codes, state.queries), consume: gcCountsConsume, verify: result => checkCounts(readCounts(result)) },
    { name: "walz", run: () => api["count-many"](column, state.queries), consume: gcCountsConsume, verify: result => checkCounts(readCounts(result)) },
  ]);
  const key = d.vocabulary[0], answer = queryRaw(d.values, d.amounts, key, d.lo, d.hi);
  const checkQuery = result => assert.deepEqual(normQuery(result), answer);
  const jsQueryVariants = [
    { name: "jsStrings", run: () => queryRaw(d.values, d.amounts, key, d.lo, d.hi), consume: queryConsume, verify: checkQuery },
    { name: "jsCodes", run: () => queryEncoded(encoded, d.amounts, key, d.lo, d.hi), consume: queryConsume, verify: checkQuery },
    { name: "jsCodesBranchless", run: () => queryEncoded(encoded, d.amounts, key, d.lo, d.hi, true), consume: queryConsume, verify: checkQuery },
    { name: "jsPostings", run: () => queryPostings(index.value, d.amounts, key, d.lo, d.hi), consume: queryConsume, verify: checkQuery },
  ];
  await measure(id, "resident", "query-rows-sum", [...jsQueryVariants,
    { name: "walz", run: () => api.query(column, state.amountColumn, state.one, d.lo, d.hi), consume: gcQueryConsume, verify: result => checkQuery(readQuery(result)) },
    { name: "walzFused", run: () => api["query-fused"](column, state.amountColumn, state.one, d.lo, d.hi), consume: gcQueryConsume, verify: result => checkQuery(readQuery(result)) },
  ]);
  await measure(id, "portable", "encode", [
    { name: "jsMap", run: () => encode(d.values), consume: consumeEncoded, verify: verifyEncoded },
    ...portable.map(p => ({ name: p.name, run: () => p.api.encode(d.values), consume: consumeEncoded, verify: verifyEncoded, clean: p.clean })),
  ]);
  await measure(id, "portable", "raw-counts-32", [
    { name: "jsHistogramBuild", run: () => countsHistogram(histogram(d.values), d.queries), consume: jsCountsConsume, verify: checkCounts },
    { name: "jsCodesBuildScan", run: () => countsEncoded(encode(d.values), d.queries), consume: jsCountsConsume, verify: checkCounts },
    ...portable.map(p => ({ name: p.name, run: () => p.api.countMany(d.values, d.queries), consume: jsCountsConsume, verify: checkCounts, clean: p.clean })),
  ]);
  await measure(id, "portable", "encoded-counts-32", [
    { name: "jsValidatedHistogram", run: () => countsValidated(encoded, d.queries), consume: jsCountsConsume, verify: checkCounts },
    { name: "jsValidatedScan", run: () => countsEncoded(validate(encoded), d.queries), consume: jsCountsConsume, verify: checkCounts },
    ...portable.map(p => ({ name: p.name, run: () => p.api.countEncoded(encoded, d.queries), consume: jsCountsConsume, verify: checkCounts, clean: p.clean })),
  ]);
  await measure(id, "portable", "query-rows-sum", [jsQueryVariants[0],
    { name: "jsIndexBuild", run: () => queryPostings(postings(encode(d.values)), d.amounts, key, d.lo, d.hi), consume: queryConsume, verify: checkQuery },
    ...portable.map(p => ({ name: p.name, run: () => p.api.query(d.values, d.amounts, [key], d.lo, d.hi), consume: queryConsume, verify: checkQuery, clean: p.clean })),
    ...portable.map(p => ({ name: `${p.name}Composed`, run: () => p.api.queryComposed(d.values, d.amounts, [key], d.lo, d.hi), consume: queryConsume, verify: checkQuery, clean: p.clean })),
  ]);
  sink = null;
  await writeFile(join(output, "progress.json"), JSON.stringify(report, null, 2) + "\n");
}
await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2) + "\n");
console.log(`report: ${join(output, "report.json")}`);
