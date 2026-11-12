// Finite memory regression for recent lowering and ABI changes. Build tools run
// in children, outside measured JS heaps. This is evidence, not a leak theorem.
import assert from "node:assert/strict";
import { execFile as callback } from "node:child_process";
import { promisify } from "node:util";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { collect, tick, instrument, instrumentScopes } from "./testing/memory.mjs";

if (!global.gc) throw new Error("use node --expose-gc");
const root = fileURLToPath(new URL("..", import.meta.url));
const output = resolve(process.argv[2] ?? "target/recent-memory-audit/current");
const run = (command, args) => promisify(callback)(command, args, { cwd: root, maxBuffer: 8 * 1024 * 1024 });
const sha = data => createHash("sha256").update(data).digest("hex");
const report = { version: 2, date: new Date().toISOString(), node: process.version,
  platform: process.platform, arch: process.arch, head: (await run("git", ["rev-parse", "HEAD"])).stdout.trim(),
  method: { warmupBatches: 1, measuredBatches: 6, gcsPerSample: 4,
    callsPerBatch: 250, retainedGrowthLimit: 2 * 1024 * 1024, maxCollectionPasses: 8 },
  cases: [], artifacts: {} };
await mkdir(output, { recursive: true });
async function fingerprint(directory) {
  const files = {};
  for (const name of (await readdir(directory)).sort())
    if (/\.(?:wasm|m?js|json)$/.test(name)) files[name] = sha(await readFile(join(directory, name)));
  return files;
}
async function verifyCollected(refs, name, passes = report.method.maxCollectionPasses) {
  for (let pass = 1; pass <= passes; pass++) {
    await collect();
    if (refs.every(ref => ref.deref() === undefined)) return pass;
  }
  throw new Error(`${name}: objects remain after ${passes * 4} GC requests`);
}
async function sample(name, work, inspect, calls = report.method.callsPerBatch) {
  await work(calls); await collect();
  const samples = [];
  for (let round = 0; round < report.method.measuredBatches; round++) {
    await work(calls);
    const observation = { ...await collect(), ...inspect() };
    samples.push(observation);
    console.log(JSON.stringify({ name, round, ...observation }));
  }
  const first = samples[0], last = samples.at(-1), limit = report.method.retainedGrowthLimit;
  const heapGrowth = last.heapUsed - first.heapUsed, arrayBufferGrowth = last.arrayBuffers - first.arrayBuffers;
  assert.ok(heapGrowth < limit, `${name}: heap grew by ${heapGrowth}`);
  assert.ok(arrayBufferGrowth < limit, `${name}: ArrayBuffers grew by ${arrayBufferGrowth}`);
  assert.equal(last.linearBytes, first.linearBytes, `${name}: linear capacity kept growing`);
  report.cases.push({ name, callsPerBatch: calls, heapGrowth, arrayBufferGrowth, samples });
}

// Observe real guest GC objects, with positive controls held through real Wasm
// fields, escaped views, and escaped closure environments.
async function rawGc() {
  const path = join(output, "recent-gc.wasm");
  await run("cargo", ["run", "--locked", "--quiet", "--", "build", "tests/gc-fixtures/recent-optimizations.walz", path]);
  const bytes = await readFile(path);
  report.artifacts.rawGc = sha(bytes);
  const { instance } = await WebAssembly.instantiate(bytes), api = instance.exports;
  const holder = api.root();
  const pinned = (() => { const box = api.make(); api.pin(holder, box); return [new WeakRef(box), new WeakRef(api.data(box))]; })();
  await assert.rejects(verifyCollected(pinned, "pinned guest root", 2), /objects remain/);
  assert.ok(pinned.every(ref => ref.deref() !== undefined));
  api.clear(holder); await verifyCollected(pinned, "cleared guest root");
  const view = (() => { const box = api.make(); return { held: api.view(box), ref: new WeakRef(api.data(box)) }; })();
  await assert.rejects(verifyCollected([view.ref], "live view owns bytes", 2), /objects remain/);
  assert.equal(api["read-view"](view.held), 0);
  const copy = api.copy(view.held); assert.ok(copy);
  view.held = null;
  await verifyCollected([view.ref], "materialized bytes do not retain owner");
  const closure = (() => { const box = api.make(); return { held: api.closure(box), refs: [new WeakRef(box), new WeakRef(api.data(box))] }; })();
  await assert.rejects(verifyCollected(closure.refs, "live closure owns capture", 2), /objects remain/);
  assert.ok(closure.refs.every(ref => ref.deref() !== undefined));
  closure.held = null; await verifyCollected(closure.refs, "discarded closure releases capture");
  const refs = [];
  function exercise(mode) {
    const box = api.make(), data = api.data(box);
    assert.equal(api.exercise(box, mode), 42);
    refs.push(new WeakRef(box), new WeakRef(data));
  }
  for (let i = 0; i < 192; i++) exercise(i % 2);
  const collectionPasses = await verifyCollected(refs, "optimized helper and value frames");
  report.rawGc = { checkedObjects: refs.length + pinned.length + closure.refs.length + 1,
    collectionPasses, allCollected: true, pinnedControl: true, liveViewControl: true,
    detachedCopyControl: true, liveClosureControl: true };
}
await rawGc();

