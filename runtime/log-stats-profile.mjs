// Native compute samples and unprofiled end-to-end JSPI measurements are
// separate: benchmark.gwat shares the parser/stats, but has no I/O adapters.
import { execFile as rawExec } from "node:child_process";
import { promisify } from "node:util";
import {
  mkdir,
  readFile,
  writeFile,
  readdir,
  copyFile,
} from "node:fs/promises";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { performance } from "node:perf_hooks";
import { createHash } from "node:crypto";
import { cpus } from "node:os";
import assert from "node:assert/strict";
import { build } from "./build-core.mjs";
import { createTextIO, TEXT_IO } from "./text-io.mjs";
import { summarize } from "./profile-summary.mjs";
const exec = promisify(rawExec);
const [dir = "target/profile/log-stats", count = "30000", repeats = "100"] =
  process.argv.slice(2);
const rows = Number(count),
  rounds = Number(repeats),
  output = resolve(dir);
if (
  !Number.isInteger(rows) ||
  rows < 1 ||
  rows > 1000000 ||
  !Number.isInteger(rounds) ||
  rounds < 1 ||
  rounds > 1000
)
  throw new Error("rows must be 1..1000000 and rounds 1..1000");
const source = "examples/apps/log-stats";
await build(join(source, "main.gwat"), output);
for (const directory of [source, "stdlib"]) {
  await mkdir(join(output, "source", directory), { recursive: true });
  for (const name of await readdir(directory))
    if (name.endsWith(".gwat"))
      await copyFile(
        join(directory, name),
        join(output, "source", directory, name),
      );
}
await exec("cargo", [
  "run",
  "--locked",
  "--quiet",
  "--",
  "build",
  join(source, "benchmark.gwat"),
  join(output, "compute.wasm"),
]);
const corpus = [
  "INFO\t12\trequest started",
  "WARN\t40\t遅いリクエスト🦀",
  "ERROR\t7\tfailed",
  "INFO\t8\trequest completed",
  "bad",
  "# comment",
];
const levels = {
  INFO: { count: 0, total_ms: 0, max_ms: 0 },
  WARN: { count: 0, total_ms: 0, max_ms: 0 },
  ERROR: { count: 0, total_ms: 0, max_ms: 0 },
};
const expected = { accepted: 0, rejected: 0, ignored: 0, levels };
const lines = Array.from({ length: rows }, (_, i) => {
  const line = corpus[i % corpus.length];
  if (line.startsWith("#")) expected.ignored++;
  else if (line === "bad") expected.rejected++;
  else {
    expected.accepted++;
    const [level, duration] = line.split("\t"),
      ms = Number(duration),
      bucket = levels[level];
    bucket.count++;
    bucket.total_ms += ms;
    bucket.max_ms = Math.max(bucket.max_ms, ms);
  }
  return line;
});
const bytes = new TextEncoder().encode(lines.join("\n") + "\n");
const checksum =
  (rows + Object.values(levels).reduce((n, l) => n + l.total_ms, 0)) * rounds;
const native = await exec(
  "wasmtime",
  [
    "run",
    "-W",
    "gc=y,function-references=y,exceptions=y",
    `--profile=guest,${join(output, "guest.json")},1ms`,
    "--invoke",
    "run",
    join(output, "compute.wasm"),
    String(rows),
    String(rounds),
  ],
  { timeout: 60000, maxBuffer: 4 * 1024 * 1024 },
);
assert.equal(Number(native.stdout.trim()), checksum);
const profile = summarize(
  JSON.parse(await readFile(join(output, "guest.json"), "utf8")),
);
if (profile.samples < 50)
  throw new Error("too few guest samples; increase rounds");
const { instantiate } = await import(pathToFileURL(join(output, "index.js")));
const modules = new Map();
for (const name of ["core.wasm", "memory.wasm"])
  modules.set(
    name,
    await WebAssembly.compile(await readFile(join(output, name))),
  );
const run = async (probe = false) => {
  let memory,
    reads = 0,
    writes = 0;
  const result = [];
  const { api } = await instantiate({
    getCoreModule: (name) => modules.get(name),
    imports: {
      [TEXT_IO]: createTextIO({
        input: (async function* () {
          for (let i = 0; i < bytes.length; i += 4096)
            yield bytes.subarray(i, i + 4096);
        })(),
        write: async (data) => {
          writes++;
          result.push(Buffer.from(data));
        },
      }),
    },
    onTaskEvent: probe
      ? (event) => {
          if (event.type === "read-pending") reads++;
        }
      : undefined,
    instantiateCore: async (module, imports) => {
      const instance = await WebAssembly.instantiate(module, imports);
      if (instance.exports.alloc) memory = instance.exports;
      return instance;
    },
  });
  const start = performance.now();
  const value = await api.main();
  const milliseconds = performance.now() - start;
  assert.deepEqual(value, { tag: "ok", val: 0 });
  assert.deepEqual(JSON.parse(Buffer.concat(result).toString()), expected);
  assert.equal(memory.live(), 0);
  return {
    milliseconds,
    linearBytes: memory.memory.buffer.byteLength,
    live: memory.live(),
    jspiReads: reads,
    hostWrites: writes,
  };
};
for (let i = 0; i < 3; i++) await run();
const samples = [];
for (let i = 0; i < 7; i++) samples.push((await run()).milliseconds);
const metrics = await run(true);
delete metrics.milliseconds;
const hash = async (name) =>
  createHash("sha256")
    .update(await readFile(join(output, name)))
    .digest("hex");
const report = {
  date: new Date().toISOString(),
  host: {
    platform: process.platform,
    arch: process.arch,
    cpu: cpus()[0]?.model,
  },
  wasmtime: (await exec("wasmtime", ["--version"])).stdout.trim(),
  node: process.version,
  coreHash: await hash("core.wasm"),
  computeHash: await hash("compute.wasm"),
  native: { rows, rounds, checksum, profile },
  jspi: {
    method:
      "memory I/O, compiled modules cached, three warmups, seven execution-only samples, read counters probed afterwards",
    rows,
    inputBytes: bytes.length,
    medianMs: [...samples].sort((a, b) => a - b)[3],
    samplesMs: samples,
    ...metrics,
  },
  expected,
};
await writeFile(
  join(output, "report.json"),
  JSON.stringify(report, null, 2) + "\n",
);
console.log(
  JSON.stringify(
    {
      directory: output,
      samples: profile.samples,
      top: profile.self.slice(0, 6),
      jspi: report.jspi,
    },
    null,
    2,
  ),
);
