// Interleave saved before/after bindings while holding the guest binaries fixed.
import assert from "node:assert/strict";
import { readFile, writeFile, mkdir, copyFile } from "node:fs/promises";
import { join, resolve, dirname } from "node:path";
import { pathToFileURL, fileURLToPath } from "node:url";
import { cpus } from "node:os";
import { hash, verify } from "./simd-kernels/runner.mjs";

const [beforeArg, afterArg, outputArg, roundsArg = "9", sizesArg = "4096,16384,262144"] = process.argv.slice(2);
if (!beforeArg || !afterArg || !outputArg) throw new Error("usage: simd-kernels-compare.mjs before after new-output [rounds] [sizes]");
const before = resolve(beforeArg), after = resolve(afterArg), output = resolve(outputArg);
const rounds = Number(roundsArg), sizes = sizesArg.split(",").map(Number);
assert.ok(Number.isInteger(rounds) && rounds >= 5 && rounds <= 31);
assert.ok(sizes.length && sizes.every(n => Number.isInteger(n) && n > 0 && n <= 1048576));
const manifests = await Promise.all([verify(before), verify(after)]);
const unchanged = [
  "workloads.mjs", "runner.mjs", "core/core.wasm", "core/memory.wasm",
  "core/index.js", "core/core.mjs", "core/host.mjs", "core/tasks.mjs", "core/resources.mjs",
  "component/component.wasm",
];
for (const name of unchanged) assert.equal(manifests[0].hashes[name], manifests[1].hashes[name], `comparison requires identical ${name}`);
const { measure } = await import(pathToFileURL(join(before, "runner.mjs")));
const { corpus } = await import(pathToFileURL(join(before, "workloads.mjs")));
await mkdir(dirname(output), { recursive: true });
await mkdir(output); // A new comparison never overwrites earlier measurements.
const loaded = {};
for (const [label, directory] of [["before", before], ["after", after]])
  for (const backend of ["core", "component"]) {
    const { instantiate } = await import(pathToFileURL(join(directory, backend, "index.js")));
    let arena;
    const { api } = await instantiate({ instantiateCore: async (m, i) => {
      const instance = await WebAssembly.instantiate(m, i);
      if (instance.exports.live) arena = instance.exports;
      return instance;
    } });
    loaded[`${label}${backend}`] = { api, arena: () => arena };
  }
const report = {
  schema: "walz-list-abi-comparison-v1", created: new Date().toISOString(),
  node: process.version, v8: process.versions.v8, cpu: cpus()[0]?.model,
  platform: process.platform, arch: process.arch, execArgv: process.execArgv,
  rounds, sizes, unchanged,
  before: hash(await readFile(join(before, "capture.json"))),
  after: hash(await readFile(join(after, "capture.json"))),
  comparator: hash(await readFile(fileURLToPath(import.meta.url))), cases: [],
};
for (const length of sizes) for (const c of corpus(length).filter(c => !c.name.startsWith("bytes-") || c.name === "bytes-index-miss")) {
  const variants = {};
  for (const [label, { api }] of Object.entries(loaded)) {
    const fn = api[`${c.portable}Simd`];
    variants[label] = c.b ? () => fn(c.a, c.b) : c.kind === "bytes" ? () => fn(c.a, c.needle) : () => fn(c.a);
    if (label.endsWith("core") && c.kind !== "bytes") {
      const a = Array.from(c.a), b = c.b && Array.from(c.b);
      variants[`${label}Array`] = b ? () => fn(a, b) : () => fn(a);
    }
  }
  const timings = measure(variants, c.expected, rounds, 8);
  for (const instance of Object.values(loaded)) assert.equal(instance.arena().live(), 0, "ABI buffer leak");
  const inputHash = hash(Buffer.concat([c.a, c.b].filter(Boolean).map(a => Buffer.from(a.buffer, a.byteOffset, a.byteLength))));
  report.cases.push({ name: c.name, length, expected: c.expected, inputHash, timings });
  console.log(`${c.name} n=${length}: Core ${(timings.beforecore.medianNs / timings.aftercore.medianNs).toFixed(2)}x`);
  global.gc?.();
}
await writeFile(join(output, "report.json"), JSON.stringify(report, null, 2) + "\n", { flag: "wx" });
for (const [name, directory] of [["before", before], ["after", after]])
  await copyFile(join(directory, "capture.json"), join(output, `${name}-capture.json`));
await copyFile(fileURLToPath(import.meta.url), join(output, "comparator.mjs"));
const lines = [
  "# Numeric list ABI comparison", "", `${report.node} / ${report.cpu}; ${rounds} interleaved rounds. Median microseconds/call.`, "",
  "Identical guest binaries, workload and measurement code. Timings include input copies, packing, SIMD calculation and ABI cleanup. Array fallback remains validated per element. Component and bytes are controls.", "",
  "| workload | n | Core before | Core after | speedup | Array before | Array after | Component before | Component after |",
  "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
];
for (const c of report.cases) {
  const time = name => c.timings[name] ? (c.timings[name].medianNs / 1000).toFixed(3) : "—";
  lines.push(`| ${c.name} | ${c.length} | ${time("beforecore")} | ${time("aftercore")} | ${(c.timings.beforecore.medianNs / c.timings.aftercore.medianNs).toFixed(2)}x | ${time("beforecoreArray")} | ${time("aftercoreArray")} | ${time("beforecomponent")} | ${time("aftercomponent")} |`);
}
await writeFile(join(output, "report.md"), lines.join("\n") + "\n", { flag: "wx" });
console.log(output);
