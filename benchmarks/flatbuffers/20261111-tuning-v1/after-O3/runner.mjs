import assert from "node:assert/strict";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
import { cpus, endianness, loadavg } from "node:os";
import { hash, materialize } from "./workloads.mjs";
import { createReference } from "./reference.mjs";

let sink;
const median = xs => { const s = [...xs].sort((a, b) => a - b); return s[Math.floor(s.length / 2)]; };
export async function verify(directory) {
  const manifest = JSON.parse(await readFile(join(directory, "capture.json"), "utf8"));
  assert.equal(manifest.schema, "walz-flatbuffers-capture-v1");
  for (const [path, expected] of Object.entries(manifest.hashes))
    assert.equal(hash(await readFile(join(directory, path))), expected, `snapshot changed: ${path}`);
  return manifest;
}
export async function measure(variants, { rounds = 9, targetMs = 30 } = {}) {
  assert.ok(Number.isInteger(rounds) && rounds >= 5 && rounds <= 31);
  assert.ok(Number.isFinite(targetMs) && targetMs >= 1 && targetMs <= 100);
  const entries = Object.entries(variants), results = {};
  const sample = (v, calls) => {
    const start = performance.now();
    for (let i = 0; i < calls; i++) sink = v.fn();
    const elapsed = (performance.now() - start) * 1e6;
    v.check(sink); v.clean?.(); // All correctness/ownership checks are outside the clock.
    return elapsed;
  };
  for (const [name, v] of entries) {
    v.check(v.fn()); v.clean?.();
    let calls = 1;
    while (calls < 1 << 20 && sample(v, calls) < targetMs * 1e6) calls *= 2;
    for (let i = 0; i < 3; i++) sample(v, calls);
    results[name] = { callsPerSample: calls, samplesNs: [] };
  }
  for (let round = 0; round < rounds; round++) for (let i = 0; i < entries.length; i++) {
    const [name, v] = entries[(round + i) % entries.length], r = results[name];
    global.gc?.(); // Automatic collection during a batch remains in its measured cost.
    r.samplesNs.push(sample(v, r.callsPerSample) / r.callsPerSample);
  }
  for (const r of Object.values(results)) {
    r.medianNs = median(r.samplesNs); r.minNs = Math.min(...r.samplesNs); r.maxNs = Math.max(...r.samplesNs);
  }
  return results;
}
function residentInput(raw, bytes) {
  const data = raw["input-new"](bytes.length);
  for (let i = 0; i < bytes.length; i++) raw["input-set"](data, i, bytes[i]);
  return raw["input-bytes"](data);
}
function residentOutput(raw, bytes) {
  return Uint8Array.from({ length: raw["byte-length"](bytes) }, (_, i) => raw["byte-get"](bytes, i));
}
const encodeNames = ["jsFresh", "jsReused", "walzResident", "walzCore", "walzComponent"];
const decodeNames = ["jsMaterialized", "walzResident", "walzCore", "walzComponent"];
function table(report, op, names) {
  return [
    `## ${op}`, "", `Median µs/op. ${op === "encode" ? "All variants produce owned output bytes." : "All variants materialize every field and copy vectors."}`, "",
    `| input | wire bytes | ${names.join(" | ")} |`, `| --- | ---: | ${names.map(() => "---:").join(" | ")} |`,
    ...report.cases.map(c => `| ${c.name} | ${c.bytes} | ${names.map(n => (c[op][n].medianNs / 1000).toFixed(2)).join(" | ")} |`), "",
  ].join("\n");
}
export async function run(directory, output, { rounds = 9, targetMs = 30 } = {}) {
  if (typeof global.gc !== "function") throw new Error("run with node --expose-gc");
  assert.equal(endianness(), "LE", "official typed-array accessor requires little-endian input storage");
  const manifest = await verify(directory);
  await mkdir(output); // Results never replace an existing run.
  const corpus = JSON.parse(await readFile(join(directory, "corpus.json"), "utf8"));
  const reference = createReference(await import(pathToFileURL(join(directory, "reference-package/js/flatbuffers.js"))));
  const { instance } = await WebAssembly.instantiate(await readFile(join(directory, "resident.wasm")));
  const raw = instance.exports, adapters = {};
  for (const backend of ["core", "component"]) {
    const { instantiate } = await import(pathToFileURL(join(directory, backend, "index.js")));
    let arena;
    const program = await instantiate({ instantiateCore: async (module, imports) => {
      const instance = await WebAssembly.instantiate(module, imports);
      if (instance.exports.live) arena = instance.exports;
      return instance;
    } });
    adapters[backend] = { api: program.api, clean() {
      assert.equal(arena.live(), 0, `${backend}: live ABI buffers`);
      assert.ok(Object.values(program.__memory()).every(n => n === 0), `${backend}: live host handles`);
    } };
  }
  const report = {
    schema: "walz-flatbuffers-bench-v1", created: new Date().toISOString(), node: process.version, v8: process.versions.v8,
    cpu: cpus()[0]?.model, platform: process.platform, arch: process.arch, loadavg: loadavg(), rounds, targetMs,
    captureHash: hash(await readFile(join(directory, "capture.json"))), reference: manifest.reference, sizes: manifest.sizes,
    method: { setupExcluded: true, compilationExcluded: true, resident: "opaque Wasm GC refs, one JS/Wasm call per op; no value ABI",
      api: "full input/output ABI conversion included", outputRetained: true, rotatingOrder: true,
      gc: "explicit GC before each measured batch; automatic GC inside timer", decode: "materialize all fields and own vector copies",
      validation: "Walz checks bounds/alignment/UTF-8; official JS adapter checks identifier only", freshBuilderBytes: 64 }, cases: [],
  };
  for (const c of corpus) {
    assert.equal(c.inputHash, hash(JSON.stringify(c.value)));
    const value = materialize(c.value), bytes = new Uint8Array(await readFile(join(directory, "inputs", c.name + ".bin")));
    assert.deepEqual(reference.decode(bytes), value);
    const input = residentInput(raw, bytes), record = raw.decode(input), reused = reference.createEncoder();
    const encodedCheck = b => assert.deepEqual(reference.decode(b), value);
    const recordCheck = m => assert.deepEqual(m, value);
    const encode = {
      jsFresh: { fn: () => reference.encode(value), check: encodedCheck },
      jsReused: { fn: () => reused(value), check: encodedCheck },
      walzResident: { fn: () => raw.encode(record), check: b => encodedCheck(residentOutput(raw, b)) },
    };
    const decode = {
      jsMaterialized: { fn: () => reference.decode(bytes), check: recordCheck },
      walzResident: { fn: () => raw.decode(input), check: m => encodedCheck(residentOutput(raw, raw.encode(m))) },
    };
    const outputBytes = { js: reference.encode(value).length, walz: raw["byte-length"](raw.encode(record)) };
    for (const [backend, { api, clean }] of Object.entries(adapters)) {
      const name = backend === "core" ? "walzCore" : "walzComponent";
      encode[name] = { fn: () => api.encode(value), check: encodedCheck, clean };
      decode[name] = { fn: () => api.decode(bytes), check: result => { assert.equal(result.tag, "ok"); recordCheck(result.val); }, clean };
    }
    const encodeResults = await measure(encode, { rounds, targetMs }), decodeResults = await measure(decode, { rounds, targetMs });
    for (const results of [encodeResults, decodeResults]) for (const result of Object.values(results))
      result.mibPerSecond = bytes.length / (result.medianNs / 1e9) / 1048576;
    for (const [group, baseline] of [[encodeResults, "jsFresh"], [decodeResults, "jsMaterialized"]])
      for (const result of Object.values(group)) result.slowdownVsJs = median(result.samplesNs.map((t, i) => t / group[baseline].samplesNs[i]));
    assert.deepEqual(value, materialize(c.value));
    report.cases.push({ name: c.name, inputHash: c.inputHash, bytes: bytes.length, outputBytes, encode: encodeResults, decode: decodeResults });
    console.log(`${c.name}: encode JS ${(encodeResults.jsFresh.medianNs / 1000).toFixed(2)} µs / Walz resident ${(encodeResults.walzResident.medianNs / 1000).toFixed(2)} µs; decode JS ${(decodeResults.jsMaterialized.medianNs / 1000).toFixed(2)} µs / Walz resident ${(decodeResults.walzResident.medianNs / 1000).toFixed(2)} µs`);
  }
  report.loadavgAfter = loadavg();
  await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2) + "\n", { flag: "wx" });
  const text = `# FlatBuffers speed comparison\n\n${report.node} / V8 ${report.v8} / ${report.cpu} / ${report.platform} ${report.arch}. ${rounds} samples; ${targetMs} ms calibration target.\n\n` +
    `Reference: official FlatBuffers JavaScript ${report.reference.flatbuffers}, with a handwritten adapter for message.fbs. Fresh builders start at 64 bytes on both implementations; jsReused clears and reuses its builder. Every encoder returns independently owned bytes. Decode inputs are the same saved official-JS byte buffers. Materialized vectors are copied on both implementations; this is not the official zero-copy accessor path.\n\n` +
    `Walz decode includes bounds, alignment, length, UTF-8 and terminator validation. The official JS adapter checks the identifier and otherwise trusts valid inputs. These rows compare the current APIs, not identical validation work. Walz resident keeps input values in Wasm GC and returns an opaque GC reference, with one JS/Wasm call per operation. Core/Component include full value ABI transfer; decode also constructs a tagged Result. Resident encode includes the library's finish copy.\n\n` +
    `Builds, instantiation, I/O, input construction, calibration, warmup and output equality/handle checks are outside sample clocks. All outputs are retained and checked against every source field. Each round rotates variant order; explicit GC occurs before each batch, and automatic GC within the batch counts. Raw samples, calibrated iteration counts, environment, source and artifact hashes are preserved. Each variant has its own calibrated iteration count; no samples are removed. Source capture may be dirty and its exact contents are saved.\n\n` +
    table(report, "encode", encodeNames) + "\n" + table(report, "decode", decodeNames) + "\n" +
    `Output lengths may differ because table layout and vtable deduplication are not canonical. See report.json for lengths, throughput, paired sample ratios and raw samples. This is a local V8 measurement on four fixed inputs, not a native FlatBuffers or browser benchmark.\n`;
  await writeFile(join(output, "report.md"), text, { flag: "wx" });
  return report;
}
