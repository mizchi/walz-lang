// Unprofiled V8/JSPI latency. Allocation probes use a separate instrumented
// arena afterwards, never the samples used for the timing comparison.
import { readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import { createTextIO, TEXT_IO } from "./text-io.mjs";

const directories = process.argv
  .slice(2)
  .map((directory) => resolve(directory));
if (!directories.length || directories.length > 2)
  throw new Error("usage: profile-node.mjs directory [comparison-directory]");
const variants = [];
for (const root of directories) {
  const { instantiate } = await import(pathToFileURL(join(root, "index.js")));
  const modules = new Map();
  const load = async (name) => {
    if (!modules.has(name))
      modules.set(name, WebAssembly.compile(await readFile(join(root, name))));
    return modules.get(name);
  };
  for (const name of ["core.wasm", "memory.wasm", "env.wasm"]) await load(name);
  variants.push({ root, instantiate, load, scenarios: [] });
}
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const corpus = (rows, width, unicode) => {
  const pattern =
    unicode === "mixed" ? "abc日本語🦀XYZ" : unicode ? "日本語🦀" : "x";
  const line = pattern.repeat(width / Buffer.byteLength(pattern));
  const values = Array.from({ length: rows }, (_, i) =>
    i % 8 === 0 ? "" : line,
  );
  const data = new TextEncoder().encode(values.join("\n") + "\n");
  const count = rows - Math.ceil(rows / 8);
  const expected = hash((line + "\n").repeat(count));
  return { data, count, expected };
};
const run = async ({ instantiate, load }, data, instrumented) => {
  const output = createHash("sha256");
  let writes = 0,
    reads = 0,
    pulls = 0,
    allocator;
  const io = createTextIO({
    input: (async function* () {
      for (let i = 0; i < data.data.length; i += 4096) {
        if (instrumented) pulls++;
        yield data.data.subarray(i, i + 4096);
      }
      if (instrumented) pulls++;
    })(),
    write: async (bytes) => {
      writes++;
      output.update(bytes);
    },
  });
  const { api } = await instantiate({
    imports: { [TEXT_IO]: io },
    onTaskEvent: instrumented
      ? (event) => {
          if (event.type === "read-pending") reads++;
        }
      : undefined,
    getCoreModule: (name) =>
      load(instrumented && name === "memory.wasm" ? "env.wasm" : name),
    instantiateCore: async (m, imports) => {
      const instance = await WebAssembly.instantiate(m, imports);
      if (instance.exports.alloc) allocator = instance.exports;
      return instance;
    },
  });
  const start = performance.now();
  const result = await api.main();
  const milliseconds = performance.now() - start;
  assert.deepEqual(result, { tag: "ok", val: data.count });
  assert.equal(writes, data.count);
  assert.equal(output.digest("hex"), data.expected);
  assert.equal(allocator.live(), 0);
  return {
    milliseconds,
    live: allocator.live(),
    linearBytes: allocator.memory.buffer.byteLength,
    ...(instrumented
      ? {
          allocations: Number(allocator["alloc-count"]()),
          requestedBytes: Number(allocator["alloc-bytes"]()),
          peakLive: allocator["peak-live"](),
          jspiReads: reads,
          hostWrites: writes,
          inputPulls: pulls,
        }
      : {}),
  };
};
const median = (values) =>
  [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const configurations = [
  ["ascii-64", 64, false, 12000],
  ["unicode-52", 52, true, 12000],
  ["mixed-76", 76, "mixed", 12000],
  ["ascii-4096", 4096, false, 1500],
];
for (const [name, width, unicode, rows] of configurations) {
  const data = corpus(rows, width, unicode);
  // Full-size warmups include guest tier-up and the JS binding call sites.
  for (let warm = 0; warm < 4; warm++)
    for (const variant of variants) await run(variant, data, false);
  const samples = variants.map(() => []);
  for (let i = 0; i < 9; i++) {
    const order = variants.map((_, j) => j);
    if (i % 2) order.reverse();
    for (const index of order)
      samples[index].push(
        (await run(variants[index], data, false)).milliseconds,
      );
  }
  variants.forEach((variant, i) =>
    variant.scenarios.push({
      name,
      rows,
      width,
      unicode,
      nonEmpty: data.count,
      inputBytes: data.data.length,
      medianMs: median(samples[i]),
      samplesMs: samples[i],
    }),
  );
}
// Probe after ALL latency samples: changing imported alloc targets must not
// affect guest tiering or call-site feedback in a later timing scenario.
for (const variant of variants) {
  for (const scenario of variant.scenarios) {
    const metrics = await run(
      variant,
      corpus(scenario.rows, scenario.width, scenario.unicode),
      true,
    );
    delete metrics.milliseconds;
    scenario.metrics = metrics;
  }
}
const method =
  "unprofiled execution only, memory I/O, SHA-256 validated, four full-size warmups, nine alternating samples, allocation probes afterwards";
if (variants.length === 1) {
  const { root, scenarios } = variants[0];
  await writeFile(
    join(root, "node-metrics.json"),
    JSON.stringify({ engine: process.version, method, scenarios }, null, 2) +
      "\n",
  );
  console.log(
    JSON.stringify(
      scenarios.map(({ name, medianMs, metrics }) => ({
        name,
        medianMs,
        ...metrics,
      })),
    ),
  );
} else {
  const [a, b] = variants;
  const scenarios = a.scenarios.map((old, i) => {
    const next = b.scenarios[i];
    return {
      name: old.name,
      beforeMs: old.medianMs,
      afterMs: next.medianMs,
      speedup: old.medianMs / next.medianMs,
      medianPairedSpeedup: median(
        old.samplesMs.map((ms, j) => ms / next.samplesMs[j]),
      ),
      beforeSamplesMs: old.samplesMs,
      afterSamplesMs: next.samplesMs,
      allocationsBefore: old.metrics.allocations,
      allocationsAfter: next.metrics.allocations,
      requestedBytesBefore: old.metrics.requestedBytes,
      requestedBytesAfter: next.metrics.requestedBytes,
      linearBytes: next.metrics.linearBytes,
      live: next.metrics.live,
      jspiReadsBefore: old.metrics.jspiReads,
      jspiReadsAfter: next.metrics.jspiReads,
      hostWrites: next.metrics.hostWrites,
    };
  });
  console.log(
    JSON.stringify({
      engine: process.version,
      method,
      before: a.root,
      after: b.root,
      scenarios,
    }),
  );
}
