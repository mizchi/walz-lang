// A reproducible, finite stress check. Engine heap/RSS observations supplement
// exact live-table counts; they are not a proof for arbitrary host producers.
import assert from "node:assert/strict";
import { mkdir, writeFile, readdir, readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";
import { gzipSync, brotliCompressSync } from "node:zlib";
const backend = process.env.GCWAT_BACKEND === "core" ? "core" : "component";
import { execFile as execFileCallback } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
const execFile = promisify(execFileCallback);
const builder = fileURLToPath(
  new URL(
    backend === "core" ? "./build-core.mjs" : "./build.mjs",
    import.meta.url,
  ),
);
// Keep build-tool modules out of the process whose retained heap is sampled.
const build = (source, output) =>
  execFile(process.execPath, [builder, source, output], {
    maxBuffer: 4 * 1024 * 1024,
  });
import { instrument, collect } from "./testing/memory.mjs";
const directory = resolve(
  backend === "core" ? "target/memory-audit-core" : "target/memory-audit",
);
await mkdir(directory, { recursive: true });
const report = {
  backend,
  node: process.version,
  date: new Date().toISOString(),
  cases: [],
};
for (const fixture of ["buffers", "values", "event-loop"]) {
  const output = join(directory, "checked-" + fixture);
  await build(`tests/component-fixtures/${fixture}.gwat`, output);
  const files = (await readdir(output)).filter((name) =>
    /\.(?:m?js)$/.test(name),
  );
  const javascript = { raw: 0, gzip: 0, brotli: 0 };
  for (const file of files) {
    const bytes = await readFile(join(output, file));
    javascript.raw += bytes.length;
    javascript.gzip += gzipSync(bytes).length;
    javascript.brotli += brotliCompressSync(bytes).length;
  }
  const generatedJs = javascript.raw;
  await instrument(output);
  const runtime = await import(pathToFileURL(join(output, "index.js")));
  let allocator;
  const { api, start, __memory } = await runtime.instantiate({
    instantiateCore: async (module, imports) => {
      const instance = await WebAssembly.instantiate(module, imports);
      if (instance.exports.alloc) allocator = instance.exports;
      return instance;
    },
  });
  const batch = fixture === "event-loop" ? 200 : 1000;
  const work =
    fixture === "buffers"
      ? async () => {
          for (let i = 0; i < batch; i++) {
            const bytes = new Uint8Array(1024);
            api.bytes(bytes);
            api.nested(["x".repeat(512), bytes]);
          }
        }
      : fixture === "values"
        ? async () => {
            for (let i = 0; i < batch; i++) {
              assert.equal(await api.future(), 42n);
              for await (const value of api.stream())
                assert.ok(value === 40n || value === 2n);
              assert.equal(await api.await(Promise.resolve(42n)), 42n);
              assert.equal(await api.first([42n]), 42n);
            }
          }
        : async () => {
            for (let i = 0; i < batch; i++)
              await start("closePending", [60000]).result;
          };
  await work();
  const samples = [];
  for (let round = 0; round < 6; round++) {
    await work();
    const memory = await collect(),
      tables = __memory();
    assert.equal(allocator.live(), 0);
    assert.ok(
      Object.values(tables).every((count) => count === 0),
      JSON.stringify(tables),
    );
    const sample = {
      ...memory,
      linear: allocator.memory.buffer.byteLength,
      live: allocator.live(),
      tables,
    };
    samples.push(sample);
    process.stdout.write(JSON.stringify({ fixture, round, ...sample }) + "\n");
  }
  const heapGrowth = samples.at(-1).heapUsed - samples[0].heapUsed;
  assert.ok(
    heapGrowth < 2 * 1024 * 1024,
    `${fixture}: retained heap grew by ${heapGrowth} bytes after warmup`,
  );
  assert.equal(samples.at(-1).linear, samples[0].linear);
  report.cases.push({
    fixture,
    batch,
    warmupBatches: 1,
    measuredBatches: 6,
    generatedJs,
    javascript,
    heapGrowth,
    samples,
  });
}
await writeFile(
  join(directory, "report.json"),
  JSON.stringify(report, null, 2) + "\n",
);