async function build(backend, source, name) {
  const directory = join(output, backend, name);
  const native = backend.startsWith("native-");
  const builder = native ? "build-js-native.mjs" : backend === "core" ? "build-core.mjs" : "build.mjs";
  await run(process.execPath, [join(root, "runtime", builder), source, directory, ...(native ? [backend.slice(7)] : [])]);
  report.artifacts[`${backend}/${name}`] = await fingerprint(directory);
  if (!native) { await instrument(directory, backend); await instrumentScopes(directory); }
  return import(pathToFileURL(join(directory, "index.js")));
}
async function make(runtime) {
  const state = {};
  state.program = await runtime.instantiate({ instantiateCore: async (module, imports) => {
    const instance = await WebAssembly.instantiate(module, imports);
    if (instance.exports.live) state.arena = instance.exports;
    if (instance.exports.__transfer) state.transfer = instance.exports.__transfer;
    return instance;
  } });
  return state;
}
function inspect(state) {
  const tables = state.program.__memory();
  const scopes = state.program.__taskMemory?.() ?? {};
  const linearBytes = state.arena?.memory.buffer.byteLength ?? state.transfer.buffer.byteLength;
  assert.equal(state.arena?.live() ?? 0, 0);
  for (const [key, value] of Object.entries(tables))
    if (key !== "scratchCapacityBytes") assert.equal(value, 0, `${key}: live ABI state`);
  assert.ok(Object.values(scopes).every(n => n === 0), JSON.stringify(scopes));
  return { linearBytes, live: state.arena?.live() ?? 0, tables, scopes };
}

const text = "\ufeff日本🦀".repeat(513), encoded = new TextEncoder().encode(text);
const cases = [
  { name: "views-ranges", source: "tests/component-fixtures/range-optimization.walz",
    work(api, n) { for (let i = 0; i < n; i++) {
      const bytes = new Uint8Array(1024 + (i % 32) * 1024);
      assert.equal(api.scan(bytes, 1, bytes.length - 2), 0n);
      assert.equal(api.nested(bytes, 1, bytes.length - 2, 1, bytes.length - 4), 0n);
    } }, trap: api => api.scan(new Uint8Array(32), -1, 4) },
  { name: "utf8-views", source: "tests/component-fixtures/utf8-views.walz",
    work(api, n) { for (let i = 0; i < n; i++) {
      assert.equal(api.validView(encoded, 0, encoded.length), 1);
      assert.equal(api.length(encoded, 0, encoded.length), encoded.length);
      assert.equal(api.decode(encoded, 0, encoded.length), text);
      assert.equal(api.materialize(encoded, 0, encoded.length), text);
    } }, trap: api => api.validView(encoded, -1, 4) },
  { name: "value-fusion", source: "tests/component-fixtures/value-optimization.walz",
    work(api, n) { for (let i = 0; i < n; i++) {
      assert.equal(api.identity(), 43); assert.equal(api.branch(1, 20), 41);
      assert.equal(api.layout(10, 4, 2), 42); assert.equal(api.handler(), 7);
      assert.deepEqual(api.escaped(i), { tag: "ok", val: i });
    } }, trap: api => api.ignored(0) },
  { name: "angle-applications", source: "examples/angle-applications.walz",
    work(api, n) { for (let i = 0; i < n; i++) {
      assert.equal(api.main(), 42); assert.deepEqual(api.split(i), { tag: "ok", val: [i, 2] });
    } }, badArgument: api => api.split(Symbol("invalid")) },
  { name: "flatbuffers", source: "examples/flatbuffers/main.walz",
    work(api, n) { for (let i = 0; i < n; i++) {
      const value = { id: BigInt(i), name: text, values: Int32Array.of(-1, 0, 42),
        payload: new Uint8Array(1024 + (i % 32) * 1024), enabled: true,
        weight: -0, child: { count: 42, label: "child🦀" } };
      const decoded = api.decode(api.encode(value)); assert.equal(decoded.tag, "ok");
      assert.equal(decoded.val.name, text); assert.equal(decoded.val.payload.length, value.payload.length);
      assert.deepEqual(api.decode(new Uint8Array()), { tag: "err", val: 1 });
    } }, badArgument: api => api.decode(null) },
];
for (const backend of ["core", "component", "native-utf8", "native-js-strings"]) {
  for (const entry of cases) {
    const runtime = await build(backend, entry.source, entry.name), state = await make(runtime);
    await sample(`${backend}/${entry.name}`, n => entry.work(state.program.api, n), () => inspect(state));
    // Keep both a failed-call error and its terminal task while dropping the
    // instance. Error stacks must not pin its arena/transfer memory.
    const history = await (async () => {
      const value = await make(runtime);
      let error;
      try { (entry.trap ?? entry.badArgument)(value.program.api); } catch (e) { error = e; }
      assert.ok(error instanceof Error); inspect(value);
      return { error, memory: new WeakRef(value.arena?.memory ?? value.transfer) };
    })();
    const passes = await verifyCollected([history.memory], `${backend}/${entry.name}: held error`);
    report.cases.at(-1).discardedInstance = { allCollected: true, collectionPasses: passes };
  }
}

// The browser compiler is a long-lived Rust/Wasm instance. Linear pages cannot
// shrink, so check reuse after warmup, including distinct source names/errors.
const compilerDirectory = join(output, "compiler/pkg");
await run("wasm-pack", ["build", "crates/walz-web", "--target", "web", "--release",
  "--out-dir", compilerDirectory, "--out-name", "walz_web", "--locked"]);
report.artifacts.compiler = await fingerprint(compilerDirectory);
const compiler = await import(pathToFileURL(join(compilerDirectory, "walz_web.js")));
const wasm = compiler.initSync({ module: await readFile(join(compilerDirectory, "walz_web_bg.wasm")) });
const sources = await Promise.all(["examples/value-pipeline.walz", "examples/angle-applications.walz", "examples/range-scan.walz"].map(p => readFile(join(root, p), "utf8")));
let serial = 0;
await sample("wasm-compiler/repeated-success-and-errors", async n => {
  for (let i = 0; i < n; i++) {
    const suffix = String(serial++).padStart(8, "0"), valid = i % 4 !== 3;
    const source = valid ? sources[i % sources.length].replaceAll("$main", `$main${suffix}`)
      : `(module (@use "walz:core" (type $tuple)) (func $bad${suffix} (result $tuple<i32,,i32>) (nop)))`;
    const outcome = JSON.parse(compiler.compile_source(source, i % 2 === 0));
    assert.equal(outcome.status, valid ? "success" : "error");
    if (!valid) assert.equal(outcome.diagnostic.code, "E_GENERIC_SYNTAX");
  }
}, () => ({ linearBytes: wasm.memory.buffer.byteLength }), 96);

const files = (await run("git", ["ls-files", "-co", "--exclude-standard", "src", "runtime", "walz", "examples", "tests/gc-fixtures", "tests/component-fixtures", "build.rs", "Cargo.lock", "pnpm-lock.yaml"])).stdout.trim().split("\n");
const sourcesFingerprint = {};
for (const file of [...new Set(files)].sort()) sourcesFingerprint[file] = sha(await readFile(join(root, file)));
await writeFile(join(output, "sources.json"), JSON.stringify(sourcesFingerprint, null, 2) + "\n");
report.sources = { count: Object.keys(sourcesFingerprint).length, sha256: sha(JSON.stringify(sourcesFingerprint)), file: "sources.json" };
await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2) + "\n");
console.log(`memory checks passed: ${join(output, "report.json")}`);
